use iced::Color;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, trace};

// =============================================================================
// Open Sound Grid Theme — Claude/Anthropic Design Language
//
// Wave Link 3.0 UX layout + Anthropic warm neutral tones
// Dark default, light available
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

// --- Dark Theme (Default) ---

// Background hierarchy — warm neutral darks
pub const BG_PRIMARY: Color = Color::from_rgb(0.102, 0.102, 0.102); // #1a1a1a
pub const BG_SECONDARY: Color = Color::from_rgb(0.141, 0.141, 0.141); // #242424
pub const BG_ELEVATED: Color = Color::from_rgb(0.180, 0.180, 0.180); // #2e2e2e
pub const BG_HOVER: Color = Color::from_rgb(0.220, 0.220, 0.220); // #383838
/// Empty cell background — between primary and elevated for visible distinction
pub const BG_EMPTY_CELL: Color = Color::from_rgb(0.145, 0.145, 0.145); // #252525

// Text — warm whites
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.910, 0.894, 0.875); // #e8e4df
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.639, 0.620, 0.588); // #a39e96
pub const TEXT_MUTED: Color = Color::from_rgb(0.502, 0.482, 0.459); // #807b75 — WCAG AA compliant against BG_SECONDARY

// Accent — Claude coral/orange
pub const ACCENT: Color = Color::from_rgb(0.855, 0.467, 0.337); // #da7756
#[allow(dead_code)]
pub const ACCENT_HOVER: Color = Color::from_rgb(0.769, 0.416, 0.294); // #c46a4b
#[allow(dead_code)]
pub const ACCENT_SECONDARY: Color = Color::from_rgb(0.486, 0.612, 0.749); // #7c9cbf

// Borders
pub const BORDER: Color = Color::from_rgb(0.200, 0.200, 0.200); // #333333
#[allow(dead_code)]
pub const BORDER_ACTIVE: Color = Color::from_rgb(0.855, 0.467, 0.337); // #da7756

// VU meter gradient stops
pub const VU_GREEN: Color = Color::from_rgb(0.298, 0.686, 0.314); // #4caf50
pub const VU_AMBER: Color = Color::from_rgb(1.0, 0.596, 0.0); // #ff9800
pub const VU_RED: Color = Color::from_rgb(0.957, 0.263, 0.212); // #f44336

// Mix column default colors
pub const MIX_MONITOR: Color = Color::from_rgb(0.486, 0.612, 0.749); // #7c9cbf (blue)
pub const MIX_STREAM: Color = Color::from_rgb(0.855, 0.467, 0.337); // #da7756 (coral)
pub const MIX_VOD: Color = Color::from_rgb(0.545, 0.435, 0.690); // #8b6fb0 (purple)
pub const MIX_CHAT: Color = Color::from_rgb(0.365, 0.667, 0.408); // #5daa68 (green)
pub const MIX_AUX: Color = Color::from_rgb(0.769, 0.639, 0.353); // #c4a35a (gold)

// Status
#[allow(dead_code)]
pub const STATUS_CONNECTED: Color = Color::from_rgb(0.298, 0.686, 0.314); // green
#[allow(dead_code)]
pub const STATUS_ERROR: Color = Color::from_rgb(0.957, 0.263, 0.212); // red

// --- Light Theme ---

pub const LIGHT_BG_PRIMARY: Color = Color::from_rgb(0.980, 0.976, 0.969); // #faf9f7
pub const LIGHT_BG_SECONDARY: Color = Color::from_rgb(0.941, 0.933, 0.922); // #f0eeeb
pub const LIGHT_BG_ELEVATED: Color = Color::from_rgb(1.0, 1.0, 1.0); // #ffffff
pub const LIGHT_BG_HOVER: Color = Color::from_rgb(0.918, 0.910, 0.898); // #eae8e5
pub const LIGHT_BG_EMPTY_CELL: Color = Color::from_rgb(0.960, 0.956, 0.949); // #f5f4f2
pub const LIGHT_TEXT_PRIMARY: Color = Color::from_rgb(0.102, 0.102, 0.102); // #1a1a1a
pub const LIGHT_TEXT_SECONDARY: Color = Color::from_rgb(0.420, 0.400, 0.376); // #6b6660
pub const LIGHT_TEXT_MUTED: Color = Color::from_rgb(0.600, 0.580, 0.557); // #99948e
pub const LIGHT_BORDER: Color = Color::from_rgb(0.898, 0.890, 0.875); // #e5e3df

// --- Theme-aware helpers ---

