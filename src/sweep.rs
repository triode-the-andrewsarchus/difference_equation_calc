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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SweepMode {
    Measurement,
    AttractorBifurcation,
}

#[derive(Debug, Clone)]
pub struct AttractorPoint {
    pub param_val: f64,
    pub state_val: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttractorSweepConfig {
    pub parameter_name: String,
    pub variable_name: String,
    pub min_val: f64,
    pub max_val: f64,
    pub slices: usize,
    pub total_steps: usize,
    pub tail_steps: usize,
}

#[derive(Debug, Clone)]
pub struct AttractorSweepResult {
    pub parameter_name: String,
    pub variable_name: String,
    pub points: Vec<AttractorPoint>,
    pub param_min: f64,
    pub param_max: f64,
    pub state_min: f64,
    pub state_max: f64,
}

pub fn execute_attractor_sweep(
    config: &AttractorSweepConfig,
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    base_parameters: &HashMap<String, f64>,
) -> AttractorSweepResult {
    let slices = config.slices.max(10);
    let mut points = Vec::new();
    let mut state_min = f64::INFINITY;
    let mut state_max = f64::NEG_INFINITY;

    let mut sweep_params = base_parameters.clone();
    let step_size = (config.max_val - config.min_val) / ((slices - 1) as f64);
    let tail_start = config.total_steps.saturating_sub(config.tail_steps);

    for i in 0..slices {
        let p_val = config.min_val + (i as f64) * step_size;
        sweep_params.insert(config.parameter_name.clone(), p_val);

        if let Ok(sim_res) = run_simulation(
            equations,
            initial_conditions,
            &sweep_params,
            config.total_steps,
        ) {
            if let Some(series) = sim_res.series.get(&config.variable_name) {
                let vals = &series.values;
                let tail = &vals[tail_start.min(vals.len())..vals.len()];
                for &v in tail {
                    if v.is_finite() && v.abs() < 1e5 {
                        state_min = state_min.min(v);
                        state_max = state_max.max(v);
                        points.push(AttractorPoint {
                            param_val: p_val,
                            state_val: v,
                        });
                    }
                }
            }
        }
    }

    if !state_min.is_finite() {
        state_min = 0.0;
        state_max = 1.0;
    } else if (state_max - state_min).abs() < 1e-9 {
        state_max += 1.0;
        state_min -= 1.0;
    }

    AttractorSweepResult {
        parameter_name: config.parameter_name.clone(),
        variable_name: config.variable_name.clone(),
        points,
        param_min: config.min_val,
        param_max: config.max_val,
        state_min,
        state_max,
    }
}

#[derive(Debug, Clone)]
pub struct ParameterFamilyMember {
    pub param_value: f64,
    pub series: HashMap<String, Vec<f64>>,
}

#[derive(Debug, Clone)]
pub struct ParameterFamilyResult {
    pub param_name: String,
    pub members: Vec<ParameterFamilyMember>,
}

/// Computes an ensemble / family of simulation runs across `count` evenly spaced parameter values.
#[allow(clippy::too_many_arguments)]
pub fn compute_parameter_family(
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    base_parameters: &HashMap<String, f64>,
    param_name: &str,
    min_val: f64,
    max_val: f64,
    count: usize,
    sample_count: usize,
) -> ParameterFamilyResult {
    let count = count.clamp(2, 12);
    let mut members = Vec::with_capacity(count);
    let mut params = base_parameters.clone();

    for i in 0..count {
        let t = if count > 1 {
            i as f64 / (count - 1) as f64
        } else {
            0.5
        };
        let p_val = min_val + t * (max_val - min_val);
        params.insert(param_name.to_string(), p_val);

        if let Ok(sim) = run_simulation(equations, initial_conditions, &params, sample_count) {
            let mut s_map = HashMap::new();
            for (k, v) in sim.series {
                s_map.insert(k, v.values);
            }
            members.push(ParameterFamilyMember {
                param_value: p_val,
                series: s_map,
            });
        }
    }

    ParameterFamilyResult {
        param_name: param_name.to_string(),
        members,
    }
}
