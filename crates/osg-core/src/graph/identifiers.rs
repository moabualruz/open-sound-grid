// Opaque identifier newtypes for domain entities.

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Opaque ID for a persistent (name-matched) node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PersistentNodeId(Ulid);

#[allow(clippy::new_without_default)]
impl PersistentNodeId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn inner(&self) -> Ulid {
        self.0
    }
}

/// Unique ID for a Channel. PipeWire: GroupNode ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelId(Ulid);

#[allow(clippy::new_without_default)]
impl ChannelId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn inner(&self) -> Ulid {
        self.0
    }
}

/// Unique ID for a detected audio app. PipeWire: Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppId(Ulid);

#[allow(clippy::new_without_default)]
impl AppId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

/// Opaque ID for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(Ulid);
