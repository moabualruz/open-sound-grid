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
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("open_sound_grid=info".parse().unwrap()),
        )
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

    // Use a Mutex to hand the bridge into the boot closure (Fn, called once)
    let bridge_cell = std::sync::Mutex::new(bridge);

    iced::application(
        move || {
            let mut app = app::App::new();
            if let Some(bridge) = bridge_cell.lock().unwrap().take() {
                app.engine.attach(bridge);
                tracing::info!("Plugin bridge attached to engine");
            }
            app
        },
        app::App::update,
        app::App::view,
    )
    .theme(app::App::theme)
    .window(window_settings)
    .run()?;

    tracing::info!("OpenSoundGrid exiting");
    Ok(())
}
