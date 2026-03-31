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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::graph::EqConfig;
use crate::pw::biquad::{BiquadState, Coefficients, compute_coefficients};

const SAMPLE_RATE: f32 = 48000.0;
const MAX_BANDS: usize = 10;

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

/// Shared handle for lock-free EQ parameter passing and peak reading.
#[derive(Clone)]
pub struct FilterHandle {
    eq: Arc<ArcSwap<CompiledEq>>,
    peaks: Arc<AtomicU64>,
}

impl FilterHandle {
    pub fn new() -> Self {
        Self {
            eq: Arc::new(ArcSwap::new(Arc::new(CompiledEq::empty()))),
            peaks: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Update EQ parameters (lock-free, any thread).
    pub fn set_eq(&self, config: &EqConfig) {
        self.eq.store(Arc::new(CompiledEq::from_config(config)));
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
            if *enabled {
                if let Some(state) = states.get_mut(band_idx) {
                    s = state.process(s, coeffs);
                }
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

// ---------------------------------------------------------------------------
// OsgFilter — the actual pw_filter wrapper (requires PW mainloop thread)
// ---------------------------------------------------------------------------

/// Data passed to the PW process callback via the `data` pointer.
/// Lives on the heap, leaked via `Box::into_raw`, reclaimed on drop.
struct CallbackData {
    handle: FilterHandle,
    states_l: Vec<BiquadState>,
    states_r: Vec<BiquadState>,
    in_port_l: *mut std::os::raw::c_void,
    in_port_r: *mut std::os::raw::c_void,
    out_port_l: *mut std::os::raw::c_void,
    out_port_r: *mut std::os::raw::c_void,
}

/// A PipeWire filter node for inline stereo DSP.
/// Must be created and used on the PW mainloop thread.
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
    #[allow(unsafe_code)]
    pub unsafe fn new(
        core_ptr: *mut pipewire_sys::pw_core,
        name: &str,
        description: &str,
        media_class: &str,
    ) -> Result<Self, String> {
        use std::ffi::CString;
        use std::ptr;

        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let c_desc = CString::new(description).map_err(|e| e.to_string())?;
        let c_class = CString::new(media_class).map_err(|e| e.to_string())?;

        let handle = FilterHandle::new();
        let data = Box::into_raw(Box::new(CallbackData {
            handle: handle.clone(),
            states_l: vec![BiquadState::default(); MAX_BANDS],
            states_r: vec![BiquadState::default(); MAX_BANDS],
            in_port_l: ptr::null_mut(),
            in_port_r: ptr::null_mut(),
            out_port_l: ptr::null_mut(),
            out_port_r: ptr::null_mut(),
        }));

        // Build properties — must match what null-audio-sink used so
        // WirePlumber treats this as a routable audio node, not a DSP filter.
        let props = pipewire_sys::pw_properties_new(
            c"media.type".as_ptr().cast::<std::os::raw::c_char>(),
            c"Audio".as_ptr().cast::<std::os::raw::c_char>(),
            c"media.class".as_ptr().cast::<std::os::raw::c_char>(),
            c_class.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.name".as_ptr().cast::<std::os::raw::c_char>(),
            c_name.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.nick".as_ptr().cast::<std::os::raw::c_char>(),
            c_desc.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.description".as_ptr().cast::<std::os::raw::c_char>(),
            c_desc.as_ptr().cast::<std::os::raw::c_char>(),
            c"node.virtual".as_ptr().cast::<std::os::raw::c_char>(),
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
                c"32 bit float mono audio".as_ptr().cast::<std::os::raw::c_char>(),
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
                c"32 bit float mono audio".as_ptr().cast::<std::os::raw::c_char>(),
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
                c"32 bit float mono audio".as_ptr().cast::<std::os::raw::c_char>(),
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
                c"32 bit float mono audio".as_ptr().cast::<std::os::raw::c_char>(),
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
#[allow(unsafe_code)]
pub fn create_group_filter(
    core_ptr: *mut pipewire_sys::pw_core,
    name: &str,
    id: ulid::Ulid,
    kind: super::GroupNodeKind,
) -> Result<OsgFilter, String> {
    let node_name = format!("osg.group.{id}");
    let media_class = match kind {
        super::GroupNodeKind::Source => "Audio/Source/Virtual",
        super::GroupNodeKind::Duplex => "Audio/Duplex",
        super::GroupNodeKind::Sink => "Audio/Sink",
    };
    let filter = unsafe { OsgFilter::new(core_ptr, &node_name, name, media_class) }
        .map_err(|e| format!("filter '{name}': {e}"))?;
    tracing::debug!(
        "[PW] created filter '{}' ({:?}) — node_id: {:?}",
        name, kind, filter.node_id()
    );
    Ok(filter)
}

impl Drop for OsgFilter {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
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
/// Reads stereo input, applies EQ cascade, computes peaks, writes output.
#[allow(unsafe_code)]
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

    // Always write output — silence if no input, to prevent graph stalls.
    // Output buffers may be NULL if ports aren't connected yet.
    if !out_l.is_null() {
        let out_slice_l = std::slice::from_raw_parts_mut(out_l, n);
        if !in_l.is_null() {
            let in_slice_l = std::slice::from_raw_parts(in_l, n);
            let eq = d.handle.load_eq();
            let peak_l = process_block(in_slice_l, out_slice_l, &eq, &mut d.states_l);
            d.handle.store_peaks(peak_l, 0.0);
        } else {
            out_slice_l.fill(0.0);
        }
    }
    if !out_r.is_null() {
        let out_slice_r = std::slice::from_raw_parts_mut(out_r, n);
        if !in_r.is_null() {
            let in_slice_r = std::slice::from_raw_parts(in_r, n);
            let eq = d.handle.load_eq();
            let peak_r = process_block(in_slice_r, out_slice_r, &eq, &mut d.states_r);
            let (existing_l, _) = d.handle.peak();
            d.handle.store_peaks(existing_l, peak_r);
        } else {
            out_slice_r.fill(0.0);
        }
    }
}
