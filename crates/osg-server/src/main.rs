use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), osg_core::CoreError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Open Sound Grid server starting");

    // TODO: init osg-core, start PipeWire event loop
    // TODO: start Axum server with WebSocket endpoint

    Ok(())
}
