use thiserror::Error;

/// Errors originating from the PipeWire backend.
#[derive(Error, Debug)]
pub enum PwError {
    #[error("failed to connect to PipeWire: {0}")]
    ConnectionFailed(String),

    #[error("node {0} not found")]
    NodeNotFound(u32),

    #[error("port {0} not found")]
    PortNotFound(u32),

    #[error("device {0} not found")]
    DeviceNotFound(u32),

    #[error("no active route found on device {device_id} with device index {device_index}")]
    RouteNotFound { device_id: u32, device_index: i32 },

    #[error("failed to create sink: {0}")]
    SinkCreationFailed(String),

    #[error("failed to create link: {0}")]
    LinkCreationFailed(String),

    #[error("invalid port: {0}")]
    InvalidPort(String),

    #[error("no port pairs to connect between nodes {start_id} and {end_id}")]
    NoPortPairs { start_id: u32, end_id: u32 },

    #[error("group node with id '{0}' does not exist")]
    GroupNodeNotFound(ulid::Ulid),

    #[error("node {0} is missing device index")]
    MissingDeviceIndex(u32),

    #[error("PipeWire thread exited unexpectedly")]
    ThreadExited,
}
