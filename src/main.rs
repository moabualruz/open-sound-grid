use iced::window;
use iced::Size;

mod app;
mod config;
mod engine;
mod error;
mod plugin;
mod plugins;
mod resolve;
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
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("open_sound_grid=info")
        });

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Single instance check
    let instance = single_instance::SingleInstance::new("open-sound-grid")
        .map_err(|e| anyhow::anyhow!("Single instance check failed: {}", e))?;
    if !instance.is_single() {
        tracing::warn!("OpenSoundGrid is already running");
        return Ok(());
    }

    tracing::info!("Starting OpenSoundGrid");

    // Spawn system tray (returns receiver for tray commands)
    let _tray_rx = tray::spawn_tray();

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
                }
                app
            },
            app::App::update,
            app::App::view,
        )
        .subscription(app::App::subscription)
        .theme(app::App::theme)
        .window(window_settings)
        .run()?;
    } else {
        // No plugin — run UI without audio backend
        iced::application(app::App::new, app::App::update, app::App::view)
            .subscription(app::App::subscription)
            .theme(app::App::theme)
            .window(window_settings)
            .run()?;
    }

    tracing::info!("OpenSoundGrid exiting");
    Ok(())
}
