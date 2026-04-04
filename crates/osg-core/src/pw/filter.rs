//! Safe wrapper around `pw_filter` for inline DSP processing.
//!
//! Isolates ALL unsafe PipeWire FFI behind a safe API. The `OsgFilter`
//! creates a PW filter node with stereo in/out, applies biquad EQ in
//! the process callback, and reports peak levels.
//!
//! Rust 2024 edition requires explicit `unsafe {}` blocks even inside
//! `unsafe fn`. Since this module is entirely FFI glue, we allow the
//! legacy behavior to keep the code readable.
#![allow(unsafe_op_in_unsafe_fn)]
//!
//! Thread model:
//! - `OsgFilter` is created on the PW mainloop thread.
//! - The `process` callback runs on the PW real-time thread.
//! - EQ params: main → RT via `ArcSwap` (lock-free).
//! - Peak levels: RT → main via packed `AtomicU64`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use arc_swap::ArcSwap;

use crate::graph::EqConfig;
use crate::pw::biquad::{BiquadState, Coefficients, compute_coefficients};

const SAMPLE_RATE: f32 = 48000.0;
const MAX_BANDS: usize = 10;
/// Max macro bands (Bass, Voice, Treble) — separate from user's 10 bands.
/// Used by the frontend; kept here for parity with MAX_BANDS.
pub const MAX_MACRO_BANDS: usize = 3;

// ---------------------------------------------------------------------------
// RT-safe shared state
// ---------------------------------------------------------------------------

/// Pack two f32 peak values into a single atomic u64.
pub fn pack_peaks(left: f32, right: f32) -> u64 {
    let l = left.to_bits() as u64;
    let r = (right.to_bits() as u64) << 32;
    l | r
}

/// Unpack two f32 peak values from a single atomic u64.
pub fn unpack_peaks(packed: u64) -> (f32, f32) {
    let l = f32::from_bits(packed as u32);
    let r = f32::from_bits((packed >> 32) as u32);
    (l, r)
}

/// Pre-computed biquad coefficients for all bands.
#[derive(Debug)]
pub struct CompiledEq {
    pub bands: Vec<(Coefficients, bool)>,
}

impl CompiledEq {
    pub fn from_config(config: &EqConfig) -> Self {
        let bands = config
            .bands
            .iter()
            .take(MAX_BANDS)
            .map(|b| {
                let coeffs =
                    compute_coefficients(b.filter_type, b.frequency, b.gain, b.q, SAMPLE_RATE);
                (coeffs, b.enabled && config.enabled)
            })
            .collect();
        Self { bands }
    }

    pub fn empty() -> Self {
        Self { bands: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Effects DSP params (shared main→RT via ArcSwap)
// ---------------------------------------------------------------------------

/// Compressor parameters. Operates on per-sample envelope.
#[derive(Debug, Clone)]
pub struct CompressorParams {
    pub enabled: bool,
    pub threshold: f32, // dB (negative, e.g., -18.0)
    pub ratio: f32,     // e.g., 3.0 for 3:1
    pub attack: f32,    // seconds
    pub release: f32,   // seconds
    pub makeup: f32,    // dB
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -18.0,
            ratio: 3.0,
            attack: 0.008,
            release: 0.15,
            makeup: 4.0,
        }
    }
}

/// Noise gate parameters.
#[derive(Debug, Clone)]
pub struct GateParams {
    pub enabled: bool,
    pub threshold: f32, // dB (e.g., -45.0)
    pub hold: f32,      // seconds
    pub attack: f32,    // seconds
    pub release: f32,   // seconds
}

impl Default for GateParams {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -45.0,
            hold: 0.15,
            attack: 0.001,
            release: 0.05,
        }
    }
}

/// De-esser parameters — simple sidechain bandpass + gain reduction.
#[derive(Debug, Clone)]
pub struct DeEsserParams {
    pub enabled: bool,
    pub frequency: f32, // center Hz (5000-8000)
    pub threshold: f32, // dB
    pub reduction: f32, // max reduction in dB (positive, e.g., 6.0)
}

