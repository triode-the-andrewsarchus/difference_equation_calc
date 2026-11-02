use crate::sim::SimulationResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementType {
    Mean,
    StdDev,
    Min,
    Max,
    Range,
    Sum,
    Rms,
    First,
    Last,
    Delta,
    Overshoot,
    SettlingSteps,
    ZeroCrossings,
    DomFreq,
    Duration, // Δn
}

impl MeasurementType {
    pub const ALL: &'static [MeasurementType] = &[
        MeasurementType::Mean,
        MeasurementType::StdDev,
        MeasurementType::Min,
        MeasurementType::Max,
        MeasurementType::Range,
        MeasurementType::Sum,
        MeasurementType::Rms,
        MeasurementType::First,
        MeasurementType::Last,
        MeasurementType::Delta,
        MeasurementType::Overshoot,
        MeasurementType::SettlingSteps,
        MeasurementType::ZeroCrossings,
        MeasurementType::DomFreq,
        MeasurementType::Duration,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            MeasurementType::Mean => "Mean",
            MeasurementType::StdDev => "StdDev (σ)",
            MeasurementType::Min => "Min",
            MeasurementType::Max => "Max",
            MeasurementType::Range => "Range",
            MeasurementType::Sum => "Sum",
            MeasurementType::Rms => "RMS",
            MeasurementType::First => "First",
            MeasurementType::Last => "Last",
            MeasurementType::Delta => "Delta (Δy)",
            MeasurementType::Overshoot => "Overshoot",
            MeasurementType::SettlingSteps => "Settling (n)",
            MeasurementType::ZeroCrossings => "Zero Cross",
            MeasurementType::DomFreq => "Freq (f)",
            MeasurementType::Duration => "Duration (Δn)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementSpec {
    pub variable: String,
    pub kind: MeasurementType,
}

impl MeasurementSpec {
    pub fn new(variable: impl Into<String>, kind: MeasurementType) -> Self {
        Self {
            variable: variable.into(),
            kind,
        }
    }

    pub fn display_name(&self) -> String {
        if self.kind == MeasurementType::Duration {
            "Duration (Δn)".to_string()
        } else {
            format!("{}({})", self.kind.name(), self.variable)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MeasureError {
    UndefinedVariable(String),
    EmptySelection,
    OutOfBounds,
    AllValuesNonFinite,
}

impl std::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeasureError::UndefinedVariable(v) => write!(f, "Undefined variable: {v}"),
            MeasureError::EmptySelection => write!(f, "Selection range is empty"),
            MeasureError::OutOfBounds => {
                write!(f, "Selection is out of bounds of simulation history")
            }
            MeasureError::AllValuesNonFinite => {
                write!(f, "All values in selection are NaN or infinite")
            }
        }
    }
}

impl std::error::Error for MeasureError {}

