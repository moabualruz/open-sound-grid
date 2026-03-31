//! Filter-chain EQ nodes via `libpipewire-module-filter-chain`.
//!
//! Each channel/cell gets a filter-chain module instance that creates:
//! - A capture node (`Audio/Sink` or `Audio/Duplex`) — apps route to this
//! - A chain of builtin biquad EQ bands
//! - A playback node (`node.passive = true`) — connects downstream
//!
//! The module is loaded via `pw_context_load_module` at runtime.
//! To update EQ params: unload + reload with new config string.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::CString;
use std::os::raw::c_void;

use crate::graph::{EqBand, EqConfig, FilterType};

/// Maps our FilterType to PipeWire filter-chain builtin label names.
fn filter_label(ft: FilterType) -> &'static str {
    match ft {
        FilterType::Peaking => "bq_peaking",
        FilterType::LowShelf => "bq_lowshelf",
        FilterType::HighShelf => "bq_highshelf",
        FilterType::LowPass => "bq_lowpass",
        FilterType::HighPass => "bq_highpass",
        FilterType::Notch => "bq_notch",
    }
}

/// Build the filter-chain args string from an EqConfig.
/// If no bands are enabled, returns a passthrough (copy builtin).
pub fn build_filter_chain_args(
    node_name: &str,
    node_description: &str,
    media_class: &str,
    eq: &EqConfig,
) -> String {
    let enabled_bands: Vec<&EqBand> = if eq.enabled {
        eq.bands.iter().filter(|b| b.enabled).collect()
    } else {
        Vec::new()
    };

    let mut nodes = String::new();
    let mut links = String::new();

    if enabled_bands.is_empty() {
        // Passthrough — single copy node
        nodes.push_str(
            "{ type = builtin name = passthrough label = copy }",
        );
    } else {
        // Build biquad chain
        for (i, band) in enabled_bands.iter().enumerate() {
            let name = format!("eq_{i}");
            let label = filter_label(band.filter_type);
            nodes.push_str(&format!(
                "{{ type = builtin name = {name} label = {label} \
                 control = {{ \"Freq\" = {freq:.1} \"Q\" = {q:.3} \"Gain\" = {gain:.1} }} }}\n",
                freq = band.frequency,
                q = band.q,
                gain = band.gain,
            ));

            if i > 0 {
                let prev = format!("eq_{}", i - 1);
                links.push_str(&format!(
                    "{{ output = \"{prev}:Out\" input = \"{name}:In\" }}\n"
                ));
            }
        }
    }

    format!(
        r#"{{
    node.description = "{node_description}"
    media.name       = "{node_description}"
    filter.graph = {{
        nodes = [
            {nodes}
        ]
        links = [
            {links}
        ]
    }}
    audio.channels = 2
    audio.position = [ FL FR ]
    capture.props = {{
        node.name   = "{node_name}"
        media.class = {media_class}
        node.virtual = true
    }}
    playback.props = {{
        node.name   = "{node_name}.out"
        node.passive = true
    }}
}}"#
    )
}

/// A loaded filter-chain module instance. Dropping it unloads the module.
pub struct EqFilterChain {
    module: *mut pipewire_sys::pw_impl_module,
    context: *mut pipewire_sys::pw_context,
    node_name: String,
}

impl std::fmt::Debug for EqFilterChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EqFilterChain({})", self.node_name)
    }
}

impl EqFilterChain {
    /// Load a filter-chain module with the given EQ configuration.
    ///
    /// # Safety
    /// Must be called from the PW mainloop thread. `context` must be valid.
    #[allow(unsafe_code)]
    pub unsafe fn load(
        context: *mut pipewire_sys::pw_context,
        node_name: &str,
        node_description: &str,
        media_class: &str,
        eq: &EqConfig,
    ) -> Result<Self, String> {
        let args_str = build_filter_chain_args(node_name, node_description, media_class, eq);
        let c_module_name =
            CString::new("libpipewire-module-filter-chain").map_err(|e| e.to_string())?;
        let c_args = CString::new(args_str).map_err(|e| e.to_string())?;

        let module = pipewire_sys::pw_context_load_module(
            context,
            c_module_name.as_ptr(),
            c_args.as_ptr(),
            std::ptr::null_mut(), // no extra properties
        );

        if module.is_null() {
            return Err(format!("pw_context_load_module returned null for '{node_name}'"));
        }

        tracing::debug!("[PW] loaded filter-chain '{node_description}' as {node_name}");

        Ok(Self {
            module,
            context,
            node_name: node_name.to_owned(),
        })
    }

    /// Reload the filter-chain with updated EQ parameters.
    /// Unloads the old module and loads a new one.
    ///
    /// # Safety
    /// Must be called from the PW mainloop thread.
    #[allow(unsafe_code)]
    pub unsafe fn update_eq(
        &mut self,
        node_description: &str,
        media_class: &str,
        eq: &EqConfig,
    ) -> Result<(), String> {
        // Unload old
        pipewire_sys::pw_impl_module_destroy(self.module);

        // Load new with updated config
        let args_str =
            build_filter_chain_args(&self.node_name, node_description, media_class, eq);
        let c_module_name =
            CString::new("libpipewire-module-filter-chain").map_err(|e| e.to_string())?;
        let c_args = CString::new(args_str).map_err(|e| e.to_string())?;

        let module = pipewire_sys::pw_context_load_module(
            self.context,
            c_module_name.as_ptr(),
            c_args.as_ptr(),
            std::ptr::null_mut(),
        );

        if module.is_null() {
            return Err(format!(
                "pw_context_load_module returned null on reload for '{}'",
                self.node_name
            ));
        }

        self.module = module;
        tracing::debug!("[PW] reloaded filter-chain '{}'", self.node_name);
        Ok(())
    }

    pub fn node_name(&self) -> &str {
        &self.node_name
    }
}

impl Drop for EqFilterChain {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            pipewire_sys::pw_impl_module_destroy(self.module);
        }
        tracing::debug!("[PW] unloaded filter-chain '{}'", self.node_name);
    }
}

// Keep the old filter.rs exports available for effects processing (compressor, etc.)
// that filter-chain doesn't support. Those will use pw_filter directly.
