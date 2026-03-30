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
use osg_core::commands::Command;

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

    let core = OsgCore::new().await?;

    let state = Arc::new(AppState { core });

    let app = Router::new()
        .route("/api/graph", get(get_graph))
        .route("/api/session", get(get_session))
        .route("/ws/graph", get(ws_graph))
        .route("/ws/session", get(ws_session))
        .route("/ws/commands", get(ws_commands))
        .route("/ws/levels", get(ws_levels))
        .fallback_service(ServeDir::new("web/dist"))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:9100")
        .await
        .map_err(|e| osg_core::pw::PwError::ConnectionFailed(format!("bind failed: {e}")))?;
    tracing::info!("Listening on http://127.0.0.1:9100");

    // Graceful shutdown: save state on Ctrl+C
    let shutdown_state = state;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::error!("Failed to listen for ctrl_c: {err}");
                return;
            }
            tracing::info!("Shutting down, saving state...");
            shutdown_state.core.reducer().save_and_exit();
        })
        .await
        .map_err(|e| osg_core::pw::PwError::ConnectionFailed(format!("serve failed: {e}")))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// REST endpoints
// ---------------------------------------------------------------------------

/// GET /api/graph — current AudioGraph as JSON (read model).
async fn get_graph(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let graph = state.core.snapshot();
    axum::Json(graph)
}

/// GET /api/session — current MixerSession as JSON (write model).
async fn get_session(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let session = state.core.reducer().state();
    axum::Json((*session).clone())
}

// ---------------------------------------------------------------------------
// WebSocket: graph (read model)
// ---------------------------------------------------------------------------

async fn ws_graph(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
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
                tracing::debug!("WebSocket graph client lagged by {n} messages, skipping");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket: session (write model)
// ---------------------------------------------------------------------------

async fn ws_session(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_session(socket, state))
}

async fn handle_ws_session(mut socket: ws::WebSocket, state: Arc<AppState>) {
    let mut rx = state.core.reducer().subscribe_state();

    // Send initial session state
    let session = state.core.reducer().state();
    if let Ok(json) = serde_json::to_string(&*session)
        && socket.send(ws::Message::Text(json.into())).await.is_err()
    {
        return;
    }

    // Stream session updates
    loop {
        if rx.changed().await.is_err() {
            break;
        }
        let session = rx.borrow_and_update().clone();
        if let Ok(json) = serde_json::to_string(&*session)
            && socket.send(ws::Message::Text(json.into())).await.is_err()
        {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket: commands (frontend → backend)
// ---------------------------------------------------------------------------

async fn ws_commands(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_commands(socket, state))
}

async fn handle_ws_commands(mut socket: ws::WebSocket, state: Arc<AppState>) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let ws::Message::Text(text) = msg {
            match serde_json::from_str::<Command>(&text) {
                Ok(cmd) => {
                    tracing::debug!("Command received: {cmd:?}");
                    state.core.command(cmd.into_state_msg());
                }
                Err(err) => {
                    tracing::warn!("Invalid command: {err}");
                    let reply = format!(r#"{{"error":"{err}"}}"#);
                    if socket.send(ws::Message::Text(reply.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket: peak levels (read-only, 25Hz broadcast)
// ---------------------------------------------------------------------------

async fn ws_levels(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_levels(socket, state))
}

async fn handle_ws_levels(mut socket: ws::WebSocket, state: Arc<AppState>) {
    let peak_store = state.core.peak_store().clone();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(40)); // 25 Hz

    // Auto-start peak monitors for all group nodes (channels/mixes)
    {
        let graph = state.core.snapshot();
        for (&_ulid, group_node) in &graph.group_nodes {
            if let Some(pw_id) = group_node.id {
                state.core.start_peak_monitor(pw_id);
            }
        }
    }

    loop {
        interval.tick().await;
        let levels = peak_store.snapshot();
        if levels.is_empty() {
            continue;
        }
        match serde_json::to_string(&levels) {
            Ok(json) => {
                if socket.send(ws::Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
