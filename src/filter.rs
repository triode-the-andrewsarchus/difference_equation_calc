use crate::document::{DocumentState, EquationDoc, ParameterDoc};
use crate::measure::{MeasurementSpec, MeasurementType};
use crate::sweep::SweepConfig;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn from_polar(r: f64, theta: f64) -> Self {
        Self {
            re: r * theta.cos(),
            im: r * theta.sin(),
        }
    }

    pub fn mag(self) -> f64 {
        self.re.hypot(self.im)
    }

    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let denom = rhs.re * rhs.re + rhs.im * rhs.im;
        if denom == 0.0 {
            Self::ZERO
        } else {
            Self {
                re: (self.re * rhs.re + self.im * rhs.im) / denom,
                im: (self.im * rhs.re - self.re * rhs.im) / denom,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEngine {
    BiquadDirectForm2,
    ChamberlinSvf,
}

impl FilterEngine {
    pub fn name(self) -> &'static str {
        match self {
            Self::BiquadDirectForm2 => "Direct Form II Biquad",
            Self::ChamberlinSvf => "Chamberlin State Variable (SVF)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

impl FilterType {
    pub const ALL: [Self; 4] = [Self::Lowpass, Self::Highpass, Self::Bandpass, Self::Notch];

    pub fn name(self) -> &'static str {
        match self {
            Self::Lowpass => "Lowpass (2nd Order)",
            Self::Highpass => "Highpass (2nd Order)",
            Self::Bandpass => "Bandpass (2nd Order)",
            Self::Notch => "Notch / Band-Reject",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::Lowpass => "Lowpass",
            Self::Highpass => "Highpass",
            Self::Bandpass => "Bandpass",
            Self::Notch => "Notch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StimulusType {
    Impulse,
    Step,
    SineAndNoise,
    Chirp,
    SquareWave,
}

impl StimulusType {
    pub const ALL: [Self; 5] = [
        Self::Impulse,
        Self::Step,
        Self::SineAndNoise,
        Self::Chirp,
        Self::SquareWave,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Impulse => "Impulse δ[n]",
            Self::Step => "Unit Step u[n]",
            Self::SineAndNoise => "Sine + High-Freq Noise",
            Self::Chirp => "Frequency Chirp Sweep",
            Self::SquareWave => "Square Wave Train",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::Impulse => "Impulse δ",
            Self::Step => "Step u",
            Self::SineAndNoise => "Sine + Noise",
            Self::Chirp => "Chirp",
            Self::SquareWave => "Square",
        }
    }

    pub fn equation_source(self) -> &'static str {
        match self {
            Self::Impulse => "x[n+1] = cond(n == 15, 1.0, 0.0)",
            Self::Step => "x[n+1] = cond(n >= 15, 1.0, 0.0)",
            Self::SineAndNoise => "x[n+1] = sin(0.04 * n) + 0.35 * sin(0.55 * n)",
            Self::Chirp => "x[n+1] = sin(0.0008 * n * n)",
            Self::SquareWave => "x[n+1] = cond(sin(0.06 * n) > 0, 1.0, -1.0)",
        }
    }
}

/// Converts a pole position in the complex Z-plane (re, im) to corresponding (fc, Q)
pub fn pole_to_fc_q(re: f64, im: f64) -> (f64, f64) {
    let r = re.hypot(im).clamp(0.02, 0.998);
    let theta = im.abs().atan2(re);
    let fc = (theta / (2.0 * std::f64::consts::PI)).clamp(0.005, 0.495);
    let q = (-std::f64::consts::PI * fc / r.ln()).clamp(0.2, 30.0);
    (fc, q)
}

#[derive(Debug, Clone, PartialEq)]
pub struct BiquadCoeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl BiquadCoeffs {
    pub fn from_filter_params(filter_type: FilterType, fc: f64, q: f64) -> Self {
        let fc_clamped = fc.clamp(0.001, 0.499);
        let q_clamped = q.clamp(0.1, 30.0);
        let omega0 = 2.0 * std::f64::consts::PI * fc_clamped;
        let alpha = omega0.sin() / (2.0 * q_clamped);
        let cos_w0 = omega0.cos();

        let (b0_raw, b1_raw, b2_raw, a0_raw, a1_raw, a2_raw) = match filter_type {
            FilterType::Lowpass => {
                let b0 = (1.0 - cos_w0) * 0.5;
                let b1 = 1.0 - cos_w0;
                let b2 = (1.0 - cos_w0) * 0.5;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Highpass => {
                let b0 = (1.0 + cos_w0) * 0.5;
                let b1 = -(1.0 + cos_w0);
                let b2 = (1.0 + cos_w0) * 0.5;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Bandpass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        Self {
            b0: b0_raw / a0_raw,
            b1: b1_raw / a0_raw,
            b2: b2_raw / a0_raw,
            a1: a1_raw / a0_raw,
            a2: a2_raw / a0_raw,
        }
    }

    /// Evaluates frequency response H(e^{j omega}) in decibels
    pub fn eval_frequency_response(&self, omega: f64) -> (f64, f64) {
        let ejw = Complex::from_polar(1.0, -omega);
        let ej2w = Complex::from_polar(1.0, -2.0 * omega);

        // Numerator B(z) = b0 + b1 z^-1 + b2 z^-2
        let num = Complex::new(self.b0, 0.0)
            + Complex::new(self.b1, 0.0) * ejw
            + Complex::new(self.b2, 0.0) * ej2w;

        // Denominator A(z) = 1 + a1 z^-1 + a2 z^-2
        let den =
            Complex::ONE + Complex::new(self.a1, 0.0) * ejw + Complex::new(self.a2, 0.0) * ej2w;

        let h = num / den;
        let mag = h.mag();
        let mag_db = if mag > 1e-6 {
            20.0 * mag.log10().clamp(-5.0, 2.0)
        } else {
            -100.0
        };

        (mag_db, h.arg())
    }

    /// Solves for the conjugate pole pair of A(z) = 1 + a1 z^-1 + a2 z^-2 = 0
    /// Roots of z^2 + a1 z + a2 = 0
    pub fn poles(&self) -> (Complex, Complex) {
        let disc = self.a1 * self.a1 - 4.0 * self.a2;
        if disc >= 0.0 {
            let r1 = (-self.a1 + disc.sqrt()) * 0.5;
            let r2 = (-self.a1 - disc.sqrt()) * 0.5;
            (Complex::new(r1, 0.0), Complex::new(r2, 0.0))
        } else {
            let real = -self.a1 * 0.5;
            let imag = (-disc).sqrt() * 0.5;
            (Complex::new(real, imag), Complex::new(real, -imag))
        }
    }

    /// Solves for the conjugate zero pair of B(z) = b0 z^2 + b1 z + b2 = 0
    pub fn zeros(&self) -> (Complex, Complex) {
        if self.b0.abs() < 1e-9 {
            return (Complex::ZERO, Complex::ZERO);
        }
        let disc = self.b1 * self.b1 - 4.0 * self.b0 * self.b2;
        if disc >= 0.0 {
            let r1 = (-self.b1 + disc.sqrt()) / (2.0 * self.b0);
            let r2 = (-self.b1 - disc.sqrt()) / (2.0 * self.b0);
            (Complex::new(r1, 0.0), Complex::new(r2, 0.0))
        } else {
            let real = -self.b1 / (2.0 * self.b0);
            let imag = (-disc).sqrt() / (2.0 * self.b0);
            (Complex::new(real, imag), Complex::new(real, -imag))
        }
    }
}

/// Generates a difference equation system document for a standard digital biquad filter
pub fn create_biquad_filter_doc(
    filter_type: FilterType,
    fc: f64,
    q: f64,
    stimulus: StimulusType,
) -> DocumentState {
    let coeffs = BiquadCoeffs::from_filter_params(filter_type, fc, q);

    let equations = vec![
        EquationDoc {
            id: 1,
            source: stimulus.equation_source().to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "y[n+1] = b0 * x[n+1] + b1 * x[n] + b2 * x[n-1] - a1 * y[n] - a2 * y[n-1]"
                .to_string(),
            color_idx: 1,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.0);
    initial_conditions.insert("y".to_string(), 0.0);

    let parameters = vec![
        ParameterDoc {
            name: "b0".to_string(),
            value: (coeffs.b0 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        },
        ParameterDoc {
            name: "b1".to_string(),
            value: (coeffs.b1 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        },
        ParameterDoc {
            name: "b2".to_string(),
            value: (coeffs.b2 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        },
        ParameterDoc {
            name: "a1".to_string(),
            value: (coeffs.a1 * 10000.0).round() / 10000.0,
            min: -2.5,
            max: 2.5,
            step: 0.001,
        },
        ParameterDoc {
            name: "a2".to_string(),
            value: (coeffs.a2 * 10000.0).round() / 10000.0,
            min: -1.5,
            max: 1.5,
            step: 0.001,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 500,
        selection: Some((15, 120)),
        search_query: "y[n] > 0.5".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "a1".to_string(),
            measurement: MeasurementSpec::new("y", MeasurementType::Max),
            min_val: -1.9,
            max_val: -0.2,
            steps: 120,
        },
    }
}

/// Generates a Chamberlin State Variable Filter (SVF) with simultaneous lowpass, bandpass, highpass outputs
pub fn create_chamberlin_svf_doc(fc: f64, q: f64, stimulus: StimulusType) -> DocumentState {
    let f_val = 2.0 * (std::f64::consts::PI * fc.clamp(0.01, 0.45)).sin();
    let q_damp = 1.0 / q.clamp(0.2, 20.0);

    let equations = vec![
        EquationDoc {
            id: 1,
            source: stimulus.equation_source().to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "lp[n+1] = lp[n] + f * bp[n]".to_string(),
            color_idx: 1,
        },
        EquationDoc {
            id: 3,
            source: "hp[n+1] = x[n+1] - lp[n+1] - q * bp[n]".to_string(),
            color_idx: 2,
        },
        EquationDoc {
            id: 4,
            source: "bp[n+1] = bp[n] + f * hp[n+1]".to_string(),
            color_idx: 3,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.0);
    initial_conditions.insert("lp".to_string(), 0.0);
    initial_conditions.insert("hp".to_string(), 0.0);
    initial_conditions.insert("bp".to_string(), 0.0);

    let parameters = vec![
        ParameterDoc {
            name: "f".to_string(),
            value: (f_val * 1000.0).round() / 1000.0,
            min: 0.02,
            max: 0.8,
            step: 0.005,
        },
        ParameterDoc {
            name: "q".to_string(),
            value: (q_damp * 1000.0).round() / 1000.0,
            min: 0.05,
            max: 2.0,
            step: 0.01,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 600,
        selection: Some((15, 150)),
        search_query: "lp[n+1] > lp[n]".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "f".to_string(),
            measurement: MeasurementSpec::new("lp", MeasurementType::Rms),
            min_val: 0.05,
            max_val: 0.6,
            steps: 120,
        },
    }
}
