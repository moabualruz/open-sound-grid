// Domain-owned port direction. Infrastructure layers convert to/from this.

use serde::{Deserialize, Serialize};

/// Whether a port carries audio in (Sink) or out (Source).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortKind {
    Source,
    Sink,
}