pub fn calculate_measurement(
    spec: &MeasurementSpec,
    sim_result: &SimulationResult,
    selection_start: usize,
    selection_end: usize, // inclusive
) -> Result<f64, MeasureError> {
    if selection_start > selection_end {
        return Err(MeasureError::EmptySelection);
    }

    if spec.kind == MeasurementType::Duration {
        return Ok((selection_end - selection_start + 1) as f64);
    }

    let series = sim_result
        .series
        .get(&spec.variable)
        .ok_or_else(|| MeasureError::UndefinedVariable(spec.variable.clone()))?;

    if selection_start >= series.values.len() {
        return Err(MeasureError::OutOfBounds);
    }

    let actual_end = selection_end.min(series.values.len() - 1);
    let slice = &series.values[selection_start..=actual_end];

    let valid_values: Vec<f64> = slice.iter().copied().filter(|v| v.is_finite()).collect();
    if valid_values.is_empty() {
        return Err(MeasureError::AllValuesNonFinite);
    }

    match spec.kind {
        MeasurementType::Duration => Ok((selection_end - selection_start + 1) as f64),
        MeasurementType::First => Ok(*valid_values.first().unwrap()),
        MeasurementType::Last => Ok(*valid_values.last().unwrap()),
        MeasurementType::Delta => {
            let first = *valid_values.first().unwrap();
            let last = *valid_values.last().unwrap();
            Ok(last - first)
        }
        MeasurementType::Min => {
            let min = valid_values.iter().copied().fold(f64::INFINITY, f64::min);
            Ok(min)
        }
        MeasurementType::Max => {
            let max = valid_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            Ok(max)
        }
        MeasurementType::Range => {
            let min = valid_values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = valid_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            Ok(max - min)
        }
        MeasurementType::Sum => {
            let sum: f64 = valid_values.iter().sum();
            Ok(sum)
        }
        MeasurementType::Mean => {
            let sum: f64 = valid_values.iter().sum();
            Ok(sum / (valid_values.len() as f64))
        }
        MeasurementType::StdDev => {
            let n = valid_values.len() as f64;
            let mean = valid_values.iter().sum::<f64>() / n;
            let var = valid_values
                .iter()
                .map(|v| {
                    let d = *v - mean;
                    d * d
                })
                .sum::<f64>()
                / n;
            Ok(var.sqrt())
        }
        MeasurementType::Rms => {
            let sum_sq: f64 = valid_values.iter().map(|v| v * v).sum();
            Ok((sum_sq / (valid_values.len() as f64)).sqrt())
        }
        MeasurementType::Overshoot => {
            let first = *valid_values.first().unwrap();
            let last = *valid_values.last().unwrap();
            let min = valid_values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = valid_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            if (last - first).abs() < 1e-6 {
                Ok((max - last).abs().max((last - min).abs()))
            } else if last > first {
                Ok((max - last).max(0.0))
            } else {
                Ok((last - min).max(0.0))
            }
        }
        MeasurementType::SettlingSteps => {
            let last = *valid_values.last().unwrap();
            let min = valid_values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = valid_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let tol = ((max - min) * 0.02).max(1e-3);
            let mut last_outside = None;
            for (idx, &v) in valid_values.iter().enumerate() {
                if (v - last).abs() > tol {
                    last_outside = Some(idx);
                }
            }
            Ok(last_outside.map_or(0.0, |idx| (idx + 1) as f64))
        }
        MeasurementType::ZeroCrossings => {
            let mut crossings = 0usize;
            for w in valid_values.windows(2) {
                if (w[0] < 0.0 && w[1] >= 0.0) || (w[0] > 0.0 && w[1] <= 0.0) {
                    crossings += 1;
                }
            }
            Ok(crossings as f64)
        }
        MeasurementType::DomFreq => Ok(estimate_dominant_frequency(&valid_values)),
    }
}

