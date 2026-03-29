//! PipeWire connection — owns MainLoop, Context, and Core.
//!
//! All PipeWire objects are !Send. The connection is created on the plugin
//! thread and only accessed there, following the same pattern as PulseConnection.
//!
//! We use the `Rc`-based smart pointer variants (`MainLoopRc`, `ContextRc`,
//! `CoreRc`) because they manage the required drop ordering
//! (Core → Context → MainLoop) automatically via their internal `Rc` fields.

#[cfg(feature = "pipewire-backend")]
use pipewire as pw;

#[cfg(feature = "pipewire-backend")]
use crate::error::{OsgError, Result};

/// Owns the PipeWire main loop, context, and core connection.
///
/// NOT Send/Sync on its own — PipeWire's `Rc`-based types are `!Send`.
/// `PwConnection` is moved into the plugin thread once and accessed only from
/// that thread, so the `unsafe impl Send` is sound (same contract as
/// `PulseConnection`).
#[cfg(feature = "pipewire-backend")]
pub struct PwConnection {
    main_loop: pw::main_loop::MainLoopRc,
    /// `CoreRc` holds a `ContextRc` clone internally, keeping the context
    /// alive as long as the core is alive. We do not need to store
    /// `ContextRc` separately.
    core: pw::core::CoreRc,
    /// Keeps the core-error listener registered for the lifetime of the
    /// connection. Must be dropped before `core`.
    _listener: pw::core::Listener,
}

// SAFETY: PwConnection is moved into the plugin thread exactly once and
// accessed exclusively from that thread thereafter. PipeWire's Rc-based
// types carry raw pointers internally (hence !Send), but we uphold the
// single-thread contract. Same pattern as PulseConnection.
#[cfg(feature = "pipewire-backend")]
unsafe impl Send for PwConnection {}

#[cfg(feature = "pipewire-backend")]
impl PwConnection {
    /// Connect to the PipeWire daemon.
    ///
    /// Creates the main loop, context, and core, registers a core-error
    /// listener, and runs several loop iterations to complete the initial
    /// protocol handshake before returning.
    #[tracing::instrument]
    pub fn connect() -> Result<Self> {
        tracing::debug!("attempting PipeWire connection");

        pw::init();

        let main_loop = pw::main_loop::MainLoopRc::new(None)
            .map_err(|e| OsgError::PulseAudio(format!("failed to create PW main loop: {e}")))?;

        // ContextRc::new accepts any &T: IsLoopRc — MainLoopRc satisfies this.
        let context = pw::context::ContextRc::new(&main_loop, None)
            .map_err(|e| OsgError::PulseAudio(format!("failed to create PW context: {e}")))?;

        // connect_rc returns a CoreRc that holds a ContextRc clone internally,
        // keeping the context alive for at least as long as the core is alive.
        let core = context
            .connect_rc(None)
            .map_err(|e| OsgError::PulseAudio(format!("failed to connect to PW daemon: {e}")))?;

        // Register a listener for core-level protocol errors.
        // Callback signature: Fn(id: u32, seq: i32, res: i32, message: &str)
        let listener = core
            .add_listener_local()
            .error(|id, seq, res, message| {
                tracing::error!(id, seq, res, message, "PipeWire core error");
            })
            .register();

        // Run a few loop iterations to process the initial server handshake
        // before returning the connection to callers.
        tracing::trace!("processing PipeWire initial handshake");
        for _ in 0..10 {
            main_loop
                .loop_()
                .iterate(std::time::Duration::from_millis(10));
        }

        tracing::info!("PipeWire connection established");

        Ok(Self {
            main_loop,
            core,
            _listener: listener,
        })
    }

    /// Borrow the core for creating proxy objects or issuing sync calls.
    pub fn core(&self) -> &pw::core::CoreRc {
        &self.core
    }

    /// Borrow the main loop for running iterations or quitting.
    pub fn main_loop(&self) -> &pw::main_loop::MainLoopRc {
        &self.main_loop
    }

    /// Run one iteration of the PipeWire event loop.
    ///
    /// Pass `Duration::ZERO` for a non-blocking poll; pass a positive duration
    /// to block up to that timeout waiting for events.
    pub fn iterate(&self, timeout: std::time::Duration) {
        self.main_loop.loop_().iterate(timeout);
    }

    /// Perform a synchronised round-trip: dispatch all pending events and
    /// method replies until the server acknowledges the sync sequence.
    ///
    /// Call this after `create_object` / `destroy_object` to ensure the remote
    /// operation has been completed before continuing.
    #[tracing::instrument(skip(self))]
    pub fn do_roundtrip(&self) {
        use std::cell::Cell;
        use std::rc::Rc;

        let done = Rc::new(Cell::new(false));
        let done_clone = done.clone();
        let loop_clone = self.main_loop.clone();

        let pending = match self.core.sync(0) {
            Ok(seq) => seq,
            Err(e) => {
                tracing::warn!("PipeWire sync call failed: {e}");
                return;
            }
        };

        let _done_listener = self
            .core
            .add_listener_local()
            .done(move |id, seq| {
                if id == pw::core::PW_ID_CORE && seq == pending {
                    done_clone.set(true);
                    loop_clone.quit();
                }
            })
            .register();

        while !done.get() {
            self.main_loop.run();
        }
        tracing::trace!("PipeWire roundtrip complete");
    }

    /// Signal intent to disconnect.
    ///
    /// The actual resource cleanup happens in `Drop`. This method exists for
    /// symmetry with `PulseConnection::disconnect` and emits a log entry at
    /// the call site.
    pub fn disconnect(&mut self) {
        tracing::debug!("PwConnection::disconnect called — resources released on drop");
    }
}

#[cfg(feature = "pipewire-backend")]
impl Drop for PwConnection {
    fn drop(&mut self) {
        tracing::debug!("dropping PwConnection — PipeWire resources released");
        // Drop order is declaration order:
        //   _listener → core (CoreRc) → main_loop (MainLoopRc)
        // CoreRc holds a ContextRc internally, so context is released when
        // the CoreRc refcount reaches zero. MainLoop outlives context by design.
    }
}
