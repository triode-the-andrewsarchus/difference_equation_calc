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
        match name {
            "n" => Some(self.current_step as f64),
            "pi" | "PI" => Some(std::f64::consts::PI),
            "e" | "E" => Some(std::f64::consts::E),
            _ => self.parameters.get(name).copied(),
        }
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
        match name {
            "n" => Some(self.current_step as f64),
            "pi" | "PI" => Some(std::f64::consts::PI),
            "e" | "E" => Some(std::f64::consts::E),
            _ => self.parameters.get(name).copied(),
        }
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

#[derive(Debug, Clone)]
pub struct VectorFieldPoint {
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub norm_dx: f64,
    pub norm_dy: f64,
    pub magnitude: f64,
}

#[derive(Debug, Clone)]
pub struct NullclineSegment {
    pub p0: (f64, f64),
    pub p1: (f64, f64),
}

#[derive(Debug, Clone)]
pub struct PhaseFieldResult {
    pub vectors: Vec<VectorFieldPoint>,
    pub x_nullclines: Vec<NullclineSegment>,
    pub y_nullclines: Vec<NullclineSegment>,
}

/// Evaluates direction field arrows (dx, dy) and zero-crossing nullclines across a 2D phase grid.
#[allow(clippy::too_many_arguments)]
pub fn compute_phase_field(
    equations: &[Equation],
    var_x: &str,
    var_y: &str,
    other_vars: &HashMap<String, f64>,
    parameters: &HashMap<String, f64>,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    grid_cols: usize,
    grid_rows: usize,
) -> PhaseFieldResult {
    let cols = grid_cols.clamp(5, 30);
    let rows = grid_rows.clamp(5, 30);

    let mut grid_dx = vec![vec![0.0; cols]; rows];
    let mut grid_dy = vec![vec![0.0; cols]; rows];
    let mut vectors = Vec::with_capacity(cols * rows);

    let dx_span = (x_max - x_min).max(1e-6);
    let dy_span = (y_max - y_min).max(1e-6);

    for r in 0..rows {
        let ty = if rows > 1 {
            r as f64 / (rows - 1) as f64
        } else {
            0.5
        };
        let y_val = y_min + ty * dy_span;

        for c in 0..cols {
            let tx = if cols > 1 {
                c as f64 / (cols - 1) as f64
            } else {
                0.5
            };
            let x_val = x_min + tx * dx_span;

            let mut test_inits = other_vars.clone();
            test_inits.insert(var_x.to_string(), x_val);
            test_inits.insert(var_y.to_string(), y_val);

            let (next_x, next_y) = match run_simulation(equations, &test_inits, parameters, 2) {
                Ok(res) => {
                    let nx = res.get_value(var_x, 1).unwrap_or(x_val);
                    let ny = res.get_value(var_y, 1).unwrap_or(y_val);
                    (nx, ny)
                }
                Err(_) => (x_val, y_val),
            };

            let dx = next_x - x_val;
            let dy = next_y - y_val;
            grid_dx[r][c] = dx;
            grid_dy[r][c] = dy;

            let mag = (dx * dx + dy * dy).sqrt();
            let (norm_dx, norm_dy) = if mag > 1e-12 {
                (dx / mag, dy / mag)
            } else {
                (0.0, 0.0)
            };

            vectors.push(VectorFieldPoint {
                x: x_val,
                y: y_val,
                dx,
                dy,
                norm_dx,
                norm_dy,
                magnitude: mag,
            });
        }
    }

    // Extract nullclines via zero-crossing contour segments
    let mut x_nullclines = Vec::new();
    let mut y_nullclines = Vec::new();

    // Horizontal edges
    for r in 0..rows {
        let ty = if rows > 1 {
            r as f64 / (rows - 1) as f64
        } else {
            0.5
        };
        let y_val = y_min + ty * dy_span;

        for c in 0..cols.saturating_sub(1) {
            let tx0 = c as f64 / (cols - 1) as f64;
            let tx1 = (c + 1) as f64 / (cols - 1) as f64;
            let x0 = x_min + tx0 * dx_span;
            let x1 = x_min + tx1 * dx_span;

            let dx0 = grid_dx[r][c];
            let dx1 = grid_dx[r][c + 1];
            if (dx0 <= 0.0 && dx1 >= 0.0) || (dx0 >= 0.0 && dx1 <= 0.0) {
                let denom = (dx1 - dx0).abs();
                let frac = if denom > 1e-12 {
                    (-dx0) / (dx1 - dx0)
                } else {
                    0.5
                };
                let x_cross = (x0 + frac.clamp(0.0, 1.0) * (x1 - x0)).clamp(x_min, x_max);
                let tick = dy_span * 0.4 / (rows as f64);
                x_nullclines.push(NullclineSegment {
                    p0: (x_cross, (y_val - tick).max(y_min)),
                    p1: (x_cross, (y_val + tick).min(y_max)),
                });
            }

            let dy0 = grid_dy[r][c];
            let dy1 = grid_dy[r][c + 1];
            if (dy0 <= 0.0 && dy1 >= 0.0) || (dy0 >= 0.0 && dy1 <= 0.0) {
                let denom = (dy1 - dy0).abs();
                let frac = if denom > 1e-12 {
                    (-dy0) / (dy1 - dy0)
                } else {
                    0.5
                };
                let x_cross = (x0 + frac.clamp(0.0, 1.0) * (x1 - x0)).clamp(x_min, x_max);
                let tick = dx_span * 0.4 / (cols as f64);
                y_nullclines.push(NullclineSegment {
                    p0: ((x_cross - tick).max(x_min), y_val),
                    p1: ((x_cross + tick).min(x_max), y_val),
                });
            }
        }
    }

    // Vertical edges
    for c in 0..cols {
        let tx = if cols > 1 {
            c as f64 / (cols - 1) as f64
        } else {
            0.5
        };
        let x_val = x_min + tx * dx_span;

        for r in 0..rows.saturating_sub(1) {
            let ty0 = r as f64 / (rows - 1) as f64;
            let ty1 = (r + 1) as f64 / (rows - 1) as f64;
            let y0 = y_min + ty0 * dy_span;
            let y1 = y_min + ty1 * dy_span;

            let dx0 = grid_dx[r][c];
            let dx1 = grid_dx[r + 1][c];
            if (dx0 <= 0.0 && dx1 >= 0.0) || (dx0 >= 0.0 && dx1 <= 0.0) {
                let denom = (dx1 - dx0).abs();
                let frac = if denom > 1e-12 {
                    (-dx0) / (dx1 - dx0)
                } else {
                    0.5
                };
                let y_cross = (y0 + frac.clamp(0.0, 1.0) * (y1 - y0)).clamp(y_min, y_max);
                let tick = dx_span * 0.4 / (cols as f64);
                x_nullclines.push(NullclineSegment {
                    p0: ((x_val - tick).max(x_min), y_cross),
                    p1: ((x_val + tick).min(x_max), y_cross),
                });
            }

            let dy0 = grid_dy[r][c];
            let dy1 = grid_dy[r + 1][c];
            if (dy0 <= 0.0 && dy1 >= 0.0) || (dy0 >= 0.0 && dy1 <= 0.0) {
                let denom = (dy1 - dy0).abs();
                let frac = if denom > 1e-12 {
                    (-dy0) / (dy1 - dy0)
                } else {
                    0.5
                };
                let y_cross = (y0 + frac.clamp(0.0, 1.0) * (y1 - y0)).clamp(y_min, y_max);
                let tick = dy_span * 0.4 / (rows as f64);
                y_nullclines.push(NullclineSegment {
                    p0: (x_val, (y_cross - tick).max(y_min)),
                    p1: (x_val, (y_cross + tick).min(y_max)),
                });
            }
        }
    }

    PhaseFieldResult {
        vectors,
        x_nullclines,
        y_nullclines,
    }
}

