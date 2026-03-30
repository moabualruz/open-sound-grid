use std::sync::Arc;

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws},
    response::IntoResponse,
    routing::get,
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

use osg_core::OsgCore;

#[allow(missing_debug_implementations)]
struct AppState {
    core: OsgCore,
}

#[tokio::main]
async fn main() -> Result<(), osg_core::CoreError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Open Sound Grid server starting");

    let core = OsgCore::new()?;

    let state = Arc::new(AppState { core });

    let app = Router::new()
        .route("/api/graph", get(get_graph))
        .route("/ws/graph", get(ws_graph))
        .fallback_service(ServeDir::new("web/dist"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:9100")
        .await
        .map_err(|e| osg_core::pw::PwError::ConnectionFailed(format!("bind failed: {e}")))?;
    tracing::info!("Listening on http://127.0.0.1:9100");

    axum::serve(listener, app)
        .await
        .map_err(|e| osg_core::pw::PwError::ConnectionFailed(format!("serve failed: {e}")))?;

    Ok(())
}

/// REST endpoint: returns current AudioGraph as JSON.
async fn get_graph(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let graph = state.core.snapshot();
    axum::Json(graph)
}

/// WebSocket endpoint: sends AudioGraph snapshot on connect, then streams updates.
async fn ws_graph(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_graph(socket, state))
}

async fn handle_ws_graph(mut socket: ws::WebSocket, state: Arc<AppState>) {
    // Send initial snapshot
    let snapshot = state.core.snapshot();
    if let Ok(json) = serde_json::to_string(&snapshot)
        && socket.send(ws::Message::Text(json.into())).await.is_err()
    {
        return;
    }

    // Subscribe to graph updates and forward
    let mut rx = state.core.subscribe();
    loop {
        match rx.recv().await {
            Ok(graph) => {
                if let Ok(json) = serde_json::to_string(&graph)
                    && socket.send(ws::Message::Text(json.into())).await.is_err()
                {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("WebSocket client lagged by {n} messages, skipping");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}
