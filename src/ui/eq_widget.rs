//! Parametric EQ visualizer widget.
//!
//! Renders a frequency response curve using iced canvas. Displays the
//! summed biquad magnitude response of the channel's EQ band(s) over a
//! log-scale 20 Hz–20 kHz frequency axis, with a dB grid overlay and
//! band-point markers.

use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::{mouse, Color, Element, Font, Length, Pixels, Point, Rectangle, Size, Theme};

use crate::app::Message;
use crate::effects::EffectsParams;
use crate::plugin::api::ChannelId;
use crate::ui::theme::{ACCENT, BG_ELEVATED, BORDER, TEXT_MUTED};

// --- Frequency / dB constants ---
const FREQ_MIN: f32 = 20.0;
const FREQ_MAX: f32 = 20_000.0;
const DB_MIN: f32 = -12.0;
const DB_MAX: f32 = 12.0;
const SAMPLE_RATE: f32 = 48_000.0;
const CURVE_POINTS: usize = 256;

// Padding inside the canvas bounds for the plot area.
const PAD_LEFT: f32 = 28.0;
const PAD_RIGHT: f32 = 8.0;
const PAD_TOP: f32 = 8.0;
const PAD_BOTTOM: f32 = 20.0;

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

/// The `Program` struct — carries all data needed for drawing.
///
/// State (`EqState`) is kept in the iced widget tree; the program struct
/// itself is re-created each frame from the channel's `EffectsParams`.
pub struct EqProgram {
    channel_id: ChannelId,
    bands: Vec<EqBand>,
}

/// Per-widget mutable state managed by iced.
#[derive(Default)]
pub struct EqState {
    cache: Cache,
}

// --- Public entry point ---

/// Build the EQ canvas element for `channel_id` using `params`.
///
/// Returns an `Element` sized `Length::Fill` × `Length::Fixed(200)`.
pub fn eq_canvas<'a>(channel_id: ChannelId, params: &EffectsParams) -> Element<'a, Message> {
    tracing::trace!(
        channel_id,
        eq_freq = params.eq_freq_hz,
        eq_gain = params.eq_gain_db,
        eq_q = params.eq_q,
        "building eq_canvas"
    );

    let bands = vec![EqBand {
        freq_hz: params.eq_freq_hz,
        gain_db: params.eq_gain_db,
        q: params.eq_q,
        band_type: BandType::Peak,
        label: "Peak",
    }];

    let program = EqProgram { channel_id, bands };

    canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fixed(200.0))
        .into()
}

// --- canvas::Program implementation ---

impl canvas::Program<Message> for EqProgram {
    type State = EqState;

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
            draw_db_labels(frame, bounds.size());
            draw_freq_labels(frame, bounds.size());
            draw_curve(frame, bounds.size(), &self.bands);
            draw_band_points(frame, bounds.size(), &self.bands);
        });

        tracing::trace!(
            channel_id = self.channel_id,
            bands = self.bands.len(),
            "eq_canvas draw"
        );

        vec![geometry]
    }
}

// --- Drawing helpers ---

/// Convert a frequency value (Hz) to an X pixel position within the plot area.
fn freq_to_x(freq: f32, plot_w: f32) -> f32 {
    let t = (freq.max(FREQ_MIN).log10() - FREQ_MIN.log10())
        / (FREQ_MAX.log10() - FREQ_MIN.log10());
    PAD_LEFT + t * plot_w
}

/// Convert a dB value to a Y pixel position within the plot area.
fn db_to_y(db: f32, plot_h: f32) -> f32 {
    let t = (db.clamp(DB_MIN, DB_MAX) - DB_MAX) / (DB_MIN - DB_MAX);
    PAD_TOP + t * plot_h
}

/// Width and height of the inner plot area.
fn plot_dims(size: Size) -> (f32, f32) {
    let w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
    let h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
    (w, h)
}

fn draw_background(frame: &mut Frame, size: Size) {
    frame.fill_rectangle(
        Point::ORIGIN,
        size,
        BG_ELEVATED,
    );
}

