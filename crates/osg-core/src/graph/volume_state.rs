// Volume lock / mute state machine.

use serde::{Deserialize, Serialize};

use super::utils::aggregate_bools;

/// Encodes the combined lock + mute state for an endpoint.
///
/// When an endpoint backs multiple PipeWire nodes, some may be muted while
/// others are not ("MuteMixed"). A user cannot input this state and cannot
/// lock volume while in it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeLockMuteState {
    MuteMixed,
    MutedLocked,
    MutedUnlocked,
    UnmutedLocked,
    #[default]
    UnmutedUnlocked,
}

impl VolumeLockMuteState {
    pub fn is_locked(self) -> bool {
        matches!(self, Self::MutedLocked | Self::UnmutedLocked)
    }

    pub fn is_muted(self) -> Option<bool> {
        match self {
            Self::MuteMixed => None,
            Self::MutedLocked | Self::MutedUnlocked => Some(true),
            Self::UnmutedLocked | Self::UnmutedUnlocked => Some(false),
        }
    }

    pub fn with_mute(self, muted: bool) -> Self {
        match (muted, self) {
            (true, Self::MutedLocked | Self::UnmutedLocked) => Self::MutedLocked,
            (true, Self::MuteMixed | Self::MutedUnlocked | Self::UnmutedUnlocked) => {
                Self::MutedUnlocked
            }
            (false, Self::MutedLocked | Self::UnmutedLocked) => Self::UnmutedLocked,
            (false, Self::MuteMixed | Self::MutedUnlocked | Self::UnmutedUnlocked) => {
                Self::UnmutedUnlocked
            }
        }
    }

    pub fn lock(self) -> Option<Self> {
        match self {
            Self::MuteMixed => None,
            Self::MutedLocked | Self::MutedUnlocked => Some(Self::MutedLocked),
            Self::UnmutedLocked | Self::UnmutedUnlocked => Some(Self::UnmutedLocked),
        }
    }

    pub fn unlock(self) -> Self {
        match self {
            Self::MuteMixed => Self::MuteMixed,
            Self::MutedLocked | Self::MutedUnlocked => Self::MutedUnlocked,
            Self::UnmutedLocked | Self::UnmutedUnlocked => Self::UnmutedUnlocked,
        }
    }

    /// Build the state from multiple node mute booleans (unlocked).
    pub fn from_bools_unlocked<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Self {
        match aggregate_bools(bools) {
            Some(true) => Self::MutedUnlocked,
            Some(false) => Self::UnmutedUnlocked,
            None => Self::MuteMixed,
        }
    }
}
