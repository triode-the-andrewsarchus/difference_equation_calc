use crate::ast::Equation;
use crate::deps::{analyze_dependencies, DependencyError};
use crate::eval::{eval_expr, EvalContext, EvalError};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Series {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct SimulationFailure {
    pub step: usize,
    pub variable: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub series: HashMap<String, Series>,
    pub sample_count: usize,
    pub failure: Option<SimulationFailure>,
}

impl SimulationResult {
    pub fn get_value(&self, var: &str, step: usize) -> Option<f64> {
        self.series
            .get(var)
            .and_then(|s| s.values.get(step))
            .copied()
    }
}

pub struct SimulationContext<'a> {
    pub current_step: usize,
    pub completed_history: &'a HashMap<String, Vec<f64>>,
    pub current_step_values: &'a HashMap<String, f64>,
    pub parameters: &'a HashMap<String, f64>,
}

impl<'a> EvalContext for SimulationContext<'a> {
    fn get_state(&self, variable: &str, offset: i64) -> Result<f64, EvalError> {
        // Recurrence offset: relative to current timestep n
        // If offset == 1 (or target_offset), check if already computed at current step
        if offset == 1 {
            if let Some(val) = self.current_step_values.get(variable) {
                return Ok(*val);
            }
        }

        let hist = self
            .completed_history
            .get(variable)
            .ok_or_else(|| EvalError::UndefinedVariable(variable.to_string()))?;

        let target_idx = (self.current_step as i64) + offset;
        if target_idx < 0 {
            // Before initial conditions: fallback to first element or 0.0
            return Ok(hist.first().copied().unwrap_or(0.0));
        }

        let target_usize = target_idx as usize;
        if target_usize < hist.len() {
            Ok(hist[target_usize])
        } else {
            Err(EvalError::MissingHistory {
                variable: variable.to_string(),
                requested_index: target_idx,
            })
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        self.parameters.get(name).copied()
    }
}

#[derive(Debug)]
pub struct SimResultContext<'a> {
    pub sim_result: &'a SimulationResult,
    pub current_step: usize,
    pub parameters: &'a HashMap<String, f64>,
}

impl<'a> EvalContext for SimResultContext<'a> {
    fn get_state(&self, variable: &str, offset: i64) -> Result<f64, EvalError> {
        let series = self
            .sim_result
            .series
            .get(variable)
            .ok_or_else(|| EvalError::UndefinedVariable(variable.to_string()))?;
        let target_idx = (self.current_step as i64) + offset;
        if target_idx < 0 || (target_idx as usize) >= series.values.len() {
            return Err(EvalError::MissingHistory {
                variable: variable.to_string(),
                requested_index: target_idx,
            });
        }
        Ok(series.values[target_idx as usize])
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        self.parameters.get(name).copied()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimulationSetupError {
    Dependency(DependencyError),
    NoEquations,
}

impl From<DependencyError> for SimulationSetupError {
    fn from(err: DependencyError) -> Self {
        SimulationSetupError::Dependency(err)
    }
}

pub fn run_simulation(
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    parameters: &HashMap<String, f64>,
    sample_count: usize,
) -> Result<SimulationResult, SimulationSetupError> {
    if equations.is_empty() {
        return Ok(SimulationResult {
            series: HashMap::new(),
            sample_count,
            failure: None,
        });
    }

    let dep_info = analyze_dependencies(equations)?;

    // Map equations by target_var for quick lookup
    let mut eq_map: HashMap<String, &Equation> = HashMap::new();
    for eq in equations {
        eq_map.insert(eq.target_var.clone(), eq);
    }

    // Initialize histories
    let mut history: HashMap<String, Vec<f64>> = HashMap::new();
    for var in &dep_info.eval_order {
        let mut buf = Vec::with_capacity(sample_count);
        let init = initial_conditions.get(var).copied().unwrap_or(0.0);
        buf.push(init);
        history.insert(var.clone(), buf);
    }

    let mut failure = None;

    // Simulation loop: step n from 0 up to sample_count - 1
    // At step n, we compute step n+1 for each variable in eval_order
    for n in 0..sample_count.saturating_sub(1) {
        let mut current_step_values: HashMap<String, f64> = HashMap::new();

        for var in &dep_info.eval_order {
            let eq = eq_map.get(var).unwrap();

            let ctx = SimulationContext {
                current_step: n,
                completed_history: &history,
                current_step_values: &current_step_values,
                parameters,
            };

            match eval_expr(&eq.rhs, &ctx) {
                Ok(val) => {
                    current_step_values.insert(var.clone(), val);
                }
                Err(err) => {
                    failure = Some(SimulationFailure {
                        step: n + 1,
                        variable: var.clone(),
                        message: err.to_string(),
                    });
                    break;
                }
            }
        }

        if failure.is_some() {
            // Fill remainder with NaNs so series have consistent length
            for (var, buf) in history.iter_mut() {
                if let Some(val) = current_step_values.get(var) {
                    buf.push(*val);
                } else {
                    buf.push(f64::NAN);
                }
                while buf.len() < sample_count {
                    buf.push(f64::NAN);
                }
            }
            break;
        }

        for (var, val) in current_step_values {
            history.get_mut(&var).unwrap().push(val);
        }
    }

    let mut series = HashMap::new();
    for (name, values) in history {
        series.insert(name.clone(), Series { name, values });
    }

    Ok(SimulationResult {
        series,
        sample_count,
        failure,
    })
}
