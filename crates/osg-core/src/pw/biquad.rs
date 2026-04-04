//! Biquad filter coefficient computation and per-sample processing.
//!
//! Based on Robert Bristow-Johnson's Audio EQ Cookbook.
//! Pure math — no PipeWire dependency. Used by the pw_filter process
//! callback (Phase 3+) and testable without a running PW daemon.

use crate::graph::FilterType;

/// Biquad filter coefficients (transfer function numerator/denominator).
#[derive(Debug, Clone, Copy)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a0: f32,
    pub a1: f32,
    pub a2: f32,
}

/// Per-sample biquad filter state (Direct Form I).
#[derive(Debug, Clone, Default)]
pub struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    pub fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Process a single sample through this biquad stage.
    /// Direct Form I: y[n] = (b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]) / a0
    #[inline]
    pub fn process(&mut self, input: f32, c: &Coefficients) -> f32 {
        let output =
            (c.b0 * input + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2)
                / c.a0;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// Compute biquad coefficients for the given filter type (RBJ cookbook).
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn compute_coefficients(
    filter_type: FilterType,
    freq: f32,
    gain_db: f32,
    q: f32,
    sample_rate: f32,
) -> Coefficients {
    let a = 10.0_f32.powf(gain_db / 40.0); // sqrt of linear gain
    let w0 = std::f32::consts::TAU * freq / sample_rate;
    let sin_w0 = w0.sin();
    let cos_w0 = w0.cos();
    let alpha = sin_w0 / (2.0 * q);

    let (b0, b1, b2, a0, a1, a2) = match filter_type {
        FilterType::Peaking => (
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        ),
        FilterType::LowShelf => {
            let sqrt_a = a.sqrt();
            let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
            (
                a * (a + 1.0 - (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
                2.0 * a * (a - 1.0 - (a + 1.0) * cos_w0),
                a * (a + 1.0 - (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
                a + 1.0 + (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
                -2.0 * (a - 1.0 + (a + 1.0) * cos_w0),
                a + 1.0 + (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
            )
        }
        FilterType::HighShelf => {
            let sqrt_a = a.sqrt();
            let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
            (
                a * (a + 1.0 + (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
                -2.0 * a * (a - 1.0 + (a + 1.0) * cos_w0),
                a * (a + 1.0 + (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
                a + 1.0 - (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
                2.0 * (a - 1.0 - (a + 1.0) * cos_w0),
                a + 1.0 - (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
            )
        }
        FilterType::LowPass => (
            (1.0 - cos_w0) / 2.0,
            1.0 - cos_w0,
            (1.0 - cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::HighPass => (
            (1.0 + cos_w0) / 2.0,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
        FilterType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
    };

    Coefficients {
        b0,
        b1,
        b2,
        a0,
        a1,
        a2,
    }
}