impl Default for DeEsserParams {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: 6000.0,
            threshold: -20.0,
            reduction: 6.0,
        }
    }
}

/// Limiter parameters — brickwall peak limiter.
#[derive(Debug, Clone)]
pub struct LimiterParams {
    pub enabled: bool,
    pub ceiling: f32, // dBFS (e.g., -1.0)
    pub release: f32, // seconds
}

impl Default for LimiterParams {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling: -1.0,
            release: 0.05,
        }
    }
}

/// All effects params bundled for ArcSwap sharing.
#[derive(Debug, Clone, Default)]
pub struct EffectsParams {
    pub compressor: CompressorParams,
    pub gate: GateParams,
    pub de_esser: DeEsserParams,
    pub limiter: LimiterParams,
    /// Volume boost in dB (0–12). Applied as linear gain after limiter.
    pub boost: f32,
}

/// Per-channel envelope state for compressor/gate (lives in CallbackData, not shared).
#[derive(Debug, Default)]
pub(super) struct EnvelopeState {
    pub(super) compressor_env: f32,    // current envelope level (linear)
    pub(super) gate_env: f32,          // gate envelope (0.0 = closed, 1.0 = open)
    pub(super) gate_hold_counter: f32, // samples remaining in hold phase
}

/// Shared handle for lock-free EQ/volume/mute parameter passing and peak reading.
#[derive(Clone, Debug)]
pub struct FilterHandle {
    eq: Arc<ArcSwap<CompiledEq>>,
    effects: Arc<ArcSwap<EffectsParams>>,
    peaks: Arc<AtomicU64>,
    volume_left: Arc<AtomicU32>,
    volume_right: Arc<AtomicU32>,
    muted: Arc<AtomicBool>,
    /// When true, filter passes audio through without EQ processing.
    /// Always-resident filters start bypassed; enabling EQ clears this flag.
    bypassed: Arc<AtomicBool>,
}

