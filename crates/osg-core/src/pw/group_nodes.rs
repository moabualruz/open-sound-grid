// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// PipeWire listener initialization and setup helpers extracted from mainloop.rs.

use std::cell::RefCell;
use std::rc::Rc;

use pipewire::{
    context::ContextRc, core::CoreRc, keys::*, main_loop::MainLoopRc, metadata::Metadata,
    properties::properties, registry::RegistryRc, spa::param::ParamType,
};
use tracing::{debug, warn};

use super::{OSG_APP_ID, PwError, ToPipewireMessage, store::Store};

pub fn init_node_listeners(
    store: Rc<RefCell<Store>>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    id: u32,
) {
    if let Some(node) = store.clone().borrow_mut().nodes.get_mut(&id) {
        node.listener = Some(
            node.proxy
                .add_listener_local()
                .info({
                    let store = store.clone();
                    let sender = sender.clone();
                    move |info| {
                        store.borrow_mut().update_node_info(info);
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                })
                .param({
                    move |_, type_, _, _, pod| {
                        let Ok(mut store_borrow) = store.try_borrow_mut() else {
                            warn!(
                                "[PW] re-entrant borrow in param callback for node {id}, skipping"
                            );
                            return;
                        };
                        store_borrow.update_node_param(type_, id, pod);
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                })
                .register(),
        );
        node.proxy
            .enum_params(0, Some(ParamType::Props), 0, u32::MAX);
        node.proxy.subscribe_params(&[ParamType::Props]);
    }
}

pub fn init_device_listeners(store: Rc<RefCell<Store>>, id: u32) {
    if let Some(device) = store.clone().borrow_mut().devices.get_mut(&id) {
        device.listener = Some(
            device
                .proxy
                .add_listener_local()
                .param({
                    move |_seq, type_, index, _next, pod| {
                        let Ok(mut store_borrow) = store.try_borrow_mut() else {
                            warn!(
                                "[PW] re-entrant borrow in param callback for device {id}, skipping"
                            );
                            return;
                        };
                        store_borrow.update_device_param(type_, id, index, pod);
                    }
                })
                .register(),
        );
        device
            .proxy
            .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
        device.proxy.subscribe_params(&[ParamType::Route]);
    }
}

/// Bind and listen for PipeWire metadata `default.audio.sink` changes.
/// Stores the metadata proxy in `metadata_out` so it can be used to set the default sink.
#[allow(clippy::too_many_arguments)]
pub fn init_metadata_listener(
    registry: &RegistryRc,
    store: Rc<RefCell<Store>>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    metadata_out: &Rc<RefCell<Option<Metadata>>>,
    metadata_listeners: &Rc<RefCell<Vec<pipewire::metadata::MetadataListener>>>,
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
) {
    // Only bind the "default" metadata — it has default.audio.sink/source
    let is_default = global
        .props
        .map(|p| p.get("metadata.name") == Some("default"))
        .unwrap_or(false);
    if !is_default {
        return;
    }

    let Ok(metadata) = registry.bind::<Metadata, _>(global) else {
        warn!("[PW] failed to bind metadata object {}", global.id);
        return;
    };
    debug!("[PW] bound 'default' metadata object (id={})", global.id);
    let listener = metadata
        .add_listener_local()
        .property({
            let store = store.clone();
            let sender = sender.clone();
            move |_subject, key, _type, value| {
                let parse_name = |v: &str| -> Option<String> {
                    serde_json::from_str::<serde_json::Value>(v)
                        .ok()
                        .and_then(|v| v.get("name")?.as_str().map(String::from))
                };
                match key {
                    Some("default.audio.sink") => {
                        let name = value.and_then(parse_name);
                        debug!("default.audio.sink changed: {name:?}");
                        store.borrow_mut().default_sink_name = name;
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                    Some("default.audio.source") => {
                        let name = value.and_then(parse_name);
                        debug!("default.audio.source changed: {name:?}");
                        store.borrow_mut().default_source_name = name;
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                    _ => {}
                }
                0
            }
        })
        .register();

    // Store the proxy so we can call set_property later.
    // Keep the listener alive in the vec — dropping it would stop events.
    *metadata_out.borrow_mut() = Some(metadata);
    metadata_listeners.borrow_mut().push(listener);
}

/// Initialize PipeWire mainloop, context, core, and registry.
pub fn init_pipewire() -> Result<(MainLoopRc, ContextRc, CoreRc, RegistryRc), PwError> {
    let mainloop = MainLoopRc::new(None)
        .map_err(|e| PwError::ConnectionFailed(format!("mainloop init: {e}")))?;
    let context = ContextRc::new(&mainloop, None)
        .map_err(|e| PwError::ConnectionFailed(format!("context init: {e}")))?;
    let pw_core = context
        .connect_rc(Some(properties! {
            *MEDIA_CATEGORY => "Manager",
            *APP_ICON_NAME => OSG_APP_ID,
        }))
        .map_err(|e| PwError::ConnectionFailed(format!("core connect: {e}")))?;
    let registry = pw_core
        .get_registry_rc()
        .map_err(|e| PwError::ConnectionFailed(format!("registry: {e}")))?;
    Ok((mainloop, context, pw_core, registry))
}
