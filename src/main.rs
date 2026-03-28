use iced::window;
use iced::Size;

mod app;
mod config;
mod engine;
mod error;
mod plugin;
mod plugins;
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

    // Spawn system tray (stub)
    tray::spawn_tray();

    // Launch iced application
    let cfg = config::AppConfig::load();
    let window_settings = window::Settings {
        size: Size::new(cfg.ui.window_width as f32, cfg.ui.window_height as f32),
        min_size: Some(Size::new(600.0, 400.0)),
        ..Default::default()
    };

    iced::application(app::App::new, app::App::update, app::App::view)
        .theme(app::App::theme)
        .window(window_settings)
        .run()?;

    tracing::info!("OpenSoundGrid exiting");
    Ok(())
}
