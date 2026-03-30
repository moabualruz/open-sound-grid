// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The reducer owns the desired state and processes messages on a dedicated
// thread. Graph updates from PipeWire are debounced (16ms ≈ 1/60s) before
// triggering a reconciliation pass.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, broadcast, mpsc, watch};
use tracing::{error, warn};

use crate::config::{PersistentSettings, PersistentState};
use crate::graph::{MixerSession, ReconcileSettings};
use crate::pw::{AudioGraph, ToPipewireMessage};
use crate::routing::RoutingError;
use crate::routing::messages::{ReducerMsg, StateMsg, StateOutputMsg};

/// Debounce interval for PipeWire graph updates (≈60 Hz).
const GRAPH_UPDATE_DEBOUNCE: Duration = Duration::from_millis(16);

// ---------------------------------------------------------------------------
// Public handle
// ---------------------------------------------------------------------------

/// The public handle to the reducer. Cheap to clone.
#[derive(Clone)]
#[allow(missing_debug_implementations)] // Contains channel senders which are not Debug
pub struct ReducerHandle {
    msg_tx: mpsc::UnboundedSender<ReducerMsg>,
    state_rx: watch::Receiver<Arc<MixerSession>>,
    output_tx: broadcast::Sender<StateOutputMsg>,
}

impl ReducerHandle {
    /// Send a state-mutation message.
    pub fn emit(&self, msg: StateMsg) {
        let _ = self.msg_tx.send(ReducerMsg::Update(msg));
    }

    /// Get a snapshot of the current desired state.
    pub fn state(&self) -> Arc<MixerSession> {
        self.state_rx.borrow().clone()
    }

    /// Subscribe to state changes (watch channel).
    pub fn subscribe_state(&self) -> watch::Receiver<Arc<MixerSession>> {
        self.state_rx.clone()
    }

    /// Subscribe to output messages (broadcast channel).
    pub fn subscribe_output(&self) -> broadcast::Receiver<StateOutputMsg> {
        self.output_tx.subscribe()
    }

    /// Request a save. If `clear_state` is true the state is reset first.
    pub fn save(&self, clear_state: bool, clear_settings: bool) {
        let _ = self.msg_tx.send(ReducerMsg::Save {
            clear_state,
            clear_settings,
        });
    }

    /// Save and shut down the reducer loop.
    pub fn save_and_exit(&self) {
        let _ = self.msg_tx.send(ReducerMsg::SaveAndExit);
    }

    /// Notify the reducer that settings changed.
    pub fn notify_settings_changed(&self) {
        let _ = self.msg_tx.send(ReducerMsg::SettingsChanged);
    }
}

// ---------------------------------------------------------------------------
// Graph update debouncer
// ---------------------------------------------------------------------------

