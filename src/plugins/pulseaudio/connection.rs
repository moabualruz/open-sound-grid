//! PulseAudio connection using the threaded mainloop.
//!
//! The threaded mainloop provides its own locking. We avoid RefCell entirely
//! since the PA state callback fires from the PA internal thread.

use std::time::{Duration, Instant};

use libpulse_binding::context::{self, Context, FlagSet as ContextFlagSet};
use libpulse_binding::mainloop::threaded::Mainloop;
use tracing::instrument;

use crate::error::{OsgError, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Owns the PulseAudio threaded mainloop and context.
///
/// NOT Send/Sync safe on its own — the PulseAudioPlugin uses
/// `unsafe impl Send` because the plugin is moved into a dedicated thread.
pub struct PulseConnection {
    mainloop: Mainloop,
    context: Context,
    connected: bool,
}

impl PulseConnection {
    /// Connect to the default PulseAudio server.
    #[instrument]
    pub fn connect() -> Result<Self> {
        tracing::debug!("attempting PulseAudio connection");

        let mut mainloop = Mainloop::new()
            .ok_or_else(|| OsgError::PulseAudio("failed to create threaded mainloop".into()))?;

        let mut context = Context::new(&mainloop, "OpenSoundGrid")
            .ok_or_else(|| OsgError::PulseAudio("failed to create context".into()))?;

        // State callback: signal the mainloop on every context state transition.
        // We use a raw pointer to the mainloop because the callback fires from
        // the PA internal thread, and Rc<RefCell<>> panics on concurrent access.
        let ml_ptr = &mainloop as *const Mainloop as *mut Mainloop;
        context.set_state_callback(Some(Box::new(move || {
            // SAFETY: The mainloop outlives the context (we disconnect before dropping).
            // The PA threaded mainloop is designed for this signal pattern.
            unsafe { (*ml_ptr).signal(false) };
        })));

        // Start mainloop thread
        tracing::trace!("locking mainloop before start");
        mainloop.lock();

        if mainloop.start().is_err() {
            tracing::trace!("unlocking mainloop after failed start");
            mainloop.unlock();
            tracing::error!("failed to start PulseAudio threaded mainloop");
            return Err(OsgError::PulseAudio("failed to start threaded mainloop".into()));
        }
        tracing::debug!("PulseAudio threaded mainloop started");

        // Connect (mainloop lock is held)
        if context
            .connect(None, ContextFlagSet::NOAUTOSPAWN, None)
            .is_err()
        {
            tracing::trace!("unlocking mainloop after failed connect");
            mainloop.unlock();
            mainloop.stop();
            tracing::error!("PulseAudio context connect call failed");
            return Err(OsgError::PulseAudio("context connect call failed".into()));
        }

        // Wait for Ready
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            let state = context.get_state();
            tracing::debug!(state = ?state, "PulseAudio context state change");
            match state {
                context::State::Ready => break,
                context::State::Failed | context::State::Terminated => {
                    tracing::trace!("unlocking mainloop after terminal state");
                    mainloop.unlock();
                    mainloop.stop();
                    tracing::error!(state = ?state, "PulseAudio context entered terminal state");
                    return Err(OsgError::PulseAudio(format!(
                        "context entered terminal state: {state:?}"
                    )));
                }
                _ => {
                    if Instant::now() >= deadline {
                        tracing::trace!("unlocking mainloop after timeout");
                        mainloop.unlock();
                        mainloop.stop();
                        tracing::error!(timeout_secs = CONNECT_TIMEOUT.as_secs(), "timed out waiting for PulseAudio context Ready");
                        return Err(OsgError::PulseAudio(
                            "timed out waiting for context Ready".into(),
                        ));
                    }
                    mainloop.wait();
                }
            }
        }

        tracing::trace!("unlocking mainloop after context Ready");
        mainloop.unlock();
        tracing::info!("PulseAudio connection established");

        Ok(Self {
            mainloop,
            context,
            connected: true,
        })
    }

    /// Disconnect and stop the mainloop. Safe to call multiple times.
    #[instrument(skip(self))]
    pub fn disconnect(&mut self) {
        if !self.connected {
            tracing::debug!("disconnect called but already disconnected");
            return;
        }
        tracing::debug!("disconnecting PulseAudio connection");
        tracing::trace!("locking mainloop for disconnect");
        self.mainloop.lock();
        self.context.disconnect();
        tracing::trace!("unlocking mainloop after context disconnect");
        self.mainloop.unlock();
        self.mainloop.stop();
        self.connected = false;
        tracing::info!("PulseAudio connection closed");
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Access the mainloop for locking during PA operations.
    /// All PA introspect/stream calls must be made with the mainloop locked.
    #[allow(dead_code)]
    pub fn mainloop(&self) -> &Mainloop {
        &self.mainloop
    }

    /// Mutable access to mainloop (needed for lock/unlock/wait/signal).
    pub fn mainloop_mut(&mut self) -> &mut Mainloop {
        &mut self.mainloop
    }

    /// Access the context for introspect and stream operations.
    #[allow(dead_code)]
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Mutable access to context (needed for introspect calls).
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }
}

impl Drop for PulseConnection {
    fn drop(&mut self) {
        self.disconnect();
    }
}
