use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws},
    response::IntoResponse,
    routing::get,
};
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

use osg_core::OsgCore;
use osg_core::commands::Command;
use osg_server::spectrum::SpectrumMessage;

mod icons;

#[allow(missing_debug_implementations)]
struct AppState {
    core: OsgCore,
    icon_cache: icons::IconCache,
    spectrum_subscribers: AtomicUsize,
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), osg_core::CoreError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Open Sound Grid server starting");

    // Server configuration from environment variables
    let host = std::env::var("OSG_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("OSG_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9100);
    let dev_port: u16 = std::env::var("OSG_DEV_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5173);
    let bind_addr = format!("{host}:{port}");

    let core = OsgCore::new().await?;

    let state = Arc::new(AppState {
        core,
        icon_cache: icons::IconCache::new(),
        spectrum_subscribers: AtomicUsize::new(0),
    });

    // Build CORS origins from configured host/ports
    let cors_origins: Vec<axum::http::HeaderValue> = [
        format!("http://{host}:{port}"),
        format!("http://localhost:{dev_port}"),
    ]
    .iter()
    .filter_map(|o| o.parse().ok())
    .collect();

    let app = Router::new()
        .route("/api/graph", get(get_graph))
        .route("/api/session", get(get_session))
        .route("/api/icons/{app_name}", get(get_icon))
        .route("/ws/graph", get(ws_graph))
        .route("/ws/session", get(ws_session))
        .route("/ws/commands", get(ws_commands))
        .route("/ws/levels", get(ws_levels))
        .route("/ws/spectrum", get(ws_spectrum))
        .fallback_service(ServeDir::new("web/dist"))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(cors_origins))
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state.clone());

    let listener = TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| osg_core::pw::PwError::ServerError(format!("bind failed: {e}")))?;
    tracing::info!("Listening on http://{bind_addr}");

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
        .map_err(|e| osg_core::pw::PwError::ServerError(format!("serve failed: {e}")))?;

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

/// GET /api/icons/:app_name — resolve and serve the icon for an application.
async fn get_icon(
    axum::extract::Path(app_name): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    icons::serve_icon(&state.icon_cache, &app_name)
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

    // Rate-limited session broadcast (30 fps). Buffer the latest state change
    // and send on tick to avoid flooding the frontend during rapid mutations
    // (e.g., volume slider drag producing 60+ updates/s).
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(33));
    let mut pending = false;
    loop {
        tokio::select! {
            result = rx.changed() => {
                if result.is_err() {
                    break;
                }
                pending = true;
            }
            _ = interval.tick(), if pending => {
                let session = rx.borrow_and_update().clone();
                if let Ok(json) = serde_json::to_string(&*session)
                    && socket.send(ws::Message::Text(json.into())).await.is_err()
                {
                    break;
                }
                pending = false;
            }
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

    // Auto-start peak monitors for all audio nodes (channels, mixes, and app streams)
    // Peak streams disabled on initial connect — app streams may not
    // exist yet. The reconciler handles peak monitoring for app streams
    // as they appear. We do NOT monitor hardware or osg.* nodes.
    {}

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

// ---------------------------------------------------------------------------
// WebSocket: FFT spectrum (read-only, 15fps broadcast)
// ---------------------------------------------------------------------------

async fn ws_spectrum(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_spectrum(socket, state))
}

async fn handle_ws_spectrum(mut socket: ws::WebSocket, state: Arc<AppState>) {
    let filter_store = state.core.filter_store().clone();
    let previous = state.spectrum_subscribers.fetch_add(1, Ordering::AcqRel);
    if previous == 0 {
        filter_store.set_spectrum_enabled_for_all(true);
    }

    let mut interval = tokio::time::interval(std::time::Duration::from_nanos(66_666_667));

    loop {
        tokio::select! {
            maybe_msg = socket.recv() => match maybe_msg {
                Some(Ok(ws::Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
            _ = interval.tick() => {
                let spectra = filter_store.read_all_spectra();
                let mut send_failed = false;
                for (node_id, spectrum) in spectra {
                    let payload = SpectrumMessage::new(node_id, spectrum.bins);
                    let Ok(json) = serde_json::to_string(&payload) else {
                        continue;
                    };
                    if socket.send(ws::Message::Text(json.into())).await.is_err() {
                        send_failed = true;
                        break;
                    }
                }
                if send_failed {
                    break;
                }
            }
        }
    }

    let remaining = state.spectrum_subscribers.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        filter_store.set_spectrum_enabled_for_all(false);
    }
}
