use crate::ast::Equation;
use crate::measure::{calculate_measurement, MeasurementSpec};
use crate::sim::run_simulation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepConfig {
    pub parameter_name: String,
    pub min_val: f64,
    pub max_val: f64,
    pub steps: usize,
    pub measurement: MeasurementSpec,
}

impl Default for SweepConfig {
    fn default() -> Self {
        Self {
            parameter_name: "k".to_string(),
            min_val: 0.0,
            max_val: 0.2,
            steps: 120,
            measurement: MeasurementSpec::new("a", crate::measure::MeasurementType::Mean),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SweepPoint {
    pub param_value: f64,
    pub measurement_value: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SweepResult {
    pub config: SweepConfig,
    pub points: Vec<SweepPoint>,
    pub min_measure: f64,
    pub max_measure: f64,
}

pub fn execute_parameter_sweep(
    config: &SweepConfig,
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    base_parameters: &HashMap<String, f64>,
    sample_count: usize,
    selection_start: usize,
    selection_end: usize,
) -> SweepResult {
    let steps = config.steps.max(2);
    let mut points = Vec::with_capacity(steps);
    let mut min_measure = f64::INFINITY;
    let mut max_measure = f64::NEG_INFINITY;

    // Clone base parameters so original document is untouched
    let mut sweep_params = base_parameters.clone();

    let step_size = (config.max_val - config.min_val) / ((steps - 1) as f64);

    for i in 0..steps {
        let p_val = config.min_val + (i as f64) * step_size;
        sweep_params.insert(config.parameter_name.clone(), p_val);

        let measure_val =
            match run_simulation(equations, initial_conditions, &sweep_params, sample_count) {
                Ok(sim_result) => {
                    match calculate_measurement(
                        &config.measurement,
                        &sim_result,
                        selection_start,
                        selection_end,
                    ) {
                        Ok(m) if m.is_finite() => {
                            min_measure = min_measure.min(m);
                            max_measure = max_measure.max(m);
                            Some(m)
                        }
                        _ => None,
                    }
                }
                Err(_) => None,
            };

        points.push(SweepPoint {
            param_value: p_val,
            measurement_value: measure_val,
        });
    }

    if !min_measure.is_finite() {
        min_measure = 0.0;
        max_measure = 1.0;
    } else if (max_measure - min_measure).abs() < 1e-9 {
        max_measure += 1.0;
        min_measure -= 1.0;
    }

    SweepResult {
        config: config.clone(),
        points,
        min_measure,
        max_measure,
    }
}
