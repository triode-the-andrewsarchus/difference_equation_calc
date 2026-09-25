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

impl std::ops::Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl std::ops::Div<f64> for Complex {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self {
            re: self.re / rhs,
            im: self.im / rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEngine {
    BiquadDirectForm2,
    ChamberlinSvf,
    WindowedSincFir,
    MoogLadder,
}

impl FilterEngine {
    pub const ALL: [Self; 4] = [
        Self::BiquadDirectForm2,
        Self::ChamberlinSvf,
        Self::WindowedSincFir,
        Self::MoogLadder,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BiquadDirectForm2 => "Direct Form II Biquad",
            Self::ChamberlinSvf => "Chamberlin State Variable (SVF)",
            Self::WindowedSincFir => "Windowed Sinc FIR Filter",
            Self::MoogLadder => "Moog 24dB Transistor Ladder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirWindow {
    Rectangular,
    Hann,
    Hamming,
    Blackman,
}

impl FirWindow {
    pub const ALL: [Self; 4] = [Self::Rectangular, Self::Hann, Self::Hamming, Self::Blackman];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangular => "Rectangular (Truncated)",
            Self::Hann => "Hann (Smooth)",
            Self::Hamming => "Hamming (Classic Audio)",
            Self::Blackman => "Blackman (Deep Rejection)",
        }
    }

