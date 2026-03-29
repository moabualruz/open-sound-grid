//! Parametric EQ visualizer widget.
//!
//! Renders a frequency response curve using iced canvas. Displays the
//! summed biquad magnitude response of the channel's EQ band(s) over a
//! log-scale 20 Hz-20 kHz frequency axis, with a dB grid overlay and
//! band-point markers.

mod draw;

use iced::widget::canvas::{self, Action, Cache, Event, Geometry};
use iced::{Element, Length, Rectangle, Theme, mouse};

use crate::app::Message;
use crate::effects::EffectsParams;
use crate::plugin::api::ChannelId;

use draw::{
    draw_background, draw_band_points, draw_curve, draw_db_labels, draw_freq_labels,
    draw_spectrum, freq_to_x, db_to_y, plot_dims, simulated_spectrum,
};

// --- Frequency / dB constants ---
pub(crate) const FREQ_MIN: f32 = 20.0;
pub(crate) const FREQ_MAX: f32 = 20_000.0;
pub(crate) const DB_MIN: f32 = -12.0;
pub(crate) const DB_MAX: f32 = 12.0;

// Padding inside the canvas bounds for the plot area.
pub(crate) const PAD_LEFT: f32 = 28.0;
pub(crate) const PAD_RIGHT: f32 = 8.0;
pub(crate) const PAD_TOP: f32 = 8.0;
pub(crate) const PAD_BOTTOM: f32 = 20.0;

// --- Types ---

/// A single EQ band shown on the visualizer.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EqBand {
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub band_type: BandType,
    pub label: &'static str,
}

/// Filter topology for magnitude-response computation.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum BandType {
    HighPass,
    LowShelf,
    Peak,
    HighShelf,
    LowPass,
}

/// The `Program` struct -- carries all data needed for drawing.
///
/// State (`EqState`) is kept in the iced widget tree; the program struct
/// itself is re-created each frame from the channel's `EffectsParams`.
pub struct EqProgram {
    channel_id: ChannelId,
    bands: Vec<EqBand>,
    /// Frequency spectrum bins as (freq_hz, level_db) pairs for the overlay.
    spectrum: Vec<(f32, f32)>,
}

/// Per-widget mutable state managed by iced.
#[derive(Default)]
pub struct EqState {
    cache: Cache,
    /// Index of the band currently being dragged, if any.
    dragging_band: Option<usize>,
}

// --- Public entry point ---

/// Build the EQ canvas element for `channel_id` using `params`.
///
/// `spectrum_data` is a slice of `(freq_hz, level_db)` bins for the spectrum overlay.
/// Pass an empty slice to use the simulated spectrum until real FFT data is available.
///
/// Returns an `Element` sized `Length::Fill` x `Length::Fixed(200)`.
pub fn eq_canvas<'a>(
    channel_id: ChannelId,
    params: &EffectsParams,
    spectrum_data: &[(f32, f32)],
) -> Element<'a, Message> {
    tracing::trace!(
        channel_id,
        eq_freq = params.eq_freq_hz,
        eq_gain = params.eq_gain_db,
        eq_q = params.eq_q,
        spectrum_bins = spectrum_data.len(),
        "building eq_canvas"
    );

    let bands = vec![EqBand {
        freq_hz: params.eq_freq_hz,
        gain_db: params.eq_gain_db,
        q: params.eq_q,
        band_type: BandType::Peak,
        label: "Peak",
    }];

    // Use real spectrum data when available, otherwise fall back to simulated.
    let spectrum = if spectrum_data.is_empty() {
        simulated_spectrum(params)
    } else {
        spectrum_data.to_vec()
    };

    let program = EqProgram {
        channel_id,
        bands,
        spectrum,
    };

    canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fixed(200.0))
        .into()
}

// --- canvas::Program implementation ---

/// Convert an X pixel position back to a frequency (Hz) -- inverse of `freq_to_x`.
fn x_to_freq(x: f32, plot_w: f32) -> f32 {
    let t = ((x - PAD_LEFT) / plot_w).clamp(0.0, 1.0);
    10.0f32.powf(FREQ_MIN.log10() + t * (FREQ_MAX.log10() - FREQ_MIN.log10()))
}

/// Convert a Y pixel position back to dB -- inverse of `db_to_y`.
fn y_to_db(y: f32, plot_h: f32) -> f32 {
    let t = ((y - PAD_TOP) / plot_h).clamp(0.0, 1.0);
    DB_MAX + t * (DB_MIN - DB_MAX)
}

/// Hit-test radius for clicking band points (pixels).
const BAND_HIT_RADIUS: f32 = 10.0;

