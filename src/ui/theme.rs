use iced::Color;

// =============================================================================
// OpenSoundGrid Theme — Claude/Anthropic Design Language
//
// Wave Link 3.0 UX layout + Anthropic warm neutral tones
// Dark default, light available
// =============================================================================

// --- Dark Theme (Default) ---

// Background hierarchy — warm neutral darks
pub const BG_PRIMARY: Color = Color::from_rgb(0.102, 0.102, 0.102); // #1a1a1a
pub const BG_SECONDARY: Color = Color::from_rgb(0.141, 0.141, 0.141); // #242424
pub const BG_ELEVATED: Color = Color::from_rgb(0.180, 0.180, 0.180); // #2e2e2e
pub const BG_HOVER: Color = Color::from_rgb(0.220, 0.220, 0.220); // #383838

// Text — warm whites
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.910, 0.894, 0.875); // #e8e4df
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.639, 0.620, 0.588); // #a39e96
pub const TEXT_MUTED: Color = Color::from_rgb(0.420, 0.400, 0.376); // #6b6660

// Accent — Claude coral/orange
pub const ACCENT: Color = Color::from_rgb(0.855, 0.467, 0.337); // #da7756
pub const ACCENT_HOVER: Color = Color::from_rgb(0.769, 0.416, 0.294); // #c46a4b
pub const ACCENT_SECONDARY: Color = Color::from_rgb(0.486, 0.612, 0.749); // #7c9cbf

// Borders
pub const BORDER: Color = Color::from_rgb(0.200, 0.200, 0.200); // #333333
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
pub const STATUS_CONNECTED: Color = Color::from_rgb(0.298, 0.686, 0.314); // green
pub const STATUS_ERROR: Color = Color::from_rgb(0.957, 0.263, 0.212); // red

// --- Backward compat aliases (used in existing widgets) ---
pub const BG_DARKEST: Color = BG_PRIMARY;
pub const BG_DARK: Color = BG_SECONDARY;
pub const BG_PANEL: Color = BG_ELEVATED;
pub const BORDER_SUBTLE: Color = BORDER;