fn draw_grid(frame: &mut Frame, size: Size) {
    let (plot_w, plot_h) = plot_dims(size);

    // Frequency grid lines (octave intervals)
    let freq_lines: &[f32] = &[
        31.5, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0,
        8_000.0, 16_000.0,
    ];

    let grid_stroke = Stroke {
        style: canvas::stroke::Style::Solid(BORDER),
        width: 1.0,
        ..Stroke::default()
    };

    for &f in freq_lines {
        let x = freq_to_x(f, plot_w);
        let path = Path::new(|b| {
            b.move_to(Point::new(x, PAD_TOP));
            b.line_to(Point::new(x, PAD_TOP + plot_h));
        });
        frame.stroke(&path, grid_stroke.clone());
    }

    // dB grid lines
    let db_lines: &[f32] = &[-12.0, -6.0, 0.0, 6.0, 12.0];

    for &db in db_lines {
        let y = db_to_y(db, plot_h);
        let zero_line = db == 0.0;

        let color = if zero_line {
            Color { a: 0.5, ..BORDER }
        } else {
            Color { a: 0.35, ..BORDER }
        };

        let path = Path::new(|b| {
            b.move_to(Point::new(PAD_LEFT, y));
            b.line_to(Point::new(PAD_LEFT + plot_w, y));
        });

        frame.stroke(
            &path,
            Stroke {
                style: canvas::stroke::Style::Solid(color),
                width: if zero_line { 1.5 } else { 1.0 },
                ..Stroke::default()
            },
        );
    }
}

fn draw_db_labels(frame: &mut Frame, size: Size) {
    let (_, plot_h) = plot_dims(size);
    let db_labels: &[(f32, &str)] =
        &[(-12.0, "-12"), (-6.0, "-6"), (0.0, "0"), (6.0, "+6"), (12.0, "+12")];

    for &(db, label) in db_labels {
        let y = db_to_y(db, plot_h);
        frame.fill_text(Text {
            content: label.to_string(),
            position: Point::new(PAD_LEFT - 4.0, y),
            color: TEXT_MUTED,
            size: Pixels(9.0),
            font: Font::default(),
            align_x: iced::alignment::Horizontal::Right.into(),
            align_y: iced::alignment::Vertical::Center,
            ..Text::default()
        });
    }
}

fn draw_freq_labels(frame: &mut Frame, size: Size) {
    let (plot_w, plot_h) = plot_dims(size);
    let label_y = PAD_TOP + plot_h + 4.0;

    // Human-readable zone labels placed at their center frequency.
    let zone_labels: &[(&str, f32, f32)] = &[
        ("Sub",      20.0,    60.0),
        ("Bass",     60.0,    250.0),
        ("Warmth",   250.0,   500.0),
        ("Body",     500.0,   2_000.0),
        ("Presence", 2_000.0, 4_000.0),
        ("Clarity",  4_000.0, 8_000.0),
        ("Air",      8_000.0, 20_000.0),
    ];

    for &(name, f_lo, f_hi) in zone_labels {
        let x_lo = freq_to_x(f_lo, plot_w);
        let x_hi = freq_to_x(f_hi, plot_w);
        let x_mid = (x_lo + x_hi) / 2.0;

        frame.fill_text(Text {
            content: name.to_string(),
            position: Point::new(x_mid, label_y),
            color: TEXT_MUTED,
            size: Pixels(8.0),
            font: Font::default(),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Top,
            ..Text::default()
        });
    }
}

fn draw_curve(frame: &mut Frame, size: Size, bands: &[EqBand]) {
    let (plot_w, plot_h) = plot_dims(size);

    // Build 256 log-spaced evaluation frequencies.
    let log_min = FREQ_MIN.log10();
    let log_max = FREQ_MAX.log10();
    let step = (log_max - log_min) / (CURVE_POINTS - 1) as f32;

    let points: Vec<Point> = (0..CURVE_POINTS)
        .map(|i| {
            let eval_freq = 10.0f32.powf(log_min + i as f32 * step);
            let db: f32 = bands
                .iter()
                .map(|b| band_magnitude_db(b, SAMPLE_RATE, eval_freq))
                .sum();
            let x = freq_to_x(eval_freq, plot_w);
            let y = db_to_y(db, plot_h);
            Point::new(x, y)
        })
        .collect();

    if points.is_empty() {
        return;
    }

    let curve = Path::new(|b| {
        b.move_to(points[0]);
        for &pt in &points[1..] {
            b.line_to(pt);
        }
    });

    frame.stroke(
        &curve,
        Stroke {
            style: canvas::stroke::Style::Solid(ACCENT),
            width: 1.5,
            line_cap: canvas::LineCap::Round,
            line_join: canvas::LineJoin::Round,
            ..Stroke::default()
        },
    );
}

fn draw_band_points(frame: &mut Frame, size: Size, bands: &[EqBand]) {
    let (plot_w, plot_h) = plot_dims(size);

    for band in bands {
        let x = freq_to_x(band.freq_hz, plot_w);
        let y = db_to_y(band.gain_db, plot_h);
        let dot = Path::circle(Point::new(x, y), 5.0);
        frame.fill(&dot, ACCENT);

        // Thin white border ring for contrast.
        let ring = Path::circle(Point::new(x, y), 5.0);
        frame.stroke(
            &ring,
            Stroke {
                style: canvas::stroke::Style::Solid(Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.4,
                }),
                width: 1.0,
                ..Stroke::default()
            },
        );
    }
}

