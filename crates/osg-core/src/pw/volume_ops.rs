// Volume and mute write operations on PipeWire nodes/devices.
// Standalone functions taking &Store, extracted from Store methods to keep
// store.rs focused on registry mirroring (add/remove/update/dump).

use pipewire::spa::param::ParamType;

use super::{
    PwError,
    object::EndpointId,
    pod::{build_node_mute_pod, build_node_volume_pod},
    store::Store,
};

/// Set per-channel volumes on a node or its parent device route.
pub(super) fn set_node_volume(
    store: &Store,
    id: u32,
    channel_volumes: Vec<f32>,
) -> Result<(), PwError> {
    let node = store.nodes.get(&id).ok_or(PwError::NodeNotFound(id))?;

    if let EndpointId::Device {
        id: device_id,
        device_index,
    } = node.endpoint
    {
        let device_index = device_index.ok_or(PwError::MissingDeviceIndex(id))?;
        let device = store
            .devices
            .get(&device_id)
            .ok_or(PwError::DeviceNotFound(device_id))?;
        let route = device
            .active_routes
            .iter()
            .find(|route| route.device_index == device_index)
            .ok_or(PwError::RouteNotFound {
                device_id,
                device_index,
            })?;
        let (param_type, pod) = route.build_device_volume_pod(channel_volumes);
        device.proxy.set_param(param_type, 0, pod.pod());
    } else {
        let (param_type, pod) = build_node_volume_pod(channel_volumes);
        node.proxy.set_param(param_type, 0, pod.pod());
        node.proxy.enum_params(7, Some(ParamType::Props), 0, 1);
    }
    Ok(())
}

/// Set mute state on a node or its parent device route.
pub(super) fn set_node_mute(store: &Store, id: u32, mute: bool) -> Result<(), PwError> {
    let node = store.nodes.get(&id).ok_or(PwError::NodeNotFound(id))?;

    if let EndpointId::Device {
        id: device_id,
        device_index,
    } = node.endpoint
    {
        let device_index = device_index.ok_or(PwError::MissingDeviceIndex(id))?;
        let device = store
            .devices
            .get(&device_id)
            .ok_or(PwError::DeviceNotFound(device_id))?;
        let route = device
            .active_routes
            .iter()
            .find(|route| route.device_index == device_index)
            .ok_or(PwError::RouteNotFound {
                device_id,
                device_index,
            })?;
        let (param_type, pod) = route.build_device_mute_pod(mute);
        device.proxy.set_param(param_type, 0, pod.pod());
    } else {
        let (param_type, pod) = build_node_mute_pod(mute);
        node.proxy.set_param(param_type, 0, pod.pod());
    }
    Ok(())
}