    pub fn weight(self, i: usize, m: usize) -> f64 {
        if m <= 1 {
            return 1.0;
        }
        let n = i as f64;
        let denom = (m - 1) as f64;
        match self {
            Self::Rectangular => 1.0,
            Self::Hann => 0.5 - 0.5 * (2.0 * std::f64::consts::PI * n / denom).cos(),
            Self::Hamming => 0.54 - 0.46 * (2.0 * std::f64::consts::PI * n / denom).cos(),
            Self::Blackman => {
                0.42 - 0.5 * (2.0 * std::f64::consts::PI * n / denom).cos()
                    + 0.08 * (4.0 * std::f64::consts::PI * n / denom).cos()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonLinearityKind {
    Clean,
    Tanh,
    HardClip,
    Wavefold,
    SoftAlgebraic,
    AsymmetricHarmonic,
    Bitcrush,
}

impl NonLinearityKind {
    pub const ALL: [Self; 7] = [
        Self::Clean,
        Self::Tanh,
        Self::HardClip,
        Self::Wavefold,
        Self::SoftAlgebraic,
        Self::AsymmetricHarmonic,
        Self::Bitcrush,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Clean => "Clean (Linear)",
            Self::Tanh => "Tanh (Warm Tape/Tube)",
            Self::HardClip => "Hard Clip (Diode/Rail)",
            Self::Wavefold => "Wavefolder (Buchla Sine)",
            Self::SoftAlgebraic => "Soft Drive (u / (1 + |u|))",
            Self::AsymmetricHarmonic => "Asymmetric (2nd Harm)",
            Self::Bitcrush => "Bitcrush (Lo-Fi Crunch)",
        }
    }

    pub fn wrap_expr(self, inner: &str, drive: f64) -> String {
        let drive_str = if (drive - 1.0).abs() < 1e-4 {
            "".to_string()
        } else {
            format!("{:.3} * ", drive)
        };
        match self {
            Self::Clean => inner.to_string(),
            Self::Tanh => format!("tanh({drive_str}{inner})"),
            Self::HardClip => format!("clamp({drive_str}{inner}, -1.0, 1.0)"),
            Self::Wavefold => format!("sin({drive_str}({inner}) * 3.14159)"),
            Self::SoftAlgebraic => {
                format!("(({drive_str}({inner})) / (1.0 + abs({drive_str}({inner}))))")
            }
            Self::AsymmetricHarmonic => {
                format!("(tanh({drive_str}({inner})) + 0.25 * tanh(({drive_str}({inner})) * ({drive_str}({inner}))))")
            }
            Self::Bitcrush => {
                format!("(floor(({drive_str}({inner})) * 6.0) / 6.0)")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonLinearPlacement {
    FeedbackLoop,
    BetweenStages,
    Output,
}

impl NonLinearPlacement {
    pub const ALL: [Self; 3] = [Self::FeedbackLoop, Self::BetweenStages, Self::Output];

    pub fn name(self) -> &'static str {
        match self {
            Self::FeedbackLoop => "Feedback Loop (Limit Cycle)",
            Self::BetweenStages => "Between Poles (Ladder Stages)",
            Self::Output => "Post Output",
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
pub fn pole_to_fc_q_ext(re: f64, im: f64, allow_self_osc: bool) -> (f64, f64) {
    let r_max = if allow_self_osc { 1.25 } else { 0.998 };
    let r = re.hypot(im).clamp(0.02, r_max);
    let theta = im.abs().atan2(re);
    let fc = (theta / (2.0 * std::f64::consts::PI)).clamp(0.005, 0.495);
    let q = if r >= 1.0 {
        // Self-oscillation zone: maps r in [1.0, 1.25] to Q in [25.0, 60.0]
        25.0 + (r - 1.0) * 140.0
    } else {
        // Stable zone: maps r in [0.02, 0.998] to Q in [0.2, 25.0]
        (-std::f64::consts::PI * fc / r.ln()).clamp(0.2, 25.0)
    };
    (fc, q)
}

pub fn pole_to_fc_q(re: f64, im: f64) -> (f64, f64) {
    pole_to_fc_q_ext(re, im, false)
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
        Self::from_filter_params_ext(filter_type, fc, q, false)
    }

    pub fn from_filter_params_ext(
        filter_type: FilterType,
        fc: f64,
        q: f64,
        allow_self_osc: bool,
    ) -> Self {
        let fc_clamped = fc.clamp(0.001, 0.499);
        let omega0 = 2.0 * std::f64::consts::PI * fc_clamped;
        let cos_w0 = omega0.cos();

        if allow_self_osc && q >= 25.0 {
            // Unstable / Self-oscillating regime: pole radius r >= 1.0
            let r = (1.0 + (q - 25.0) * (0.25 / 35.0)).clamp(1.0, 1.25);
            let a1 = -2.0 * r * cos_w0;
            let a2 = r * r;

            let (b0, b1, b2) = match filter_type {
                FilterType::Lowpass => {
                    let b0 = (1.0 + a1 + a2).abs().max(0.02) * 0.25;
                    (b0, 2.0 * b0, b0)
                }
                FilterType::Highpass => {
                    let b0 = (1.0 - a1 + a2).abs().max(0.02) * 0.25;
                    (b0, -2.0 * b0, b0)
                }
                FilterType::Bandpass => {
                    let b0 = ((r * r - 1.0).abs() * 0.5 + 0.1).clamp(0.02, 1.0);
                    (b0, 0.0, -b0)
                }
                FilterType::Notch => {
                    let b0 = 1.0;
                    let b1 = -2.0 * cos_w0;
                    let b2 = 1.0;
                    (b0, b1, b2)
                }
            };

            Self { b0, b1, b2, a1, a2 }
        } else {
            // Standard stable RBJ audio cookbook
            let q_clamped = q.clamp(0.1, 25.0);
            let alpha = omega0.sin() / (2.0 * q_clamped);

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

/// Multi-stage cascaded filter supporting 2, 4, 6, and 8 poles
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeFilter {
    pub filter_type: FilterType,
    pub pole_count: usize,
    pub fc: f64,
    pub q: f64,
    pub allow_self_oscillation: bool,
    pub stages: Vec<BiquadCoeffs>,
}

impl CascadeFilter {
    pub fn new(
        filter_type: FilterType,
        pole_count: usize,
        fc: f64,
        q: f64,
        allow_self_osc: bool,
    ) -> Self {
        let pole_count = match pole_count {
            2 | 4 | 6 | 8 => pole_count,
            _ => 2,
        };
        let num_stages = pole_count / 2;
        let mut stages = Vec::with_capacity(num_stages);

        if num_stages == 1 {
            let coeffs = BiquadCoeffs::from_filter_params_ext(filter_type, fc, q, allow_self_osc);
            stages.push(coeffs);
        } else {
            // Butterworth pole distribution across stages
            let theta_max =
                (std::f64::consts::PI / (2.0 * pole_count as f64)) * (2 * num_stages - 1) as f64;
            let q_butter_max = 1.0 / (2.0 * theta_max.cos().max(1e-4));
            let q_ratio = q / q_butter_max;

            for k in 1..=num_stages {
                let theta_k =
                    (std::f64::consts::PI / (2.0 * pole_count as f64)) * (2 * k - 1) as f64;
                let q_butter_k = 1.0 / (2.0 * theta_k.cos().max(1e-4));
                let is_resonant_stage = k == num_stages;

                let stage_q = (q_butter_k * q_ratio).max(0.1);
                let coeffs = if is_resonant_stage {
                    BiquadCoeffs::from_filter_params_ext(filter_type, fc, stage_q, allow_self_osc)
                } else {
                    BiquadCoeffs::from_filter_params_ext(filter_type, fc, stage_q, false)
                };
                stages.push(coeffs);
            }
        }

        Self {
            filter_type,
            pole_count,
            fc,
            q,
            allow_self_oscillation: allow_self_osc,
            stages,
        }
    }

    pub fn eval_frequency_response(&self, omega: f64) -> (f64, f64) {
        let mut total_db = 0.0;
        let mut total_phase = 0.0;
        for stage in &self.stages {
            let (db, phase) = stage.eval_frequency_response(omega);
            total_db += db;
            total_phase += phase;
        }
        (total_db.clamp(-120.0, 40.0), total_phase)
    }

    pub fn all_poles(&self) -> Vec<(Complex, Complex)> {
        self.stages.iter().map(|s| s.poles()).collect()
    }

    pub fn all_zeros(&self) -> Vec<(Complex, Complex)> {
        self.stages.iter().map(|s| s.zeros()).collect()
    }

    pub fn max_pole_mag(&self) -> f64 {
        let mut max_mag = 0.0_f64;
        for stage in &self.stages {
            let (p1, p2) = stage.poles();
            max_mag = max_mag.max(p1.mag()).max(p2.mag());
        }
        max_mag
    }
}

pub fn compute_fir_coeffs(
    filter_type: FilterType,
    num_taps: usize,
    fc: f64,
    window: FirWindow,
) -> Vec<f64> {
    let m = num_taps.clamp(3, 64);
    let center = (m - 1) as f64 / 2.0;
    let fc = fc.clamp(0.01, 0.49);

    let mut h = Vec::with_capacity(m);

    match filter_type {
        FilterType::Lowpass => {
            let mut sum = 0.0;
            for i in 0..m {
                let k = i as f64 - center;
                let sinc = if k.abs() < 1e-8 {
                    2.0 * fc
                } else {
                    (2.0 * std::f64::consts::PI * fc * k).sin() / (std::f64::consts::PI * k)
                };
                let val = sinc * window.weight(i, m);
                sum += val;
                h.push(val);
            }
            if sum.abs() > 1e-6 {
                for v in &mut h {
                    *v /= sum;
                }
            }
        }
        FilterType::Highpass => {
            let mut lp = compute_fir_coeffs(FilterType::Lowpass, m, fc, window);
            for (i, val) in lp.iter_mut().enumerate() {
                let delta = if (i as f64 - center).abs() < 0.5 {
                    1.0
                } else {
                    0.0
                };
                *val = delta - *val;
            }
            let mut nyq_gain = 0.0;
            for (i, &v) in lp.iter().enumerate() {
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                nyq_gain += v * sign;
            }
            if nyq_gain.abs() > 1e-6 {
                for v in &mut lp {
                    *v /= nyq_gain.abs();
                }
            }
            h = lp;
        }
        FilterType::Bandpass => {
            let bw = (fc * 0.5).clamp(0.02, 0.3);
            let f1 = (fc - bw * 0.5).clamp(0.01, 0.47);
            let f2 = (fc + bw * 0.5).clamp(f1 + 0.01, 0.49);
            let lp1 = compute_fir_coeffs(FilterType::Lowpass, m, f1, window);
            let lp2 = compute_fir_coeffs(FilterType::Lowpass, m, f2, window);
            for i in 0..m {
                h.push(lp2[i] - lp1[i]);
            }
            let omega_c = 2.0 * std::f64::consts::PI * fc;
            let mut re = 0.0;
            let mut im = 0.0;
            for (k, &coeff) in h.iter().enumerate() {
                re += coeff * (omega_c * k as f64).cos();
                im -= coeff * (omega_c * k as f64).sin();
            }
            let mag = re.hypot(im);
            if mag > 1e-6 {
                for v in &mut h {
                    *v /= mag;
                }
            }
        }
        FilterType::Notch => {
            let bp = compute_fir_coeffs(FilterType::Bandpass, m, fc, window);
            for (i, &v) in bp.iter().enumerate() {
                let delta = if (i as f64 - center).abs() < 0.5 {
                    1.0
                } else {
                    0.0
                };
                h.push(delta - v);
            }
        }
    }

    h
}

pub fn eval_fir_frequency_response(coeffs: &[f64], omega: f64) -> (f64, f64) {
    let mut re = 0.0;
    let mut im = 0.0;
    for (k, &c) in coeffs.iter().enumerate() {
        let angle = omega * k as f64;
        re += c * angle.cos();
        im -= c * angle.sin();
    }
    let mag = re.hypot(im);
    let db = (20.0 * (mag + 1e-12).log10()).clamp(-120.0, 40.0);
    let phase = im.atan2(re);
    (db, phase)
}

pub fn eval_moog_frequency_response(fc: f64, r: f64, omega: f64) -> (f64, f64) {
    let f = (2.0 * std::f64::consts::PI * fc.clamp(0.01, 0.45)).clamp(0.02, 0.85);
    let z = Complex::from_polar(1.0, omega);
    let denom_1 = z - Complex::new(1.0 - f, 0.0);
    let h1 = Complex::new(f, 0.0) / denom_1;
    let h4 = h1 * h1 * h1 * h1;
    let fb_denom = Complex::ONE + Complex::new(4.0 * r, 0.0) * h4;
    let h_cl = h4 / fb_denom;

    let mag = h_cl.mag();
    let db = (20.0 * (mag + 1e-12).log10()).clamp(-120.0, 40.0);
    (db, h_cl.arg())
}

/// Generates a difference equation system document for a standard or multi-pole cascade biquad filter
pub fn create_cascade_biquad_doc(
    filter_type: FilterType,
    pole_count: usize,
    fc: f64,
    q: f64,
    stimulus: StimulusType,
    allow_self_osc: bool,
    soft_saturation: bool,
) -> DocumentState {
    let nl = if soft_saturation {
        NonLinearityKind::Tanh
    } else {
        NonLinearityKind::HardClip
    };
    create_cascade_biquad_doc_ext(
        filter_type,
        pole_count,
        fc,
        q,
        stimulus,
        allow_self_osc,
        nl,
        1.0,
        NonLinearPlacement::Output,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_cascade_biquad_doc_ext(
    filter_type: FilterType,
    pole_count: usize,
    fc: f64,
    q: f64,
    stimulus: StimulusType,
    allow_self_osc: bool,
    non_linear: NonLinearityKind,
    drive: f64,
    placement: NonLinearPlacement,
) -> DocumentState {
    let cascade = CascadeFilter::new(filter_type, pole_count, fc, q, allow_self_osc);
    let num_stages = cascade.stages.len();

    let mut equations = Vec::new();
    let mut initial_conditions = HashMap::new();
    let mut parameters = Vec::new();

    // 1. Stimulus equation
    equations.push(EquationDoc {
        id: 1,
        source: stimulus.equation_source().to_string(),
        color_idx: 0,
    });
    initial_conditions.insert("x".to_string(), 0.0);

    if num_stages == 1 {
        let c = &cascade.stages[0];
        let eq_src = match placement {
            NonLinearPlacement::FeedbackLoop if non_linear != NonLinearityKind::Clean => {
                let fb = non_linear.wrap_expr("a1 * y[n] + a2 * y[n-1]", drive);
                format!(
                    "y[n+1] = clamp(b0 * x[n+1] + b1 * x[n] + b2 * x[n-1] - {}, -1.5, 1.5)",
                    fb
                )
            }
            _ => {
                let rhs_inner = "b0 * x[n+1] + b1 * x[n] + b2 * x[n-1] - a1 * y[n] - a2 * y[n-1]";
                let wrapped = non_linear.wrap_expr(rhs_inner, drive);
                format!("y[n+1] = {wrapped}")
            }
        };

        equations.push(EquationDoc {
            id: 2,
            source: eq_src,
            color_idx: 1,
        });
        initial_conditions.insert("y".to_string(), 0.0);

        parameters.push(ParameterDoc {
            name: "b0".to_string(),
            value: (c.b0 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        });
        parameters.push(ParameterDoc {
            name: "b1".to_string(),
            value: (c.b1 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        });
        parameters.push(ParameterDoc {
            name: "b2".to_string(),
            value: (c.b2 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        });
        parameters.push(ParameterDoc {
            name: "a1".to_string(),
            value: (c.a1 * 10000.0).round() / 10000.0,
            min: -2.5,
            max: 2.5,
            step: 0.001,
        });
        parameters.push(ParameterDoc {
            name: "a2".to_string(),
            value: (c.a2 * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        });
    } else {
        let mut prev_var = "x".to_string();
        for k in 0..num_stages {
            let is_final = k == num_stages - 1;
            let stage_num = k + 1;
            let curr_var = if is_final {
                "y".to_string()
            } else {
                format!("y{}", stage_num)
            };
            let c = &cascade.stages[k];

            let b0_name = format!("b0_{}", stage_num);
            let b1_name = format!("b1_{}", stage_num);
            let b2_name = format!("b2_{}", stage_num);
            let a1_name = format!("a1_{}", stage_num);
            let a2_name = format!("a2_{}", stage_num);

            let eq_src = match placement {
                NonLinearPlacement::FeedbackLoop if non_linear != NonLinearityKind::Clean => {
                    let fb = non_linear.wrap_expr(
                        &format!(
                            "{} * {}[n] + {} * {}[n-1]",
                            a1_name, curr_var, a2_name, curr_var
                        ),
                        drive,
                    );
                    format!(
                        "{}[n+1] = clamp({} * {}[n+1] + {} * {}[n] + {} * {}[n-1] - {}, -1.5, 1.5)",
                        curr_var, b0_name, prev_var, b1_name, prev_var, b2_name, prev_var, fb
                    )
                }
                NonLinearPlacement::BetweenStages
                    if !is_final && non_linear != NonLinearityKind::Clean =>
                {
                    let rhs_core = format!(
                        "{} * {}[n+1] + {} * {}[n] + {} * {}[n-1] - {} * {}[n] - {} * {}[n-1]",
                        b0_name,
                        prev_var,
                        b1_name,
                        prev_var,
                        b2_name,
                        prev_var,
                        a1_name,
                        curr_var,
                        a2_name,
                        curr_var
                    );
                    let wrapped = non_linear.wrap_expr(&rhs_core, drive);
                    format!("{}[n+1] = {wrapped}", curr_var)
                }
                _ => {
                    let rhs_core = format!(
                        "{} * {}[n+1] + {} * {}[n] + {} * {}[n-1] - {} * {}[n] - {} * {}[n-1]",
                        b0_name,
                        prev_var,
                        b1_name,
                        prev_var,
                        b2_name,
                        prev_var,
                        a1_name,
                        curr_var,
                        a2_name,
                        curr_var
                    );
                    let wrapped = non_linear.wrap_expr(&rhs_core, drive);
                    format!("{}[n+1] = {wrapped}", curr_var)
                }
            };

            equations.push(EquationDoc {
                id: (k + 2) as u64,
                source: eq_src,
                color_idx: ((k + 1) % 6),
            });
            initial_conditions.insert(curr_var.clone(), 0.0);

            parameters.push(ParameterDoc {
                name: b0_name,
                value: (c.b0 * 10000.0).round() / 10000.0,
                min: -2.0,
                max: 2.0,
                step: 0.001,
            });
            parameters.push(ParameterDoc {
                name: b1_name,
                value: (c.b1 * 10000.0).round() / 10000.0,
                min: -2.0,
                max: 2.0,
                step: 0.001,
            });
            parameters.push(ParameterDoc {
                name: b2_name,
                value: (c.b2 * 10000.0).round() / 10000.0,
                min: -2.0,
                max: 2.0,
                step: 0.001,
            });
            parameters.push(ParameterDoc {
                name: a1_name,
                value: (c.a1 * 10000.0).round() / 10000.0,
                min: -2.5,
                max: 2.5,
                step: 0.001,
            });
            parameters.push(ParameterDoc {
                name: a2_name,
                value: (c.a2 * 10000.0).round() / 10000.0,
                min: -2.0,
                max: 2.0,
                step: 0.001,
            });

            prev_var = curr_var;
        }
    }

    if non_linear != NonLinearityKind::Clean {
        parameters.push(ParameterDoc {
            name: "drive".to_string(),
            value: drive,
            min: 0.2,
            max: 5.0,
            step: 0.05,
        });
    }

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 500,
        selection: Some((15, 120)),
        search_query: "y[n] > 0.5".to_string(),
        sweep_config: SweepConfig {
            parameter_name: if num_stages == 1 {
                "a1".to_string()
            } else {
                "a1_1".to_string()
            },
            measurement: MeasurementSpec::new("y", MeasurementType::Max),
            min_val: -1.9,
            max_val: -0.2,
            steps: 120,
        },
        clamping_mode: crate::document::ClampingMode::Unit,
    }
}

pub fn create_biquad_filter_doc(
    filter_type: FilterType,
    fc: f64,
    q: f64,
    stimulus: StimulusType,
) -> DocumentState {
    create_cascade_biquad_doc(filter_type, 2, fc, q, stimulus, false, false)
}

/// Generates a Chamberlin State Variable Filter (SVF) with simultaneous lowpass, bandpass, highpass outputs
pub fn create_cascade_svf_doc(
    pole_count: usize,
    fc: f64,
    q: f64,
    stimulus: StimulusType,
    allow_self_osc: bool,
    soft_saturation: bool,
) -> DocumentState {
    let nl = if soft_saturation {
        NonLinearityKind::Tanh
    } else {
        NonLinearityKind::HardClip
    };
    create_cascade_svf_doc_ext(
        pole_count,
        fc,
        q,
        stimulus,
        allow_self_osc,
        nl,
        1.0,
        NonLinearPlacement::Output,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_cascade_svf_doc_ext(
    pole_count: usize,
    fc: f64,
    q: f64,
    stimulus: StimulusType,
    allow_self_osc: bool,
    non_linear: NonLinearityKind,
    drive: f64,
    placement: NonLinearPlacement,
) -> DocumentState {
    let f_val = 2.0 * (std::f64::consts::PI * fc.clamp(0.01, 0.45)).sin();
    let q_damp = if allow_self_osc && q >= 25.0 {
        -(q - 25.0) * 0.015
    } else {
        1.0 / q.clamp(0.2, 25.0)
    };

    let mut equations = Vec::new();
    let mut initial_conditions = HashMap::new();
    let mut parameters = Vec::new();

    equations.push(EquationDoc {
        id: 1,
        source: stimulus.equation_source().to_string(),
        color_idx: 0,
    });
    initial_conditions.insert("x".to_string(), 0.0);

    if pole_count <= 2 {
        let (lp_src, hp_src, bp_src) = match placement {
            NonLinearPlacement::FeedbackLoop if non_linear != NonLinearityKind::Clean => {
                let sat_hp = non_linear.wrap_expr("hp[n+1]", drive);
                let sat_bp = non_linear.wrap_expr("bp[n]", drive);
                (
                    format!("lp[n+1] = lp[n] + f * {sat_bp}"),
                    format!("hp[n+1] = x[n+1] - lp[n+1] - q * {sat_bp}"),
                    format!("bp[n+1] = bp[n] + f * {sat_hp}"),
                )
            }
            _ => (
                format!(
                    "lp[n+1] = {}",
                    non_linear.wrap_expr("lp[n] + f * bp[n]", drive)
                ),
                format!(
                    "hp[n+1] = {}",
                    non_linear.wrap_expr("x[n+1] - lp[n+1] - q * bp[n]", drive)
                ),
                format!(
                    "bp[n+1] = {}",
                    non_linear.wrap_expr("bp[n] + f * hp[n+1]", drive)
                ),
            ),
        };

        equations.push(EquationDoc {
            id: 2,
            source: lp_src,
            color_idx: 1,
        });
        equations.push(EquationDoc {
            id: 3,
            source: hp_src,
            color_idx: 2,
        });
        equations.push(EquationDoc {
            id: 4,
            source: bp_src,
            color_idx: 3,
        });

        initial_conditions.insert("lp".to_string(), 0.0);
        initial_conditions.insert("hp".to_string(), 0.0);
        initial_conditions.insert("bp".to_string(), 0.0);

        parameters.push(ParameterDoc {
            name: "f".to_string(),
            value: (f_val * 1000.0).round() / 1000.0,
            min: 0.02,
            max: 0.8,
            step: 0.005,
        });
        parameters.push(ParameterDoc {
            name: "q".to_string(),
            value: (q_damp * 1000.0).round() / 1000.0,
            min: if allow_self_osc { -0.5 } else { 0.05 },
            max: 2.0,
            step: 0.005,
        });
    } else {
        let (lp1_src, hp1_src, bp1_src) = match placement {
            NonLinearPlacement::FeedbackLoop if non_linear != NonLinearityKind::Clean => {
                let sat_hp = non_linear.wrap_expr("hp1[n+1]", drive);
                let sat_bp = non_linear.wrap_expr("bp1[n]", drive);
                (
                    format!("lp1[n+1] = lp1[n] + f1 * {sat_bp}"),
                    format!("hp1[n+1] = x[n+1] - lp1[n+1] - q1 * {sat_bp}"),
                    format!("bp1[n+1] = bp1[n] + f1 * {sat_hp}"),
                )
            }
            _ => (
                format!(
                    "lp1[n+1] = {}",
                    non_linear.wrap_expr("lp1[n] + f1 * bp1[n]", drive)
                ),
                format!(
                    "hp1[n+1] = {}",
                    non_linear.wrap_expr("x[n+1] - lp1[n+1] - q1 * bp1[n]", drive)
                ),
                format!(
                    "bp1[n+1] = {}",
                    non_linear.wrap_expr("bp1[n] + f1 * hp1[n+1]", drive)
                ),
            ),
        };

        let (lp2_src, hp2_src, bp2_src) = match placement {
            NonLinearPlacement::FeedbackLoop if non_linear != NonLinearityKind::Clean => {
                let sat_hp = non_linear.wrap_expr("hp[n+1]", drive);
                let sat_bp = non_linear.wrap_expr("bp[n]", drive);
                (
                    format!("lp[n+1] = lp[n] + f2 * {sat_bp}"),
                    format!("hp[n+1] = lp1[n+1] - lp[n+1] - q2 * {sat_bp}"),
                    format!("bp[n+1] = bp[n] + f2 * {sat_hp}"),
                )
            }
            _ => (
                format!(
                    "lp[n+1] = {}",
                    non_linear.wrap_expr("lp[n] + f2 * bp[n]", drive)
                ),
                format!(
                    "hp[n+1] = {}",
                    non_linear.wrap_expr("lp1[n+1] - lp[n+1] - q2 * bp[n]", drive)
                ),
                format!(
                    "bp[n+1] = {}",
                    non_linear.wrap_expr("bp[n] + f2 * hp[n+1]", drive)
                ),
            ),
        };

        equations.push(EquationDoc {
            id: 2,
            source: lp1_src,
            color_idx: 1,
        });
        equations.push(EquationDoc {
            id: 3,
            source: hp1_src,
            color_idx: 2,
        });
        equations.push(EquationDoc {
            id: 4,
            source: bp1_src,
            color_idx: 3,
        });

        equations.push(EquationDoc {
            id: 5,
            source: lp2_src,
            color_idx: 4,
        });
        equations.push(EquationDoc {
            id: 6,
            source: hp2_src,
            color_idx: 5,
        });
        equations.push(EquationDoc {
            id: 7,
            source: bp2_src,
            color_idx: 0,
        });

        initial_conditions.insert("lp1".to_string(), 0.0);
        initial_conditions.insert("hp1".to_string(), 0.0);
        initial_conditions.insert("bp1".to_string(), 0.0);
        initial_conditions.insert("lp".to_string(), 0.0);
        initial_conditions.insert("hp".to_string(), 0.0);
        initial_conditions.insert("bp".to_string(), 0.0);

        parameters.push(ParameterDoc {
            name: "f1".to_string(),
            value: (f_val * 1000.0).round() / 1000.0,
            min: 0.02,
            max: 0.8,
            step: 0.005,
        });
        parameters.push(ParameterDoc {
            name: "q1".to_string(),
            value: 0.707,
            min: 0.05,
            max: 2.0,
            step: 0.01,
        });
        parameters.push(ParameterDoc {
            name: "f2".to_string(),
            value: (f_val * 1000.0).round() / 1000.0,
            min: 0.02,
            max: 0.8,
            step: 0.005,
        });
        parameters.push(ParameterDoc {
            name: "q2".to_string(),
            value: (q_damp * 1000.0).round() / 1000.0,
            min: if allow_self_osc { -0.5 } else { 0.05 },
            max: 2.0,
            step: 0.005,
        });
    }

    if non_linear != NonLinearityKind::Clean {
        parameters.push(ParameterDoc {
            name: "drive".to_string(),
            value: drive,
            min: 0.2,
            max: 5.0,
            step: 0.05,
        });
    }

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 600,
        selection: Some((15, 150)),
        search_query: "lp[n+1] > lp[n]".to_string(),
        sweep_config: SweepConfig {
            parameter_name: if pole_count <= 2 {
                "f".to_string()
            } else {
                "f2".to_string()
            },
            measurement: MeasurementSpec::new("lp", MeasurementType::Rms),
            min_val: 0.05,
            max_val: 0.6,
            steps: 120,
        },
        clamping_mode: crate::document::ClampingMode::Unit,
    }
}

pub fn create_chamberlin_svf_doc(fc: f64, q: f64, stimulus: StimulusType) -> DocumentState {
    create_cascade_svf_doc(2, fc, q, stimulus, false, false)
}

pub fn create_fir_filter_doc(
    filter_type: FilterType,
    num_taps: usize,
    fc: f64,
    window: FirWindow,
    stimulus: StimulusType,
    non_linear: NonLinearityKind,
    drive: f64,
) -> DocumentState {
    let coeffs = compute_fir_coeffs(filter_type, num_taps, fc, window);

    let mut equations = Vec::new();
    let mut initial_conditions = HashMap::new();
    let mut parameters = Vec::new();

    // Stimulus x
    equations.push(EquationDoc {
        id: 1,
        source: stimulus.equation_source().to_string(),
        color_idx: 0,
    });
    initial_conditions.insert("x".to_string(), 0.0);

    let mut terms = Vec::new();
    for (k, _) in coeffs.iter().enumerate() {
        let h_name = format!("h{}", k);
        let x_ref = match k {
            0 => "x[n+1]".to_string(),
            1 => "x[n]".to_string(),
            _ => format!("x[n-{}]", k - 1),
        };
        terms.push(format!("{} * {}", h_name, x_ref));
    }

    let raw_sum = terms.join(" + ");
    let eq_rhs = non_linear.wrap_expr(&raw_sum, drive);
    let eq_source = format!("y[n+1] = {eq_rhs}");

    equations.push(EquationDoc {
        id: 2,
        source: eq_source,
        color_idx: 1,
    });
    initial_conditions.insert("y".to_string(), 0.0);

    for (k, &c) in coeffs.iter().enumerate() {
        parameters.push(ParameterDoc {
            name: format!("h{}", k),
            value: (c * 10000.0).round() / 10000.0,
            min: -2.0,
            max: 2.0,
            step: 0.001,
        });
    }

    if non_linear != NonLinearityKind::Clean {
        parameters.push(ParameterDoc {
            name: "drive".to_string(),
            value: drive,
            min: 0.2,
            max: 5.0,
            step: 0.05,
        });
    }

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 500,
        selection: Some((10, 150)),
        search_query: "y[n+1] > 0.0".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "h0".to_string(),
            measurement: MeasurementSpec::new("y", MeasurementType::Rms),
            min_val: -1.0,
            max_val: 1.0,
            steps: 80,
        },
        clamping_mode: crate::document::ClampingMode::Unit,
    }
}

pub fn create_moog_ladder_doc(
    fc: f64,
    resonance: f64,
    drive: f64,
    stimulus: StimulusType,
) -> DocumentState {
    let f_val = (2.0 * std::f64::consts::PI * fc.clamp(0.01, 0.45)).clamp(0.02, 0.85);

    let equations = vec![
        EquationDoc {
            id: 1,
            source: stimulus.equation_source().to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "y1[n+1] = y1[n] + f * (tanh(u[n]) - tanh(y1[n]))".to_string(),
            color_idx: 2,
        },
        EquationDoc {
            id: 3,
            source: "y2[n+1] = y2[n] + f * (tanh(y1[n]) - tanh(y2[n]))".to_string(),
            color_idx: 3,
        },
        EquationDoc {
            id: 4,
            source: "y3[n+1] = y3[n] + f * (tanh(y2[n]) - tanh(y3[n]))".to_string(),
            color_idx: 4,
        },
        EquationDoc {
            id: 5,
            source: "y4[n+1] = y4[n] + f * (tanh(y3[n]) - tanh(y4[n]))".to_string(),
            color_idx: 5,
        },
        EquationDoc {
            id: 6,
            source: "u[n+1] = tanh(drive * (x[n+1] - 4.0 * r * y4[n+1]))".to_string(),
            color_idx: 1,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.0);
    initial_conditions.insert("u".to_string(), 0.0);
    initial_conditions.insert("y1".to_string(), 0.0);
    initial_conditions.insert("y2".to_string(), 0.0);
    initial_conditions.insert("y3".to_string(), 0.0);
    initial_conditions.insert("y4".to_string(), 0.0);

    let parameters = vec![
        ParameterDoc {
            name: "f".to_string(),
            value: (f_val * 1000.0).round() / 1000.0,
            min: 0.02,
            max: 0.85,
            step: 0.005,
        },
        ParameterDoc {
            name: "r".to_string(),
            value: resonance.clamp(0.0, 1.4),
            min: 0.0,
            max: 1.4,
            step: 0.01,
        },
        ParameterDoc {
            name: "drive".to_string(),
            value: drive.clamp(0.2, 4.0),
            min: 0.2,
            max: 4.0,
            step: 0.05,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 600,
        selection: Some((20, 160)),
        search_query: "y4[n+1] > 0.0 and y4[n] <= 0.0".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "r".to_string(),
            measurement: MeasurementSpec::new("y4", MeasurementType::Rms),
            min_val: 0.0,
            max_val: 1.25,
            steps: 120,
        },
        clamping_mode: crate::document::ClampingMode::Unit,
    }
}

pub fn create_tb303_ladder_doc(
    fc: f64,
    resonance: f64,
    drive: f64,
    stimulus: StimulusType,
) -> DocumentState {
    let f_val = (2.0 * std::f64::consts::PI * fc.clamp(0.01, 0.45)).clamp(0.02, 0.85);

    let equations = vec![
        EquationDoc {
            id: 1,
            source: stimulus.equation_source().to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "hpf[n+1] = y4[n] - hp_state[n]".to_string(),
            color_idx: 1,
        },
        EquationDoc {
            id: 3,
            source: "hp_state[n+1] = hp_state[n] + 0.06 * hpf[n+1]".to_string(),
            color_idx: 2,
        },
        EquationDoc {
            id: 4,
            source: "u[n+1] = tanh(drive * (x[n+1] - r * hpf[n+1]))".to_string(),
            color_idx: 3,
        },
        EquationDoc {
            id: 5,
            source: "y1[n+1] = y1[n] + f * (tanh(u[n+1]) - y1[n])".to_string(),
            color_idx: 4,
        },
        EquationDoc {
            id: 6,
            source: "y2[n+1] = y2[n] + (f * 0.95) * (y1[n+1] - y2[n])".to_string(),
            color_idx: 5,
        },
        EquationDoc {
            id: 7,
            source: "y3[n+1] = y3[n] + (f * 0.85) * (y2[n+1] - y3[n])".to_string(),
            color_idx: 6,
        },
        EquationDoc {
            id: 8,
            source: "y4[n+1] = y4[n] + (f * 0.75) * (y3[n+1] - y4[n])".to_string(),
            color_idx: 7,
        },
        EquationDoc {
            id: 9,
            source: "acid[n+1] = tanh(1.6 * y4[n+1])".to_string(),
            color_idx: 8,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.0);
    initial_conditions.insert("hpf".to_string(), 0.0);
    initial_conditions.insert("hp_state".to_string(), 0.0);
    initial_conditions.insert("u".to_string(), 0.0);
    initial_conditions.insert("y1".to_string(), 0.0);
    initial_conditions.insert("y2".to_string(), 0.0);
    initial_conditions.insert("y3".to_string(), 0.0);
    initial_conditions.insert("y4".to_string(), 0.0);
    initial_conditions.insert("acid".to_string(), 0.0);

    let parameters = vec![
        ParameterDoc {
            name: "f".to_string(),
            value: (f_val * 1000.0).round() / 1000.0,
            min: 0.02,
            max: 0.85,
            step: 0.005,
        },
        ParameterDoc {
            name: "r".to_string(),
            value: resonance.clamp(0.0, 5.0),
            min: 0.0,
            max: 5.0,
            step: 0.02,
        },
        ParameterDoc {
            name: "drive".to_string(),
            value: drive.clamp(0.2, 4.0),
            min: 0.2,
            max: 4.0,
            step: 0.05,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 600,
        selection: Some((20, 160)),
        search_query: "acid[n+1] > 0.0".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "r".to_string(),
            measurement: MeasurementSpec::new("acid", MeasurementType::Range),
            min_val: 0.5,
            max_val: 4.8,
            steps: 120,
        },
        clamping_mode: crate::document::ClampingMode::Unit,
    }
}
