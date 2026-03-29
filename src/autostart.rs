//! XDG autostart management for Open Sound Grid.
//!
//! Creates/removes a .desktop file at ~/.config/autostart/ so the app
//! launches at login on Linux desktop environments.

use std::fs;
use std::path::PathBuf;

/// Get the path to the autostart .desktop file.
fn autostart_path() -> PathBuf {
    let config_dir = directories::BaseDirs::new()
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    config_dir.join("autostart").join("open-sound-grid.desktop")
}

/// Install the XDG autostart .desktop entry.
pub fn install_autostart() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("installing XDG autostart entry");
    let path = autostart_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        tracing::debug!(dir = %parent.display(), "ensured autostart directory exists");
    }

    let content = r#"[Desktop Entry]
Type=Application
Name=Open Sound Grid
Comment=Professional audio matrix mixer for Linux
Exec=open-sound-grid
Icon=open-sound-grid
Categories=AudioVideo;Audio;Mixer;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#;

    fs::write(&path, content)?;
    tracing::info!(path = %path.display(), "autostart entry installed");
    Ok(())
}

/// Remove the XDG autostart .desktop entry.
pub fn remove_autostart() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("removing XDG autostart entry");
    let path = autostart_path();

    if path.exists() {
        fs::remove_file(&path)?;
        tracing::info!(path = %path.display(), "autostart entry removed");
    } else {
        tracing::debug!(path = %path.display(), "autostart entry not found, nothing to remove");
    }

    Ok(())
}

/// Check if autostart is currently installed.
pub fn is_autostart_installed() -> bool {
    autostart_path().exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_path_is_under_config() {
        let path = autostart_path();
        assert!(
            path.to_string_lossy().contains("autostart"),
            "path should be under autostart dir"
        );
        assert!(
            path.to_string_lossy().ends_with("open-sound-grid.desktop"),
            "file should be named open-sound-grid.desktop"
        );
    }
}