/// Returns a closure that, when called with a new `Graph`, debounces
/// by `GRAPH_UPDATE_DEBOUNCE` and then sends a `ReducerMsg::GraphUpdate`.
pub fn debounced_graph_sender(
    msg_tx: mpsc::UnboundedSender<ReducerMsg>,
) -> impl Fn(Box<AudioGraph>) + Send + 'static {
    let pending: Arc<Mutex<Option<Box<AudioGraph>>>> = Arc::new(Mutex::new(None));
    // Capture the Tokio runtime handle so this closure works from non-Tokio threads
    // (e.g. the PipeWire std::thread callback).
    let handle = tokio::runtime::Handle::current();

    move |new_graph| {
        let pending = pending.clone();
        let tx = msg_tx.clone();

        handle.spawn(async move {
            let mut guard = pending.lock().await;
            if guard.is_some() {
                // A debounce timer is already running; just replace the graph.
                *guard = Some(new_graph);
            } else {
                *guard = Some(new_graph);
                drop(guard);

                tokio::time::sleep(GRAPH_UPDATE_DEBOUNCE).await;

                let mut guard = pending.lock().await;
                if let Some(graph) = guard.take() {
                    let _ = tx.send(ReducerMsg::GraphUpdate(graph));
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Reducer event loop
// ---------------------------------------------------------------------------

/// Initialize and run the reducer. Returns a `ReducerHandle` for interaction.
///
/// `pw_sender` is used to push corrective commands to PipeWire.
/// `settings_rx` receives updated `ReconcileSettings` from the config layer.
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub async fn run_reducer(
    pw_sender: std::sync::mpsc::Sender<ToPipewireMessage>,
    initial_settings: ReconcileSettings,
) -> Result<(ReducerHandle, mpsc::UnboundedSender<ReducerMsg>), RoutingError> {
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<ReducerMsg>();

    // Load persisted state, falling back to defaults.
    let initial_state = match PersistentState::load() {
        Ok(ps) => ps.into_state(),
        Err(err) => {
            warn!("[Reducer] failed to load persistent state: {err:#}");
            MixerSession::default()
        }
    };

    let (state_tx, state_rx) = watch::channel(Arc::new(initial_state));
    let (output_tx, _) = broadcast::channel::<StateOutputMsg>(64);

    let handle = ReducerHandle {
        msg_tx: msg_tx.clone(),
        state_rx,
        output_tx: output_tx.clone(),
    };

    let settings = Arc::new(tokio::sync::RwLock::new(initial_settings));

    // Spawn the reducer loop on a blocking-friendly task.
    let settings_clone = settings.clone();
    tokio::spawn(async move {
        let mut graph: Box<AudioGraph> = Box::default();
        let settings = settings_clone;

        let save = |state: &MixerSession, s: &ReconcileSettings| {
            let ps = PersistentState::from_state(state.clone());
            if let Err(err) = ps.save() {
                warn!("[Reducer] save state error: {err:#}");
            }
            let ps = PersistentSettings::from_settings(s.clone());
            if let Err(err) = ps.save() {
                warn!("[Reducer] save settings error: {err:#}");
            }
        };

        // Auto-save timer: saves 3s after last mutation
        let save_interval = tokio::time::Duration::from_secs(3);
        let mut save_deadline: Option<tokio::time::Instant> = None;

        loop {
            let message = if let Some(deadline) = save_deadline {
                match tokio::time::timeout_at(deadline, msg_rx.recv()).await {
                    Ok(Some(msg)) => msg,
                    Ok(None) => break, // channel closed
                    Err(_) => {
                        // Timer fired — save now
                        let state_snapshot = state_tx.borrow().as_ref().clone();
                        let settings_snapshot = settings.read().await.clone();
                        save(&state_snapshot, &settings_snapshot);
                        save_deadline = None;
                        continue;
                    }
                }
            } else {
                match msg_rx.recv().await {
                    Some(msg) => msg,
                    None => break,
                }
            };

            match message {
                ReducerMsg::Update(msg) => {
                    let mut state = state_tx.borrow().as_ref().clone();
                    let current_settings = settings.read().await.clone();
                    let (output_msg, mut pw_messages) =
                        state.update(&graph, msg, &current_settings);
                    pw_messages.extend(state.diff(&graph, &current_settings));

                    for m in pw_messages {
                        if pw_sender.send(m).is_err() {
                            error!("[Reducer] PipeWire channel closed");
                            return;
                        }
                    }

                    if let Some(out) = output_msg {
                        let _ = output_tx.send(out);
                    }
                    let _ = state_tx.send(Arc::new(state));

                    // Reset auto-save timer on every mutation
                    save_deadline = Some(tokio::time::Instant::now() + save_interval);
                }
                ReducerMsg::GraphUpdate(new_graph) => {
                    graph = new_graph;
                    let current_settings = settings.read().await.clone();
                    let mut state = state_tx.borrow().as_ref().clone();
                    state.rename_easyeffects_channels(&graph);
                    let pw_messages = state.diff(&graph, &current_settings);

                    for m in pw_messages {
                        if pw_sender.send(m).is_err() {
                            error!("[Reducer] PipeWire channel closed");
                            return;
                        }
                    }
                    let _ = state_tx.send(Arc::new(state));
                }
                ReducerMsg::SettingsChanged => {
                    let current_settings = settings.read().await.clone();
                    let mut state = state_tx.borrow().as_ref().clone();
                    let pw_messages = state.diff(&graph, &current_settings);

                    for m in pw_messages {
                        if pw_sender.send(m).is_err() {
                            error!("[Reducer] PipeWire channel closed");
                            return;
                        }
                    }
                    let _ = state_tx.send(Arc::new(state));
                }
                ReducerMsg::Save {
                    clear_state,
                    clear_settings,
                } => {
                    if clear_state {
                        let _ = state_tx.send(Arc::new(MixerSession::default()));
                    }
                    if clear_settings {
                        *settings.write().await = ReconcileSettings::default();
                    }
                    let state_snapshot = state_tx.borrow().as_ref().clone();
                    let settings_snapshot = settings.read().await.clone();
                    save(&state_snapshot, &settings_snapshot);
                }
                ReducerMsg::SaveAndExit => {
                    let state_snapshot = state_tx.borrow().as_ref().clone();
                    let settings_snapshot = settings.read().await.clone();
                    save(&state_snapshot, &settings_snapshot);
                    break;
                }
            }
        }
    });

    Ok((handle, msg_tx))
}
