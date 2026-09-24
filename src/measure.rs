use crate::sim::SimulationResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementType {
    Mean,
    Min,
    Max,
    Range,
    Sum,
    Rms,
    First,
    Last,
    Delta,
    Duration, // Δn
}

impl MeasurementType {
    pub const ALL: &'static [MeasurementType] = &[
        MeasurementType::Mean,
        MeasurementType::Min,
        MeasurementType::Max,
        MeasurementType::Range,
        MeasurementType::Sum,
        MeasurementType::Rms,
        MeasurementType::First,
        MeasurementType::Last,
        MeasurementType::Delta,
        MeasurementType::Duration,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            MeasurementType::Mean => "Mean",
            MeasurementType::Min => "Min",
            MeasurementType::Max => "Max",
            MeasurementType::Range => "Range",
            MeasurementType::Sum => "Sum",
            MeasurementType::Rms => "RMS",
            MeasurementType::First => "First",
            MeasurementType::Last => "Last",
            MeasurementType::Delta => "Delta (Δy)",
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
        MeasurementType::Rms => {
            let sum_sq: f64 = valid_values.iter().map(|v| v * v).sum();
            Ok((sum_sq / (valid_values.len() as f64)).sqrt())
        }
    }
}