// --- DSP: biquad magnitude response ---

/// Compute the magnitude response (dB) of a single EQ band at `eval_freq`.
fn band_magnitude_db(band: &EqBand, sample_rate: f32, eval_freq: f32) -> f32 {
    match band.band_type {
        BandType::Peak => {
            biquad_peak_db(band.freq_hz, band.gain_db, band.q, sample_rate, eval_freq)
        }
        BandType::LowShelf => {
            biquad_shelf_db(band.freq_hz, band.gain_db, band.q, sample_rate, eval_freq, false)
        }
        BandType::HighShelf => {
            biquad_shelf_db(band.freq_hz, band.gain_db, band.q, sample_rate, eval_freq, true)
        }
        BandType::HighPass | BandType::LowPass => {
            // Simple passthrough approximation — no gain for HP/LP in current
            // single-band visualiser (will be extended in a follow-up).
            0.0
        }
    }
}

/// Peak (bell) biquad magnitude response in dB.
///
/// Uses the direct-form II transfer function evaluated on the unit circle.
fn biquad_peak_db(
    freq: f32,
    gain_db: f32,
    q: f32,
    sample_rate: f32,
    eval_freq: f32,
) -> f32 {
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let a = 10.0f32.powf(gain_db / 40.0);
    let alpha = w0.sin() / (2.0 * q.max(0.001));

    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * w0.cos();
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * w0.cos();
    let a2 = 1.0 - alpha / a;

    let w = 2.0 * std::f32::consts::PI * eval_freq / sample_rate;
    let cos_w = w.cos();
    let cos_2w = (2.0 * w).cos();

    let num =
        b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * cos_w
            + 2.0 * b0 * b2 * cos_2w;
    let den =
        a0 * a0 + a1 * a1 + a2 * a2 + 2.0 * (a0 * a1 + a1 * a2) * cos_w
            + 2.0 * a0 * a2 * cos_2w;

    if den <= 0.0 {
        return 0.0;
    }

    10.0 * (num / den).max(f32::EPSILON).log10()
}

/// Low/High shelf biquad magnitude response in dB (Audio EQ Cookbook, R. Bristow-Johnson).
fn biquad_shelf_db(
    freq: f32,
    gain_db: f32,
    _q: f32,
    sample_rate: f32,
    eval_freq: f32,
    high: bool,
) -> f32 {
    let a = 10.0f32.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / 2.0 * (2.0f32).sqrt(); // Q = 1/sqrt(2) Butterworth

    let (b0, b1, b2, a0, a1, a2) = if high {
        // High shelf
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;
        (b0, b1, b2, a0, a1, a2)
    } else {
        // Low shelf
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;
        (b0, b1, b2, a0, a1, a2)
    };

    let w = 2.0 * std::f32::consts::PI * eval_freq / sample_rate;
    let cos_w = w.cos();
    let cos_2w = (2.0 * w).cos();

    let num =
        b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * cos_w
            + 2.0 * b0 * b2 * cos_2w;
    let den =
        a0 * a0 + a1 * a1 + a2 * a2 + 2.0 * (a0 * a1 + a1 * a2) * cos_w
            + 2.0 * a0 * a2 * cos_2w;

    if den <= 0.0 {
        return 0.0;
    }

    10.0 * (num / den).max(f32::EPSILON).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let db = band_magnitude_db(&band, 48_000.0, 1000.0);
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
        let db = band_magnitude_db(&band, 48_000.0, 1000.0);
        assert!((db - 6.0).abs() < 0.1, "expected ~6 dB, got {db}");
    }

    #[test]
    fn test_freq_to_x_bounds() {
        let plot_w = 300.0;
        let x_min = freq_to_x(FREQ_MIN, plot_w);
        let x_max = freq_to_x(FREQ_MAX, plot_w);
        assert!((x_min - PAD_LEFT).abs() < 0.01);
        assert!((x_max - (PAD_LEFT + plot_w)).abs() < 0.01);
    }

    #[test]
    fn test_db_to_y_bounds() {
        let plot_h = 180.0;
        let y_top = db_to_y(DB_MAX, plot_h);
        let y_bot = db_to_y(DB_MIN, plot_h);
        assert!((y_top - PAD_TOP).abs() < 0.01);
        assert!((y_bot - (PAD_TOP + plot_h)).abs() < 0.01);
    }
}
