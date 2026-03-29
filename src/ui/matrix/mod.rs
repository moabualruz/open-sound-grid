//! Matrix grid widget — the core UI of Open Sound Grid.
//!
//! Rows = audio sources (software channels)
//! Columns = output mixes
//! Each intersection = mute button + volume slider + VU meter (thin bar below slider)

mod cell;
mod channel_label;
mod channel_picker;
mod grid;
mod mix_header;

pub use grid::matrix_grid;

/// Height of mix column headers in pixels.
const HEADER_HEIGHT: f32 = 64.0;
/// Height of each matrix cell and channel label row in pixels.
const CELL_HEIGHT: f32 = 56.0;
/// Height when stereo L/R sliders are active — taller to fit both sliders.
const CELL_HEIGHT_STEREO: f32 = 76.0;
/// Width of mix columns and channel label cells in pixels.
const COL_WIDTH: f32 = 150.0;
const LABEL_WIDTH: f32 = 200.0;
/// Border radius for cells, headers, and labels (WL3-style rounded cards).
const CELL_RADIUS: f32 = 8.0;
/// Spacing between cells in the grid.
const CELL_SPACING: f32 = 4.0;

/// Mix column colors, cycled for each mix.
const MIX_COLORS: &[iced::Color] = &[
    crate::ui::theme::MIX_MONITOR,
    crate::ui::theme::MIX_STREAM,
    crate::ui::theme::MIX_VOD,
    crate::ui::theme::MIX_CHAT,
    crate::ui::theme::MIX_AUX,
];

/// Preset channel types for the creation picker.
const CHANNEL_PRESETS: &[(&str, &str)] = &[
    ("System", "system"),
    ("Game", "game"),
    ("Chat", "chat"),
    ("Music", "music"),
    ("Browser", "browser"),
    ("Voice", "voice"),
    ("SFX", "sfx"),
    ("Aux 1", "aux"),
    ("Aux 2", "aux"),
];
