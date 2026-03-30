pub mod config;
pub mod graph;
pub mod pw;
pub mod routing;

use thiserror::Error;

/// Top-level error type for osg-core, wrapping all module errors.
#[derive(Error, Debug)]
pub enum CoreError {
    #[error(transparent)]
    Pw(#[from] pw::PwError),

    #[error(transparent)]
    Config(#[from] config::ConfigError),

    #[error(transparent)]
    Routing(#[from] routing::RoutingError),
}
