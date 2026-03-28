//! PulseAudio connection management using the threaded mainloop.
//!
//! Wraps `libpulse_binding`'s threaded mainloop and context into a single
//! `PulseConnection` that handles connect/disconnect lifecycle with proper
//! error handling and a configurable connection timeout.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use libpulse_binding::context::{self, Context, FlagSet as ContextFlagSet};
use libpulse_binding::mainloop::threaded::Mainloop;

use crate::error::{OsgError, Result};

/// How long `connect()` waits for the context to reach `Ready` before failing.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Owns the PulseAudio threaded mainloop and context.
///
/// The mainloop thread runs in the background once connected. All PA
/// operations that touch the context must lock the mainloop first.
pub struct PulseConnection {
    mainloop: Rc<RefCell<Mainloop>>,
    context: Rc<RefCell<Context>>,
    connected: bool,
}

impl PulseConnection {
    /// Connect to the default PulseAudio server.
    ///
    /// 1. Creates a threaded mainloop.
    /// 2. Creates a context with app name `"OpenSoundGrid"`.
    /// 3. Installs a state callback that signals the mainloop on transitions.
    /// 4. Starts the mainloop thread.
    /// 5. Initiates a context connection and waits for `State::Ready`.
    /// 6. Returns `Err(OsgError::PulseAudio(_))` on failure or timeout.
    pub fn connect() -> Result<Self> {
        // --- mainloop ---
        let mainloop = Mainloop::new()
            .ok_or_else(|| OsgError::PulseAudio("failed to create threaded mainloop".into()))?;
        let mainloop = Rc::new(RefCell::new(mainloop));

        // --- context ---
        let context = Context::new(&*mainloop.borrow(), "OpenSoundGrid")
            .ok_or_else(|| OsgError::PulseAudio("failed to create context".into()))?;
        let context = Rc::new(RefCell::new(context));

        // --- state callback: signal mainloop on every state change ---
        {
            let ml_weak = Rc::clone(&mainloop);
            context
                .borrow_mut()
                .set_state_callback(Some(Box::new(move || {
                    ml_weak.borrow_mut().signal(false);
                })));
        }

        // --- start mainloop thread ---
        mainloop.borrow_mut().lock();

        mainloop.borrow_mut().start().map_err(|_| {
            mainloop.borrow_mut().unlock();
            OsgError::PulseAudio("failed to start threaded mainloop".into())
        })?;

        // --- initiate connection (lock is held) ---
        context
            .borrow_mut()
            .connect(None, ContextFlagSet::NOAUTOSPAWN, None)
            .map_err(|_| {
                mainloop.borrow_mut().unlock();
                mainloop.borrow_mut().stop();
                OsgError::PulseAudio("context connect call failed".into())
            })?;

        // --- wait for Ready state ---
        let deadline = Instant::now() + CONNECT_TIMEOUT;

        loop {
            let state = context.borrow().get_state();

            match state {
                context::State::Ready => break,

                context::State::Failed | context::State::Terminated => {
                    mainloop.borrow_mut().unlock();
                    mainloop.borrow_mut().stop();
                    return Err(OsgError::PulseAudio(format!(
                        "context entered terminal state: {state:?}"
                    )));
                }

                _ => {
                    if Instant::now() >= deadline {
                        mainloop.borrow_mut().unlock();
                        mainloop.borrow_mut().stop();
                        return Err(OsgError::PulseAudio(
                            "timed out waiting for context Ready state".into(),
                        ));
                    }
                    // Release lock, wait for signal, re-acquire lock.
                    mainloop.borrow_mut().wait();
                }
            }
        }

        mainloop.borrow_mut().unlock();

        tracing::info!("PulseAudio connection established");

        Ok(Self {
            mainloop,
            context,
            connected: true,
        })
    }

    /// Disconnect from PulseAudio and stop the mainloop thread.
    ///
    /// Safe to call multiple times; subsequent calls are no-ops.
    pub fn disconnect(&mut self) {
        if !self.connected {
            return;
        }

        // Lock the mainloop before touching the context.
        self.mainloop.borrow_mut().lock();
        self.context.borrow_mut().disconnect();
        self.mainloop.borrow_mut().unlock();

        // Stop the mainloop thread (must be called without lock held).
        self.mainloop.borrow_mut().stop();

        self.connected = false;
        tracing::info!("PulseAudio connection closed");
    }

    /// Returns `true` if the connection is currently active.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Borrows the mainloop (caller must lock/unlock around PA operations).
    pub fn mainloop(&self) -> &Rc<RefCell<Mainloop>> {
        &self.mainloop
    }

    /// Borrows the context (caller must hold the mainloop lock).
    pub fn context(&self) -> &Rc<RefCell<Context>> {
        &self.context
    }
}

impl Drop for PulseConnection {
    fn drop(&mut self) {
        self.disconnect();
    }
}