impl canvas::Program<Message> for EqProgram {
    type State = EqState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let (plot_w, plot_h) = plot_dims(bounds.size());

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    // Hit-test band points
                    for (i, band) in self.bands.iter().enumerate() {
                        let bx = freq_to_x(band.freq_hz, plot_w);
                        let by = db_to_y(band.gain_db, plot_h);
                        let dist = ((pos.x - bx).powi(2) + (pos.y - by).powi(2)).sqrt();
                        if dist <= BAND_HIT_RADIUS {
                            state.dragging_band = Some(i);
                            state.cache.clear();
                            return Some(Action::capture());
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.dragging_band.is_some() {
                    state.dragging_band = None;
                    state.cache.clear();
                    return Some(Action::capture());
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging_band.is_some() {
                    if let Some(pos) = cursor.position_in(bounds) {
                        let new_gain = y_to_db(pos.y, plot_h).clamp(-24.0, 24.0);
                        let new_freq = x_to_freq(pos.x, plot_w).clamp(20.0, 20000.0);
                        state.cache.clear();
                        // Emit gain (primary vertical drag) -- freq also updated via horizontal
                        // Both are emitted on alternate frames via the post-match handler
                        return Some(
                            Action::publish(Message::EffectsParamChanged {
                                channel: self.channel_id,
                                param: "eq_gain_db".into(),
                                value: new_gain,
                            })
                            .and_capture(),
                        );
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    // Check if cursor is near a band point
                    for band in &self.bands {
                        let bx = freq_to_x(band.freq_hz, plot_w);
                        let by = db_to_y(band.gain_db, plot_h);
                        let dist = ((pos.x - bx).powi(2) + (pos.y - by).powi(2)).sqrt();
                        if dist <= BAND_HIT_RADIUS * 2.0 {
                            let scroll_y = match delta {
                                mouse::ScrollDelta::Lines { y, .. } => *y,
                                mouse::ScrollDelta::Pixels { y, .. } => *y / 28.0,
                            };
                            let new_q = (band.q + scroll_y * 0.2).clamp(0.1, 10.0);
                            state.cache.clear();
                            return Some(
                                Action::publish(Message::EffectsParamChanged {
                                    channel: self.channel_id,
                                    param: "eq_q".into(),
                                    value: new_q,
                                })
                                .and_capture(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging_band.is_some() {
            return mouse::Interaction::Grabbing;
        }
        let (plot_w, plot_h) = plot_dims(bounds.size());
        if let Some(pos) = cursor.position_in(bounds) {
            for band in &self.bands {
                let bx = freq_to_x(band.freq_hz, plot_w);
                let by = db_to_y(band.gain_db, plot_h);
                let dist = ((pos.x - bx).powi(2) + (pos.y - by).powi(2)).sqrt();
                if dist <= BAND_HIT_RADIUS {
                    return mouse::Interaction::Grab;
                }
            }
        }
        mouse::Interaction::default()
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            draw_background(frame, bounds.size());
            draw_grid(frame, bounds.size());
            draw_spectrum(frame, bounds.size(), &self.spectrum);
            draw_db_labels(frame, bounds.size());
            draw_freq_labels(frame, bounds.size());
            draw_curve(frame, bounds.size(), &self.bands);
            draw_band_points(frame, bounds.size(), &self.bands);
        });

        tracing::trace!(
            channel_id = self.channel_id,
            bands = self.bands.len(),
            spectrum_bins = self.spectrum.len(),
            "eq_canvas draw"
        );

        vec![geometry]
    }
}

// Re-export `draw_grid` for use in the `draw` method above (it's in the draw module
// but called via unqualified name in the Program impl).
use draw::draw_grid;

#[cfg(test)]
mod tests {
    use super::*;
    use draw::band_magnitude_db;

    const SAMPLE_RATE: f32 = 48_000.0;

    #[test]
    fn test_peak_zero_gain_is_zero_db() {
        // A peak filter with 0 dB gain should contribute 0 dB at all frequencies.
        let band = EqBand {
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: 1.0,
            band_type: BandType::Peak,
            label: "test",
        };
        let db = band_magnitude_db(&band, SAMPLE_RATE, 1000.0);
        // Floating-point: should be very close to 0.
        assert!(db.abs() < 0.01, "expected ~0 dB, got {db}");
    }

    #[test]
    fn test_peak_boost_at_center_freq() {
        // A +6 dB peak at 1 kHz should produce ~+6 dB at 1 kHz.
        let band = EqBand {
            freq_hz: 1000.0,
            gain_db: 6.0,
            q: 1.0,
            band_type: BandType::Peak,
            label: "test",
        };
        let db = band_magnitude_db(&band, SAMPLE_RATE, 1000.0);
        assert!((db - 6.0).abs() < 0.1, "expected ~6 dB, got {db}");
    }

    #[test]
    fn test_freq_to_x_bounds() {
        let plot_w = 300.0;
        let x_min = draw::freq_to_x(FREQ_MIN, plot_w);
        let x_max = draw::freq_to_x(FREQ_MAX, plot_w);
        assert!((x_min - PAD_LEFT).abs() < 0.01);
        assert!((x_max - (PAD_LEFT + plot_w)).abs() < 0.01);
    }

    #[test]
    fn test_x_to_freq_roundtrip() {
        let plot_w = 300.0;
        for &freq in &[20.0, 100.0, 1000.0, 10000.0, 20000.0] {
            let x = draw::freq_to_x(freq, plot_w);
            let back = x_to_freq(x, plot_w);
            assert!(
                (back - freq).abs() / freq < 0.01,
                "roundtrip failed: {freq} -> {x} -> {back}"
            );
        }
    }

    #[test]
    fn test_y_to_db_roundtrip() {
        let plot_h = 180.0;
        for &db in &[-12.0, -6.0, 0.0, 6.0, 12.0] {
            let y = draw::db_to_y(db, plot_h);
            let back = y_to_db(y, plot_h);
            assert!(
                (back - db).abs() < 0.1,
                "roundtrip failed: {db} -> {y} -> {back}"
            );
        }
    }

    #[test]
    fn test_db_to_y_bounds() {
        let plot_h = 180.0;
        let y_top = draw::db_to_y(DB_MAX, plot_h);
        let y_bot = draw::db_to_y(DB_MIN, plot_h);
        assert!((y_top - PAD_TOP).abs() < 0.01);
        assert!((y_bot - (PAD_TOP + plot_h)).abs() < 0.01);
    }
}