/// Query the desktop environment for its preferred color scheme.
///
/// Uses the freedesktop portal Settings interface (works on KDE, GNOME, etc.).
/// Returns `Dark` or `Light`. Falls back to `Dark` if D-Bus is unavailable.
/// Cached system theme — detected once at startup via D-Bus, not per-frame.
static CACHED_SYSTEM_THEME: std::sync::OnceLock<ThemeMode> = std::sync::OnceLock::new();

pub fn detect_system_theme() -> ThemeMode {
    *CACHED_SYSTEM_THEME.get_or_init(|| {
        debug!("querying system theme via D-Bus (one-time)");
        let result = std::process::Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings",
                "Read",
                "ss",
                "org.freedesktop.appearance",
                "color-scheme",
            ])
            .output();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                trace!(raw_output = %stdout, "D-Bus color-scheme response");
                if stdout.contains('1') {
                    info!("system theme detected: Dark (cached)");
                    ThemeMode::Dark
                } else {
                    info!("system theme detected: Light (cached)");
                    ThemeMode::Light
                }
            }
            Err(e) => {
                debug!(error = %e, "D-Bus query failed — defaulting to Dark");
                ThemeMode::Dark
            }
        }
    })
}

/// Resolve `ThemeMode::System` to a concrete `Dark` or `Light`.
#[instrument]
pub fn resolve_theme(mode: ThemeMode) -> ThemeMode {
    match mode {
        ThemeMode::System => detect_system_theme(),
        other => other,
    }
}

pub fn bg_primary(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => BG_PRIMARY,
        ThemeMode::Light => LIGHT_BG_PRIMARY,
        ThemeMode::System => bg_primary(resolve_theme(mode)),
    }
}

pub fn bg_secondary(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => BG_SECONDARY,
        ThemeMode::Light => LIGHT_BG_SECONDARY,
        ThemeMode::System => bg_secondary(resolve_theme(mode)),
    }
}

pub fn bg_elevated(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => BG_ELEVATED,
        ThemeMode::Light => LIGHT_BG_ELEVATED,
        ThemeMode::System => bg_elevated(resolve_theme(mode)),
    }
}

pub fn bg_hover(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => BG_HOVER,
        ThemeMode::Light => LIGHT_BG_HOVER,
        ThemeMode::System => bg_hover(resolve_theme(mode)),
    }
}

pub fn text_primary(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => TEXT_PRIMARY,
        ThemeMode::Light => LIGHT_TEXT_PRIMARY,
        ThemeMode::System => text_primary(resolve_theme(mode)),
    }
}

pub fn text_secondary(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => TEXT_SECONDARY,
        ThemeMode::Light => LIGHT_TEXT_SECONDARY,
        ThemeMode::System => text_secondary(resolve_theme(mode)),
    }
}

pub fn text_muted(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => TEXT_MUTED,
        ThemeMode::Light => LIGHT_TEXT_MUTED,
        ThemeMode::System => text_muted(resolve_theme(mode)),
    }
}

pub fn border_color(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => BORDER,
        ThemeMode::Light => LIGHT_BORDER,
        ThemeMode::System => border_color(resolve_theme(mode)),
    }
}

/// Background for empty matrix cells — slightly lighter than primary for visible distinction.
pub fn bg_empty_cell(mode: ThemeMode) -> Color {
    match mode {
        ThemeMode::Dark => BG_EMPTY_CELL,
        ThemeMode::Light => LIGHT_BG_EMPTY_CELL,
        ThemeMode::System => bg_empty_cell(resolve_theme(mode)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_system_variant_exists() {
        let mode = ThemeMode::System;
        assert_ne!(mode, ThemeMode::Dark);
        assert_ne!(mode, ThemeMode::Light);
    }

    #[test]
    fn detect_system_theme_returns_dark_or_light() {
        let result = detect_system_theme();
        assert!(result == ThemeMode::Dark || result == ThemeMode::Light);
    }

    #[test]
    fn resolve_theme_mode_system_returns_concrete() {
        let resolved = resolve_theme(ThemeMode::System);
        assert!(resolved == ThemeMode::Dark || resolved == ThemeMode::Light);
    }

    #[test]
    fn resolve_theme_dark_passes_through() {
        assert_eq!(resolve_theme(ThemeMode::Dark), ThemeMode::Dark);
    }

    #[test]
    fn resolve_theme_light_passes_through() {
        assert_eq!(resolve_theme(ThemeMode::Light), ThemeMode::Light);
    }
}
