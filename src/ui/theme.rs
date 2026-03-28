use iced::Color;

// Background hierarchy
pub const BG_DARKEST: Color = Color::from_rgb(0.059, 0.059, 0.067); // #0f0f11
pub const BG_DARK: Color = Color::from_rgb(0.098, 0.098, 0.110); // #19191c
pub const BG_PANEL: Color = Color::from_rgb(0.137, 0.137, 0.153); // #232327
pub const BG_ELEVATED: Color = Color::from_rgb(0.176, 0.176, 0.196); // #2d2d32

// Text
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.894, 0.894, 0.906); // #e4e4e7
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.612, 0.612, 0.647); // #9c9ca5
pub const TEXT_MUTED: Color = Color::from_rgb(0.400, 0.400, 0.431); // #66666e

// Accent
pub const ACCENT_BLUE: Color = Color::from_rgb(0.392, 0.584, 0.929); // #6495ed
pub const ACCENT_RED: Color = Color::from_rgb(1.0, 0.388, 0.278); // #ff6347
pub const ACCENT_GREEN: Color = Color::from_rgb(0.298, 0.686, 0.314); // #4caf50
pub const ACCENT_YELLOW: Color = Color::from_rgb(1.0, 0.757, 0.027); // #ffc107

// VU meter gradient stops
pub const VU_GREEN: Color = Color::from_rgb(0.298, 0.686, 0.314);
pub const VU_YELLOW: Color = Color::from_rgb(1.0, 0.843, 0.0);
pub const VU_RED: Color = Color::from_rgb(0.957, 0.263, 0.212);

// Borders & dividers
pub const BORDER_SUBTLE: Color = Color::from_rgb(0.200, 0.200, 0.220);
pub const BORDER_ACTIVE: Color = Color::from_rgb(0.392, 0.584, 0.929);
