//! Safe wrapper around `pw_filter` for inline DSP processing.
//!
//! Thread model:
//! - `OsgFilter` is created on the PW mainloop thread.
//! - The `process` callback runs on the PW real-time thread.
//! - EQ params: main → RT via `ArcSwap` (lock-free).
//! - Peak levels: RT → main via packed `AtomicU64`.
#![allow(unsafe_op_in_unsafe_fn)]

pub mod filter_handle;
pub mod process;

// Re-export everything from filter_handle so existing paths (super::filter::X) still resolve.
pub(crate) use filter_handle::EnvelopeState;
pub use filter_handle::{
    CompiledEq, CompressorParams, DeEsserParams, EffectsParams, FilterHandle, GateParams,
    LimiterParams, MAX_MACRO_BANDS, SmartVolumeParams, SpatialAudioParams, pack_peaks,
    unpack_peaks,
};
pub use process::process_block;

use crate::pw::biquad::BiquadState;
use filter_handle::MAX_BANDS;
use process::on_process;

/// Data passed to the PW process callback via the `data` pointer.
/// Lives on the heap, leaked via `Box::into_raw`, reclaimed on drop.
pub(super) struct CallbackData {
    pub(super) handle: FilterHandle,
    pub(super) states_l: Vec<BiquadState>,
    pub(super) states_r: Vec<BiquadState>,
    pub(super) env_l: EnvelopeState,
    pub(super) env_r: EnvelopeState,
    pub(super) in_port_l: *mut std::os::raw::c_void,
    pub(super) in_port_r: *mut std::os::raw::c_void,
    pub(super) out_port_l: *mut std::os::raw::c_void,
    pub(super) out_port_r: *mut std::os::raw::c_void,
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
    /// # Safety
    /// Must be called from the PW mainloop thread. The `core_ptr` must
    /// be a valid `*mut pw_core` from the running PW connection.
    #[allow(unsafe_code, clippy::too_many_lines)]
    pub unsafe fn new(
        core_ptr: *mut pipewire_sys::pw_core,
        name: &str,
        description: &str,
    ) -> Result<Self, String> {
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

        let filter = pipewire_sys::pw_filter_new(core_ptr, c_name.as_ptr(), props);

        if filter.is_null() {
            drop(Box::from_raw(data));
            return Err("pw_filter_new returned null".into());
        }

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

        let in_port_l = pipewire_sys::pw_filter_add_port(
            filter,
            libspa_sys::SPA_DIRECTION_INPUT,
            pipewire_sys::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            std::mem::size_of::<*mut std::os::raw::c_void>(),
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
        unsafe {
            pipewire_sys::pw_filter_destroy(self.filter);
            drop(Box::from_raw(self.data));
        }
    }
}
