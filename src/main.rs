use iced::Size;
use iced::window;
use lucide_icons::LUCIDE_FONT_BYTES;

mod app;
mod autostart;
mod config;
mod effects;
mod engine;
mod error;
mod hotkeys;
mod notifications;
mod plugin;
mod plugins;
mod presets;
mod resolve;
mod sound_check;
mod tray;
mod ui;

fn main() -> anyhow::Result<()> {
    // Initialize logging.
    // Default: info level. Override with RUST_LOG env var.
    // Examples:
    //   RUST_LOG=debug                          — all modules at debug
    //   RUST_LOG=open_sound_grid=trace          — trace-level for our code only
    //   RUST_LOG=open_sound_grid::plugins=debug — debug PA plugin only
    //   RUST_LOG=warn                           — quiet mode
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("open_sound_grid=info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Single instance check — with orphaned process recovery
    let instance = single_instance::SingleInstance::new("open-sound-grid")
        .map_err(|e| anyhow::anyhow!("Single instance check failed: {}", e))?;
    if !instance.is_single() {
        // Check if an actual open-sound-grid process is running.
        // If the lock is held by an orphaned child (e.g. pactl subscribe),
        // kill it and retry.
        if is_real_instance_running() {
            tracing::warn!("Open Sound Grid is already running");
            return Ok(());
        }
        tracing::warn!("Lock held by orphaned process — cleaning up and retrying");
        kill_orphaned_lock_holders();
        // Drop old instance and re-acquire
        drop(instance);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let instance = single_instance::SingleInstance::new("open-sound-grid")
            .map_err(|e| anyhow::anyhow!("Single instance retry failed: {}", e))?;
        if !instance.is_single() {
            tracing::error!("Failed to acquire lock after cleanup");
            return Ok(());
        }
        // Keep instance alive for the rest of main
        std::mem::forget(instance);
    }

    tracing::info!("Starting Open Sound Grid");

    // Spawn system tray. The command receiver is stored in tray::TRAY_RX so
    // the iced subscription can consume it on first tick (BUG-003 fix).
    tray::spawn_tray();
    tracing::debug!("Tray spawned; TRAY_RX ready for subscription");

    // Create and start the audio plugin before the iced event loop
    let audio_plugin = plugins::create_default_plugin();
    let mut plugin_manager = plugin::manager::PluginManager::new();
    let bridge = match plugin_manager.start(audio_plugin) {
        Ok(b) => {
            tracing::info!("Audio plugin started");
            Some(b)
        }
        Err(e) => {
            tracing::error!("Failed to start audio plugin: {e}");
            None
        }
    };

    // Launch iced application
    let cfg = config::AppConfig::load();
    let window_settings = window::Settings {
        size: Size::new(cfg.ui.window_width as f32, cfg.ui.window_height as f32),
        min_size: Some(Size::new(600.0, 400.0)),
        ..Default::default()
    };

    // Store event receiver in global slot BEFORE iced starts.
    // The subscription will consume it on first tick.
    if let Some(bridge) = bridge {
        // Split: command_tx stays with engine, event_rx goes to subscription
        let bridge_cell = std::sync::Mutex::new(Some(bridge));

        iced::application(
            move || {
                let mut app = app::App::new();
                if let Some(bridge) = bridge_cell.lock().unwrap().take() {
                    let event_rx = app.engine.attach(bridge);
                    app::App::set_event_receiver(event_rx);
                    tracing::info!("Plugin bridge attached to engine");
                    app.restore_from_config();
                }
                app
            },
            app::App::update,
            app::App::view,
        )
        .font(LUCIDE_FONT_BYTES)
        .subscription(app::App::subscription)
        .theme(app::App::theme)
        .window(window_settings)
        .run()?;
    } else {
        // No plugin — run UI without audio backend
        iced::application(app::App::new, app::App::update, app::App::view)
            .font(LUCIDE_FONT_BYTES)
            .subscription(app::App::subscription)
            .theme(app::App::theme)
            .window(window_settings)
            .run()?;
    }

    tracing::info!("Open Sound Grid exiting");
    Ok(())
}

/// Check if a real `open-sound-grid` process (not a child like pactl) is running.
fn is_real_instance_running() -> bool {
    let output = std::process::Command::new("pgrep")
        .args(["-x", "open-sound-gri"]) // pgrep truncates to 15 chars
        .output();
    match output {
        Ok(out) => {
            let pids = String::from_utf8_lossy(&out.stdout);
            let own_pid = std::process::id().to_string();
            // Check if any matching PID is NOT our own process
            pids.lines()
                .any(|pid| pid.trim() != own_pid && !pid.trim().is_empty())
        }
        Err(_) => false,
    }
}

/// Kill processes holding the abstract socket that aren't the real app.
fn kill_orphaned_lock_holders() {
    // Use ss to find who holds the @open-sound-grid socket
    let output = std::process::Command::new("ss").args(["-xlnp"]).output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("@open-sound-grid") {
                // Extract pid from users:(("pactl",pid=NNNN,fd=N))
                if let Some(pid_start) = line.find("pid=") {
                    let rest = &line[pid_start + 4..];
                    if let Some(pid_end) = rest.find(',') {
                        if let Ok(pid) = rest[..pid_end].parse::<u32>() {
                            tracing::info!(pid, "killing orphaned process holding lock");
                            let _ = std::process::Command::new("kill")
                                .arg(pid.to_string())
                                .status();
                        }
                    }
                }
            }
        }
    }
}
