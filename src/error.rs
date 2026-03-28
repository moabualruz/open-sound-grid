use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum OsgError {
    #[error("PulseAudio error: {0}")]
    PulseAudio(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(u32),

    #[error("Mix not found: {0}")]
    MixNotFound(u32),

    #[error("Output device not found: {0}")]
    OutputNotFound(String),

    #[error("Module load failed: {0}")]
    ModuleLoadFailed(String),

    #[error("Already running")]
    AlreadyRunning,
}

pub type Result<T> = std::result::Result<T, OsgError>;