impl Default for FilterHandle {
    fn default() -> Self {
        Self {
            eq: Arc::new(ArcSwap::new(Arc::new(CompiledEq::empty()))),
            effects: Arc::new(ArcSwap::new(Arc::new(EffectsParams::default()))),
            peaks: Arc::new(AtomicU64::new(0)),
            volume_left: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            volume_right: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            muted: Arc::new(AtomicBool::new(false)),
            bypassed: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl FilterHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set bypass state (lock-free, any thread). Bypassed = passthrough.
    /// Uses Release ordering to ensure EQ coefficients stored via ArcSwap
    /// are visible to the RT thread before it reads this flag.
    pub fn set_bypassed(&self, bypassed: bool) {
        self.bypassed.store(bypassed, Ordering::Release);
    }

    /// Read bypass state (lock-free).
    /// Uses Acquire ordering to synchronize with `set_bypassed` / `set_eq`
    /// Release stores, ensuring the RT thread sees the latest EQ coefficients.
    pub fn is_bypassed(&self) -> bool {
        self.bypassed.load(Ordering::Acquire)
    }

    /// Update EQ parameters and clear bypass (lock-free, any thread).
    pub fn set_eq(&self, config: &EqConfig) {
        self.eq.store(Arc::new(CompiledEq::from_config(config)));
        // Enabling EQ implicitly clears bypass.
        // Uses Release ordering to ensure the ArcSwap store above is
        // visible to the RT thread before it reads the bypassed flag.
        if !config.bands.is_empty() {
            self.bypassed.store(false, Ordering::Release);
        }
    }

    /// Update effects parameters (lock-free, any thread).
    pub fn set_effects(&self, params: EffectsParams) {
        self.effects.store(Arc::new(params));
    }

    /// Load current effects params (for RT callback).
    pub fn load_effects(&self) -> arc_swap::Guard<Arc<EffectsParams>> {
        self.effects.load()
    }

    /// Set stereo volume (lock-free, any thread). Values 0.0–1.0.
    /// Clamp to valid range — values above 1.0 are boost, above 1.5 risks distortion
    pub fn set_volume(&self, left: f32, right: f32) {
        let left = left.clamp(0.0, 1.5);
        let right = right.clamp(0.0, 1.5);
        self.volume_left.store(left.to_bits(), Ordering::Relaxed);
        self.volume_right.store(right.to_bits(), Ordering::Relaxed);
    }

    /// Set mute state (lock-free, any thread).
    pub fn set_mute(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Read current volume (lock-free).
    pub fn volume(&self) -> (f32, f32) {
        (
            f32::from_bits(self.volume_left.load(Ordering::Relaxed)),
            f32::from_bits(self.volume_right.load(Ordering::Relaxed)),
        )
    }

    /// Read current mute state (lock-free).
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Read latest peak levels (lock-free).
    pub fn peak(&self) -> (f32, f32) {
        unpack_peaks(self.peaks.load(Ordering::Relaxed))
    }

    /// Load current compiled EQ (for RT callback).
    pub fn load_eq(&self) -> arc_swap::Guard<Arc<CompiledEq>> {
        self.eq.load()
    }

    /// Store peak values (for RT callback).
    pub fn store_peaks(&self, left: f32, right: f32) {
        self.peaks.store(pack_peaks(left, right), Ordering::Relaxed);
    }
}

/// Process a block of mono audio through the EQ biquad cascade.
pub fn process_block(
    input: &[f32],
    output: &mut [f32],
    eq: &CompiledEq,
    states: &mut [BiquadState],
) -> f32 {
    let mut peak: f32 = 0.0;
    for (i, &sample) in input.iter().enumerate() {
        let mut s = sample;
        for (band_idx, (coeffs, enabled)) in eq.bands.iter().enumerate() {
            if *enabled && let Some(state) = states.get_mut(band_idx) {
                s = state.process(s, coeffs);
            }
        }
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
        output[i] = s;
    }
    peak
}

// Effects DSP processing functions live in effects_dsp.rs.
use super::effects_dsp::{
    apply_boost, apply_compressor, apply_de_esser, apply_gate, apply_limiter,
};

// ---------------------------------------------------------------------------
// OsgFilter — the actual pw_filter wrapper (requires PW mainloop thread)
// ---------------------------------------------------------------------------

/// Data passed to the PW process callback via the `data` pointer.
/// Lives on the heap, leaked via `Box::into_raw`, reclaimed on drop.
struct CallbackData {
    handle: FilterHandle,
    states_l: Vec<BiquadState>,
    states_r: Vec<BiquadState>,
    env_l: EnvelopeState,
    env_r: EnvelopeState,
    in_port_l: *mut std::os::raw::c_void,
    in_port_r: *mut std::os::raw::c_void,
    out_port_l: *mut std::os::raw::c_void,
    out_port_r: *mut std::os::raw::c_void,
}

/// A PipeWire filter node for inline stereo DSP.
/// Must be created and used on the PW mainloop thread.
#[allow(missing_debug_implementations)]
pub struct OsgFilter {
    filter: *mut pipewire_sys::pw_filter,
    handle: FilterHandle,
    data: *mut CallbackData,
    // Box-pinned: PW holds raw pointers into these. They must not move.
    _listener: Box<libspa_sys::spa_hook>,
    _events: Box<pipewire_sys::pw_filter_events>,
}

impl OsgFilter {
    /// Create a new stereo filter node on the PW mainloop thread.
    ///
    /// Uses `pw_filter_new` with the existing PW core so the filter
    /// appears in the same registry session as all other OSG nodes.
    /// The filter gets 4 mono DSP ports: in_FL, in_FR, out_FL, out_FR.
    ///
    /// # Safety
    /// Must be called from the PW mainloop thread. The `core_ptr` must
    /// be a valid `*mut pw_core` from the running PW connection.
    #[allow(unsafe_code, clippy::too_many_lines, clippy::too_many_arguments)]
    pub unsafe fn new(
        core_ptr: *mut pipewire_sys::pw_core,
        name: &str,
        description: &str,
    ) -> Result<Self, String> {
        // SAFETY: All FFI calls in this function are safe because:
        // 1. `core_ptr` is guaranteed valid by the caller (from the running PW connection).
        // 2. `data` is heap-allocated via `Box::into_raw` and lives until `drop()` reclaims it.
        //    The `data` pointer is passed to PW as a callback user-data pointer — PW never
        //    frees it; we own the lifetime exclusively.
        // 3. `events` and `listener` are Box-pinned so their addresses remain stable for PW's
        //    internal pointer references. They are stored as fields and dropped with `self`.
        // 4. All `CString` values are validated before use and live long enough for the
        //    `pw_properties_new` call (which copies the strings internally).
        // 5. Port pointers returned by `pw_filter_add_port` are stored in `CallbackData` and
        //    only read by the RT process callback — PW guarantees they remain valid while
        //    the filter is connected.
        // 6. `pw_filter_destroy` in `drop()` is the exclusive cleanup path — no double-free
        //    because `Self` is not `Clone` and `filter`/`data` are exclusively owned.
        use std::ffi::CString;
        use std::ptr;

        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let c_desc = CString::new(description).map_err(|e| e.to_string())?;

        let handle = FilterHandle::new();
        let data = Box::into_raw(Box::new(CallbackData {
            handle: handle.clone(),
            states_l: vec![BiquadState::default(); MAX_BANDS],
            states_r: vec![BiquadState::default(); MAX_BANDS],
            env_l: EnvelopeState::default(),
            env_r: EnvelopeState::default(),
            in_port_l: ptr::null_mut(),
            in_port_r: ptr::null_mut(),
            out_port_l: ptr::null_mut(),
            out_port_r: ptr::null_mut(),
        }));

        // Build properties for an inline DSP filter.
        // No media.class — prevents WirePlumber from auto-routing outputs
        // to the default sink (which causes audio leaks).
        // node.passive=true prevents PW scheduler from driving the node.
        let props = pipewire_sys::pw_properties_new(
            c"media.type".as_ptr().cast::<std::os::raw::c_char>(),
            c"Audio".as_ptr().cast::<std::os::raw::c_char>(),
            c"node.name".as_ptr().cast::<std::os::raw::c_char>(),
            c_name.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.nick".as_ptr().cast::<std::os::raw::c_char>(),
            c_desc.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.description".as_ptr().cast::<std::os::raw::c_char>(),
            c_desc.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.virtual".as_ptr().cast::<std::os::raw::c_char>(),
            c"true".as_ptr().cast::<std::os::raw::c_char>(),
            c"node.passive".as_ptr().cast::<std::os::raw::c_char>(),
            c"true".as_ptr().cast::<std::os::raw::c_char>(),
            c"pulse.disable".as_ptr().cast::<std::os::raw::c_char>(),
            c"true".as_ptr().cast::<std::os::raw::c_char>(),
            c"audio.position".as_ptr().cast::<std::os::raw::c_char>(),
            c"FL,FR".as_ptr().cast::<std::os::raw::c_char>(),
            ptr::null::<std::os::raw::c_void>(),
        );

        // Create filter on the existing core (same registry session)
        let filter = pipewire_sys::pw_filter_new(core_ptr, c_name.as_ptr(), props);

        if filter.is_null() {
            drop(Box::from_raw(data));
            return Err("pw_filter_new returned null".into());
        }

        // Attach event listener — Box-pinned so PW's pointers stay valid
        let mut events = Box::new(std::mem::zeroed::<pipewire_sys::pw_filter_events>());
        events.version = pipewire_sys::PW_VERSION_FILTER_EVENTS;
        events.process = Some(on_process);

        let mut listener = Box::new(std::mem::zeroed::<libspa_sys::spa_hook>());
        pipewire_sys::pw_filter_add_listener(
            filter,
            &mut *listener,
            &*events,
            data as *mut std::os::raw::c_void,
        );

        // Add stereo input ports (FL, FR)
        let in_port_l = pipewire_sys::pw_filter_add_port(
            filter,
            libspa_sys::SPA_DIRECTION_INPUT,
            pipewire_sys::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            std::mem::size_of::<*mut std::os::raw::c_void>(), // port_data_size
            pipewire_sys::pw_properties_new(
                c"format.dsp".as_ptr().cast::<std::os::raw::c_char>(),
                c"32 bit float mono audio"
                    .as_ptr()
                    .cast::<std::os::raw::c_char>(),
                c"port.name".as_ptr().cast::<std::os::raw::c_char>(),
                c"input_FL".as_ptr().cast::<std::os::raw::c_char>(),
                c"audio.channel".as_ptr().cast::<std::os::raw::c_char>(),
                c"FL".as_ptr().cast::<std::os::raw::c_char>(),
                ptr::null::<std::os::raw::c_void>(),
            ),
            ptr::null_mut(),
            0,
        );

        let in_port_r = pipewire_sys::pw_filter_add_port(
            filter,
            libspa_sys::SPA_DIRECTION_INPUT,
            pipewire_sys::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            0,
            pipewire_sys::pw_properties_new(
                c"format.dsp".as_ptr().cast::<std::os::raw::c_char>(),
                c"32 bit float mono audio"
                    .as_ptr()
                    .cast::<std::os::raw::c_char>(),
                c"port.name".as_ptr().cast::<std::os::raw::c_char>(),
                c"input_FR".as_ptr().cast::<std::os::raw::c_char>(),
                c"audio.channel".as_ptr().cast::<std::os::raw::c_char>(),
                c"FR".as_ptr().cast::<std::os::raw::c_char>(),
                ptr::null::<std::os::raw::c_void>(),
            ),
            ptr::null_mut(),
            0,
        );

        // Add stereo output ports (FL, FR)
        let out_port_l = pipewire_sys::pw_filter_add_port(
            filter,
            libspa_sys::SPA_DIRECTION_OUTPUT,
            pipewire_sys::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            0,
            pipewire_sys::pw_properties_new(
                c"format.dsp".as_ptr().cast::<std::os::raw::c_char>(),
                c"32 bit float mono audio"
                    .as_ptr()
                    .cast::<std::os::raw::c_char>(),
                c"port.name".as_ptr().cast::<std::os::raw::c_char>(),
                c"output_FL".as_ptr().cast::<std::os::raw::c_char>(),
                c"audio.channel".as_ptr().cast::<std::os::raw::c_char>(),
                c"FL".as_ptr().cast::<std::os::raw::c_char>(),
                ptr::null::<std::os::raw::c_void>(),
            ),
            ptr::null_mut(),
            0,
        );

        let out_port_r = pipewire_sys::pw_filter_add_port(
            filter,
            libspa_sys::SPA_DIRECTION_OUTPUT,
            pipewire_sys::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            0,
            pipewire_sys::pw_properties_new(
                c"format.dsp".as_ptr().cast::<std::os::raw::c_char>(),
                c"32 bit float mono audio"
                    .as_ptr()
                    .cast::<std::os::raw::c_char>(),
                c"port.name".as_ptr().cast::<std::os::raw::c_char>(),
                c"output_FR".as_ptr().cast::<std::os::raw::c_char>(),
                c"audio.channel".as_ptr().cast::<std::os::raw::c_char>(),
                c"FR".as_ptr().cast::<std::os::raw::c_char>(),
                ptr::null::<std::os::raw::c_void>(),
            ),
            ptr::null_mut(),
            0,
        );

        // Store port pointers in callback data
        (*data).in_port_l = in_port_l;
        (*data).in_port_r = in_port_r;
        (*data).out_port_l = out_port_l;
        (*data).out_port_r = out_port_r;

        if in_port_l.is_null()
            || in_port_r.is_null()
            || out_port_l.is_null()
            || out_port_r.is_null()
        {
            tracing::warn!(
                "[PW] filter '{name}' port creation failed: in_l={} in_r={} out_l={} out_r={}",
                !in_port_l.is_null(),
                !in_port_r.is_null(),
                !out_port_l.is_null(),
                !out_port_r.is_null(),
            );
        }

        // Connect with RT processing flag
        let result = pipewire_sys::pw_filter_connect(
            filter,
            pipewire_sys::pw_filter_flags_PW_FILTER_FLAG_RT_PROCESS,
            ptr::null_mut(),
            0,
        );
        if result < 0 {
            pipewire_sys::pw_filter_destroy(filter);
            drop(Box::from_raw(data));
            return Err(format!("pw_filter_connect failed: {result}"));
        }

        Ok(Self {
            filter,
            handle,
            data,
            _listener: listener,
            _events: events,
        })
    }

    /// Get the PW global node ID (available once the filter is streaming).
    #[allow(unsafe_code)]
    pub fn node_id(&self) -> Option<u32> {
        // SAFETY: `self.filter` is a valid pw_filter pointer created in `new()`
        // and only invalidated when `drop()` runs — which consumes self, so
        // it cannot be called while this method holds a reference.
        let id = unsafe { pipewire_sys::pw_filter_get_node_id(self.filter) };
        if id == u32::MAX { None } else { Some(id) }
    }

    /// Get the shared handle for EQ parameter passing and peak reading.
    pub fn handle(&self) -> &FilterHandle {
        &self.handle
    }
}

/// Convenience: create an OsgFilter for a channel group node.
/// Called from mainloop.rs create_group_node() for Source/Duplex kinds.
///
/// # Safety
/// `core_ptr` must be a valid `*mut pw_core` from the running PW connection.
/// Must be called from the PW mainloop thread.
#[allow(unsafe_code)]
pub unsafe fn create_group_filter(
    core_ptr: *mut pipewire_sys::pw_core,
    name: &str,
    id: ulid::Ulid,
    _kind: super::GroupNodeKind,
) -> Result<OsgFilter, String> {
    let node_name = format!("osg.group.{id}");
    // SAFETY: Caller guarantees `core_ptr` is a valid pw_core from the running
    // PW connection, and this is called from the PW mainloop thread.
    let filter = unsafe { OsgFilter::new(core_ptr, &node_name, name) }
        .map_err(|e| format!("filter '{name}': {e}"))?;
    tracing::debug!(
        "[PW] created filter '{}' — node_id: {:?}",
        name,
        filter.node_id()
    );
    Ok(filter)
}

impl Drop for OsgFilter {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: `self.filter` was created by `pw_filter_new` and is valid
        // until destroyed. `self.data` was allocated via `Box::into_raw` in
        // `new()` and is only reclaimed here in `drop()`, ensuring no double-free.
        // Both pointers are exclusively owned by this struct.
        unsafe {
            pipewire_sys::pw_filter_destroy(self.filter);
            drop(Box::from_raw(self.data));
        }
    }
}

// ---------------------------------------------------------------------------
// PW RT callback
// ---------------------------------------------------------------------------

/// Process callback — runs on PW real-time thread.
/// Reads stereo input, applies EQ cascade, volume gain, mute, computes peaks, writes output.
///
/// # SAFETY
/// This function is called by PipeWire's RT thread via the `process` function pointer
/// registered in `OsgFilter::new()`. Safety guarantees:
///
/// 1. **`data` pointer lifetime**: The `CallbackData` is allocated via `Box::into_raw` in
///    `OsgFilter::new()` and reclaimed exclusively in `OsgFilter::drop()`. The `data`
///    pointer is valid for the entire lifetime of the filter — PW calls `on_process` only
///    while the filter is connected, which is strictly within the filter's lifetime.
///
/// 2. **Buffer validity**: `pw_filter_get_dsp_buffer` returns pointers to PW-managed
///    buffers that are valid for the duration of this process callback invocation. The
///    `n_samples` value comes from `(*position).clock.duration`, which PW guarantees is
///    non-zero and matches the buffer sizes. Null checks on output pointers prevent
///    dereferencing invalid buffers.
///
/// 3. **`position` pointer**: PW always provides a valid `spa_io_position` struct during
///    process callbacks. Null check is a defensive guard.
///
/// 4. **No mutation of shared state**: The callback reads params via `FilterHandle`
///    (lock-free atomics/ArcSwap) and writes only to `d.states_l`, `d.states_r`,
///    `d.env_l`, `d.env_r` which are exclusively owned by this `CallbackData`.
#[allow(unsafe_code, clippy::too_many_lines)]
unsafe extern "C" fn on_process(
    data: *mut std::os::raw::c_void,
    position: *mut libspa_sys::spa_io_position,
) {
    let d = &mut *(data as *mut CallbackData);

    if position.is_null() {
        return;
    }
    let n_samples = (*position).clock.duration as u32;
    if n_samples == 0 {
        return;
    }

    let in_l = pipewire_sys::pw_filter_get_dsp_buffer(d.in_port_l, n_samples) as *const f32;
    let in_r = pipewire_sys::pw_filter_get_dsp_buffer(d.in_port_r, n_samples) as *const f32;
    let out_l = pipewire_sys::pw_filter_get_dsp_buffer(d.out_port_l, n_samples) as *mut f32;
    let out_r = pipewire_sys::pw_filter_get_dsp_buffer(d.out_port_r, n_samples) as *mut f32;

    let n = n_samples as usize;
    let muted = d.handle.is_muted();
    let bypassed = d.handle.is_bypassed();
    let (vol_l, vol_r) = d.handle.volume();

    let mut peak_l: f32 = 0.0;
    let mut peak_r: f32 = 0.0;

    // Always write output — silence if no input or muted, to prevent graph stalls.
    if !out_l.is_null() {
        let out_slice_l = std::slice::from_raw_parts_mut(out_l, n);
        if muted || in_l.is_null() {
            out_slice_l.fill(0.0);
        } else {
            let in_slice_l = std::slice::from_raw_parts(in_l, n);
            let fx = d.handle.load_effects();
            if bypassed {
                // Passthrough — copy input, apply effects only (no EQ)
                out_slice_l.copy_from_slice(in_slice_l);
            } else {
                let eq = d.handle.load_eq();
                process_block(in_slice_l, out_slice_l, &eq, &mut d.states_l);
            }
            // Effects chain: gate → compressor → de-esser → limiter → boost
            apply_gate(out_slice_l, &fx.gate, &mut d.env_l, SAMPLE_RATE);
            apply_compressor(out_slice_l, &fx.compressor, &mut d.env_l, SAMPLE_RATE);
            apply_de_esser(out_slice_l, &fx.de_esser);
            apply_limiter(out_slice_l, &fx.limiter);
            apply_boost(out_slice_l, fx.boost);
            // Volume gain
            if (vol_l - 1.0).abs() > f32::EPSILON {
                for s in out_slice_l.iter_mut() {
                    *s *= vol_l;
                }
            }
            peak_l = out_slice_l.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
        }
    }
    if !out_r.is_null() {
        let out_slice_r = std::slice::from_raw_parts_mut(out_r, n);
        if muted || in_r.is_null() {
            out_slice_r.fill(0.0);
        } else {
            let in_slice_r = std::slice::from_raw_parts(in_r, n);
            let fx = d.handle.load_effects();
            if bypassed {
                out_slice_r.copy_from_slice(in_slice_r);
            } else {
                let eq = d.handle.load_eq();
                process_block(in_slice_r, out_slice_r, &eq, &mut d.states_r);
            }
            // Effects chain: gate → compressor → de-esser → limiter → boost
            apply_gate(out_slice_r, &fx.gate, &mut d.env_r, SAMPLE_RATE);
            apply_compressor(out_slice_r, &fx.compressor, &mut d.env_r, SAMPLE_RATE);
            apply_de_esser(out_slice_r, &fx.de_esser);
            apply_limiter(out_slice_r, &fx.limiter);
            apply_boost(out_slice_r, fx.boost);
            // Volume gain
            if (vol_r - 1.0).abs() > f32::EPSILON {
                for s in out_slice_r.iter_mut() {
                    *s *= vol_r;
                }
            }
            peak_r = out_slice_r.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
        }
    }
    d.handle.store_peaks(peak_l, peak_r);
}
