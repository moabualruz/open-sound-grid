//! EQ drawing helpers and DSP magnitude-response functions.

use iced::widget::canvas::{self, Frame, Path, Stroke, Text};
use iced::{Color, Font, Pixels, Point, Size};

use crate::effects::EffectsParams;
use crate::ui::theme::{ACCENT, BG_ELEVATED, BORDER, TEXT_MUTED};

use super::{
    BandType, EqBand, DB_MAX, DB_MIN, FREQ_MAX, FREQ_MIN, PAD_BOTTOM, PAD_LEFT, PAD_RIGHT,
    PAD_TOP,
};

const SAMPLE_RATE: f32 = 48_000.0;
const CURVE_POINTS: usize = 256;

// --- Coordinate conversions ---

/// Convert a frequency value (Hz) to an X pixel position within the plot area.
pub(crate) fn freq_to_x(freq: f32, plot_w: f32) -> f32 {
    let t =
        (freq.max(FREQ_MIN).log10() - FREQ_MIN.log10()) / (FREQ_MAX.log10() - FREQ_MIN.log10());
    PAD_LEFT + t * plot_w
}

/// Convert a dB value to a Y pixel position within the plot area.
pub(crate) fn db_to_y(db: f32, plot_h: f32) -> f32 {
    let t = (db.clamp(DB_MIN, DB_MAX) - DB_MAX) / (DB_MIN - DB_MAX);
    PAD_TOP + t * plot_h
}

/// Width and height of the inner plot area.
pub(crate) fn plot_dims(size: Size) -> (f32, f32) {
    let w = (size.width - PAD_LEFT - PAD_RIGHT).max(1.0);
    let h = (size.height - PAD_TOP - PAD_BOTTOM).max(1.0);
    (w, h)
}

// --- Drawing functions ---

pub(crate) fn draw_background(frame: &mut Frame, size: Size) {
    frame.fill_rectangle(Point::ORIGIN, size, BG_ELEVATED);
}

