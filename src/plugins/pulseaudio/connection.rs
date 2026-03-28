//! PulseAudio connection using the threaded mainloop.
//!
//! The threaded mainloop provides its own locking. We avoid RefCell entirely
//! since the PA state callback fires from the PA internal thread.

use std::time::{Duration, Instant};

use libpulse_binding::context::{self, Context, FlagSet as ContextFlagSet};
use libpulse_binding::mainloop::threaded::Mainloop;

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
    pub fn connect() -> Result<Self> {
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
        mainloop.lock();

        if mainloop.start().is_err() {
            mainloop.unlock();
            return Err(OsgError::PulseAudio("failed to start threaded mainloop".into()));
        }

        // Connect (mainloop lock is held)
        if context
            .connect(None, ContextFlagSet::NOAUTOSPAWN, None)
            .is_err()
        {
            mainloop.unlock();
            mainloop.stop();
            return Err(OsgError::PulseAudio("context connect call failed".into()));
        }

        // Wait for Ready
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            let state = context.get_state();
            match state {
                context::State::Ready => break,
                context::State::Failed | context::State::Terminated => {
                    mainloop.unlock();
                    mainloop.stop();
                    return Err(OsgError::PulseAudio(format!(
                        "context entered terminal state: {state:?}"
                    )));
                }
                _ => {
                    if Instant::now() >= deadline {
                        mainloop.unlock();
                        mainloop.stop();
                        return Err(OsgError::PulseAudio(
                            "timed out waiting for context Ready".into(),
                        ));
                    }
                    mainloop.wait();
                }
            }
        }

        mainloop.unlock();
        tracing::info!("PulseAudio connection established");

        Ok(Self {
            mainloop,
            context,
            connected: true,
        })
    }

    /// Disconnect and stop the mainloop. Safe to call multiple times.
    pub fn disconnect(&mut self) {
        if !self.connected {
            return;
        }
        self.mainloop.lock();
        self.context.disconnect();
        self.mainloop.unlock();
        self.mainloop.stop();
        self.connected = false;
        tracing::info!("PulseAudio connection closed");
    }

    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Drop for PulseConnection {
    fn drop(&mut self) {
        self.disconnect();
    }
}