/// Estimates the dominant oscillation frequency in cycles/sample `[0.0, 0.5]`
/// using sub-sample interpolated zero-crossings around the signal mean.
pub fn estimate_dominant_frequency(values: &[f64]) -> f64 {
    if values.len() < 3 {
        return 0.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let max_dev = values
        .iter()
        .map(|v| (*v - mean).abs())
        .fold(0.0_f64, f64::max);
    if max_dev < 1e-7 {
        return 0.0;
    }

    let mut up_crossings = Vec::new();
    let mut down_crossings = Vec::new();

    for i in 0..values.len() - 1 {
        let y0 = values[i] - mean;
        let y1 = values[i + 1] - mean;
        if y0 <= 0.0 && y1 > 0.0 {
            let frac = (-y0) / (y1 - y0).max(1e-12);
            up_crossings.push(i as f64 + frac);
        } else if y0 >= 0.0 && y1 < 0.0 {
            let frac = y0 / (y0 - y1).max(1e-12);
            down_crossings.push(i as f64 + frac);
        }
    }

    if up_crossings.len() >= 2 {
        let total_span = up_crossings.last().unwrap() - up_crossings.first().unwrap();
        let periods = (up_crossings.len() - 1) as f64;
        if total_span > 1e-6 {
            return (periods / total_span).clamp(0.0, 0.5);
        }
    }
    if down_crossings.len() >= 2 {
        let total_span = down_crossings.last().unwrap() - down_crossings.first().unwrap();
        let periods = (down_crossings.len() - 1) as f64;
        if total_span > 1e-6 {
            return (periods / total_span).clamp(0.0, 0.5);
        }
    }
    if let (Some(&u), Some(&d)) = (up_crossings.first(), down_crossings.first()) {
        let half_period = (u - d).abs();
        if half_period > 0.5 {
            return (1.0 / (2.0 * half_period)).clamp(0.0, 0.5);
        }
    }
    0.0
}

#[derive(Debug, Clone)]
pub struct SpectrumBin {
    pub freq: f64, // Normalized frequency in [0.0, 0.5] cycles/sample
    pub magnitude: f64,
    pub mag_db: f64,
}

#[derive(Debug, Clone)]
pub struct SpectrumResult {
    pub bins: Vec<SpectrumBin>,
    pub peak_freq: f64,
    pub peak_mag: f64,
    pub peak_db: f64,
    pub dc_offset: f64,
}

/// Computes the Hann-windowed magnitude spectrum of a signal slice using a radix-2 FFT.
pub fn compute_spectrum(values: &[f64]) -> SpectrumResult {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.len() < 4 {
        return SpectrumResult {
            bins: Vec::new(),
            peak_freq: 0.0,
            peak_mag: 0.0,
            peak_db: -100.0,
            dc_offset: 0.0,
        };
    }

    let n_samples = finite.len();
    let dc_offset = finite.iter().sum::<f64>() / (n_samples as f64);

    // Choose FFT size as next power of two, at least 256 and at most 2048
    let fft_len = n_samples.next_power_of_two().clamp(256, 2048);
    let use_len = n_samples.min(fft_len);

    let mut re = vec![0.0_f64; fft_len];
    let mut im = vec![0.0_f64; fft_len];

    let mut window_sum = 0.0_f64;
    for (i, &val) in finite.iter().take(use_len).enumerate() {
        let w = if use_len > 1 {
            0.5 - 0.5 * (2.0 * std::f64::consts::PI * (i as f64) / ((use_len - 1) as f64)).cos()
        } else {
            1.0
        };
        re[i] = (val - dc_offset) * w;
        window_sum += w;
    }

    fft_in_place(&mut re, &mut im);

    let scale = if window_sum > 1e-9 {
        2.0 / window_sum
    } else {
        1.0
    };
    let half = fft_len / 2;
    let mut bins = Vec::with_capacity(half + 1);
    let mut peak_bin = 0usize;
    let mut peak_mag = 0.0_f64;

    for k in 0..=half {
        let freq = (k as f64) / (fft_len as f64);
        let mag = re[k].hypot(im[k]) * scale;
        let mag_db = if mag > 1e-5 {
            (20.0 * mag.log10()).clamp(-100.0, 40.0)
        } else {
            -100.0
        };
        if k > 0 && mag > peak_mag {
            peak_mag = mag;
            peak_bin = k;
        }
        bins.push(SpectrumBin {
            freq,
            magnitude: mag,
            mag_db,
        });
    }

    // Parabolic interpolation around peak_bin for sub-bin frequency & amplitude accuracy
    let (peak_freq, peak_mag) = if peak_bin > 0 && peak_bin < half {
        let m_prev = bins[peak_bin - 1].magnitude;
        let m_curr = bins[peak_bin].magnitude;
        let m_next = bins[peak_bin + 1].magnitude;
        let denom = m_prev - 2.0 * m_curr + m_next;
        let delta = if denom.abs() > 1e-12 {
            (0.5 * (m_prev - m_next) / denom).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let interp_freq = ((peak_bin as f64 + delta) / (fft_len as f64)).clamp(0.0, 0.5);
        let interp_mag = (m_curr - 0.25 * (m_prev - m_next) * delta).max(peak_mag);
        (interp_freq, interp_mag)
    } else {
        ((peak_bin as f64) / (fft_len as f64), peak_mag)
    };

    let peak_db = if peak_mag > 1e-5 {
        (20.0 * peak_mag.log10()).clamp(-100.0, 40.0)
    } else {
        -100.0
    };

    SpectrumResult {
        bins,
        peak_freq,
        peak_mag,
        peak_db,
        dc_offset,
    }
}

fn fft_in_place(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two());

    // Bit-reversal permutation
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    // Cooley-Tukey butterflies
    let mut len = 2usize;
    while len <= n {
        let half = len / 2;
        let angle = -2.0 * std::f64::consts::PI / (len as f64);
        let w_len_re = angle.cos();
        let w_len_im = angle.sin();

        for start in (0..n).step_by(len) {
            let mut w_re = 1.0_f64;
            let mut w_im = 0.0_f64;
            for k in 0..half {
                let u_re = re[start + k];
                let u_im = im[start + k];
                let v_re = re[start + k + half] * w_re - im[start + k + half] * w_im;
                let v_im = re[start + k + half] * w_im + im[start + k + half] * w_re;

                re[start + k] = u_re + v_re;
                im[start + k] = u_im + v_im;
                re[start + k + half] = u_re - v_re;
                im[start + k + half] = u_im - v_im;

                let next_w_re = w_re * w_len_re - w_im * w_len_im;
                let next_w_im = w_re * w_len_im + w_im * w_len_re;
                w_re = next_w_re;
                w_im = next_w_im;
            }
        }
        len <<= 1;
    }
}