pub(crate) fn draw_grid(frame: &mut Frame, size: Size) {
    let (plot_w, plot_h) = plot_dims(size);

    // Frequency grid lines (octave intervals)
    let freq_lines: &[f32] = &[
        31.5, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
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

pub(crate) fn draw_db_labels(frame: &mut Frame, size: Size) {
    let (_, plot_h) = plot_dims(size);
    let db_labels: &[(f32, &str)] = &[
        (-12.0, "-12"),
        (-6.0, "-6"),
        (0.0, "0"),
        (6.0, "+6"),
        (12.0, "+12"),
    ];

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

pub(crate) fn draw_freq_labels(frame: &mut Frame, size: Size) {
    let (plot_w, plot_h) = plot_dims(size);
    let label_y = PAD_TOP + plot_h + 4.0;

    // Human-readable zone labels placed at their center frequency.
    let zone_labels: &[(&str, f32, f32)] = &[
        ("Sub", 20.0, 60.0),
        ("Bass", 60.0, 250.0),
        ("Warmth", 250.0, 500.0),
        ("Body", 500.0, 2_000.0),
        ("Presence", 2_000.0, 4_000.0),
        ("Clarity", 4_000.0, 8_000.0),
        ("Air", 8_000.0, 20_000.0),
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

pub(crate) fn draw_curve(frame: &mut Frame, size: Size, bands: &[EqBand]) {
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

pub(crate) fn draw_band_points(frame: &mut Frame, size: Size, bands: &[EqBand]) {
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

// --- Spectrum overlay ---

/// Spectrum dB floor used for the y-axis mapping in `draw_spectrum`.
const SPECTRUM_DB_FLOOR: f32 = -80.0;
/// Spectrum dB ceiling (0 dB = full signal).
const SPECTRUM_DB_CEIL: f32 = 0.0;

/// Generate a simulated spectrum for display purposes.
/// In v0.4, this will be replaced with real FFT data from PA stream capture.
pub(crate) fn simulated_spectrum(_params: &EffectsParams) -> Vec<(f32, f32)> {
    let mut bins = Vec::with_capacity(128);
    for i in 0..128 {
        let t = i as f32 / 127.0;
        let freq = 20.0 * (20000.0_f32 / 20.0).powf(t);
        // Base noise floor at -60 dB, rolling off at high frequencies.
        let level = -60.0 + 20.0 * (1.0 - t) + 5.0 * ((freq * 0.01).sin());
        bins.push((freq, level.max(SPECTRUM_DB_FLOOR).min(SPECTRUM_DB_CEIL)));
    }
    bins
}

/// Map a spectrum dB value to a Y pixel position.
///
/// `SPECTRUM_DB_FLOOR` -> bottom of plot; `SPECTRUM_DB_CEIL` -> top of plot.
fn spectrum_db_to_y(db: f32, plot_h: f32) -> f32 {
    let t = (db.clamp(SPECTRUM_DB_FLOOR, SPECTRUM_DB_CEIL) - SPECTRUM_DB_CEIL)
        / (SPECTRUM_DB_FLOOR - SPECTRUM_DB_CEIL);
    PAD_TOP + t * plot_h
}

/// Draw a filled spectrum overlay behind the EQ curve.
pub(crate) fn draw_spectrum(frame: &mut Frame, size: Size, bins: &[(f32, f32)]) {
    if bins.is_empty() {
        return;
    }

    let (plot_w, plot_h) = plot_dims(size);
    let bottom_y = PAD_TOP + plot_h;
    let fill_color = Color { a: 0.15, ..ACCENT };

    let path = Path::new(|b| {
        // Start at bottom-left of plot.
        b.move_to(Point::new(PAD_LEFT, bottom_y));

        for &(freq, db) in bins {
            let x = freq_to_x(freq, plot_w);
            let y = spectrum_db_to_y(db, plot_h);
            b.line_to(Point::new(x, y));
        }

        // Close back to bottom-right then bottom-left.
        if let Some(&(last_freq, _)) = bins.last() {
            b.line_to(Point::new(freq_to_x(last_freq, plot_w), bottom_y));
        }
        b.close();
    });

    frame.fill(&path, fill_color);
}

// --- DSP: biquad magnitude response ---

/// Compute the magnitude response (dB) of a single EQ band at `eval_freq`.
pub(crate) fn band_magnitude_db(band: &EqBand, sample_rate: f32, eval_freq: f32) -> f32 {
    match band.band_type {
        BandType::Peak => {
            biquad_peak_db(band.freq_hz, band.gain_db, band.q, sample_rate, eval_freq)
        }
        BandType::LowShelf => biquad_shelf_db(
            band.freq_hz,
            band.gain_db,
            band.q,
            sample_rate,
            eval_freq,
            false,
        ),
        BandType::HighShelf => biquad_shelf_db(
            band.freq_hz,
            band.gain_db,
            band.q,
            sample_rate,
            eval_freq,
            true,
        ),
        BandType::HighPass | BandType::LowPass => {
            // Simple passthrough approximation -- no gain for HP/LP in current
            // single-band visualiser (will be extended in a follow-up).
            0.0
        }
    }
}

/// Peak (bell) biquad magnitude response in dB.
///
/// Uses the direct-form II transfer function evaluated on the unit circle.
fn biquad_peak_db(freq: f32, gain_db: f32, q: f32, sample_rate: f32, eval_freq: f32) -> f32 {
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

    let num = b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * cos_w
        + 2.0 * b0 * b2 * cos_2w;
    let den = a0 * a0 + a1 * a1 + a2 * a2 + 2.0 * (a0 * a1 + a1 * a2) * cos_w
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

    let num = b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * cos_w
        + 2.0 * b0 * b2 * cos_2w;
    let den = a0 * a0 + a1 * a1 + a2 * a2 + 2.0 * (a0 * a1 + a1 * a2) * cos_w
        + 2.0 * a0 * a2 * cos_2w;

    if den <= 0.0 {
        return 0.0;
    }

    10.0 * (num / den).max(f32::EPSILON).log10()
}