/// Solves for `param_name` in `[param_min, param_max]` such that
/// `sim.get_value(target_var, target_step) ≈ target_val`.
#[allow(clippy::too_many_arguments)]
pub fn goal_seek_parameter(
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    base_parameters: &HashMap<String, f64>,
    param_name: &str,
    param_min: f64,
    param_max: f64,
    target_var: &str,
    target_step: usize,
    target_val: f64,
    max_iters: usize,
) -> Option<f64> {
    if equations.is_empty() || param_min >= param_max {
        return None;
    }

    let mut current_params = base_parameters.clone();
    let sim_steps = target_step + 1;

    let mut eval_at = |p: f64| -> Option<f64> {
        current_params.insert(param_name.to_string(), p);
        let sim = run_simulation(equations, initial_conditions, &current_params, sim_steps).ok()?;
        sim.get_value(target_var, target_step)
    };

    let start_p = base_parameters
        .get(param_name)
        .copied()
        .unwrap_or((param_min + param_max) * 0.5)
        .clamp(param_min, param_max);

    let y_start = eval_at(start_p)?;
    if (y_start - target_val).abs() < 1e-4 {
        return Some(start_p);
    }

    // Try Secant method
    let span = param_max - param_min;
    let mut p0 = start_p;
    let mut y0 = y_start;

    let delta_p = if p0 + span * 0.02 <= param_max {
        span * 0.02
    } else {
        -span * 0.02
    };

    let mut p1 = (p0 + delta_p).clamp(param_min, param_max);
    let mut y1 = eval_at(p1)?;

    for _ in 0..max_iters.clamp(3, 20) {
        if (y1 - target_val).abs() < 1e-4 {
            return Some(p1);
        }

        let dy = y1 - y0;
        let dp = p1 - p0;
        if dy.abs() < 1e-12 {
            break;
        }

        let next_p = (p1 - (y1 - target_val) * (dp / dy)).clamp(param_min, param_max);
        if (next_p - p1).abs() < 1e-7 {
            return Some(next_p);
        }

        p0 = p1;
        y0 = y1;
        p1 = next_p;
        if let Some(yn) = eval_at(p1) {
            y1 = yn;
        } else {
            break;
        }
    }

    // Fallback: 8-step binary search over [param_min, param_max]
    let mut low_p = param_min;
    let mut high_p = param_max;
    let mut best_p = p1;
    let mut best_err = (y1 - target_val).abs();

    for _ in 0..8 {
        let mid_p = (low_p + high_p) * 0.5;
        if let Some(y_mid) = eval_at(mid_p) {
            let err = (y_mid - target_val).abs();
            if err < best_err {
                best_err = err;
                best_p = mid_p;
            }
            if y_mid < target_val {
                low_p = mid_p;
            } else {
                high_p = mid_p;
            }
        } else {
            break;
        }
    }

    Some(best_p)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClampingInterval {
    pub start_norm: f32, // 0.0 ..= 1.0 (relative to param.min ..= param.max)
    pub end_norm: f32,   // 0.0 ..= 1.0
    pub max_peak: f64,
}

/// Evaluates coarse simulation runs across `num_samples` parameter values
/// to identify sub-ranges in [min_val, max_val] where any variable exceeds `threshold`.
#[allow(clippy::too_many_arguments)]
pub fn analyze_parameter_clamping_zones(
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    base_parameters: &HashMap<String, f64>,
    param_name: &str,
    min_val: f64,
    max_val: f64,
    threshold: f64,
    num_samples: usize,
    steps_to_check: usize,
) -> Vec<ClampingInterval> {
    if equations.is_empty() || min_val >= max_val || num_samples < 2 {
        return Vec::new();
    }

    let mut intervals = Vec::new();
    let mut params = base_parameters.clone();
    let samples = num_samples.clamp(8, 32);
    let steps = steps_to_check.clamp(20, 150);

    let mut in_clamp_zone = false;
    let mut zone_start = 0.0_f32;
    let mut zone_peak = 0.0_f64;

    for i in 0..samples {
        let t = i as f64 / (samples - 1) as f64;
        let p_val = min_val + t * (max_val - min_val);
        params.insert(param_name.to_string(), p_val);

        let mut peak = 0.0_f64;
        let mut is_clamped = false;

        if let Ok(sim) = run_simulation(equations, initial_conditions, &params, steps) {
            for series in sim.series.values() {
                for &v in &series.values {
                    if v.is_finite() {
                        let abs_v = v.abs();
                        if abs_v > peak {
                            peak = abs_v;
                        }
                        if abs_v > threshold {
                            is_clamped = true;
                        }
                    } else {
                        is_clamped = true;
                        peak = f64::INFINITY;
                    }
                }
            }
        }

        let t_f32 = t as f32;
        if is_clamped {
            if !in_clamp_zone {
                in_clamp_zone = true;
                zone_start = t_f32;
                zone_peak = peak;
            } else if peak > zone_peak {
                zone_peak = peak;
            }
        } else if in_clamp_zone {
            intervals.push(ClampingInterval {
                start_norm: zone_start,
                end_norm: t_f32,
                max_peak: zone_peak,
            });
            in_clamp_zone = false;
            zone_peak = 0.0;
        }
    }

    if in_clamp_zone {
        intervals.push(ClampingInterval {
            start_norm: zone_start,
            end_norm: 1.0,
            max_peak: zone_peak,
        });
    }

    intervals
}
