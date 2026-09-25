pub mod c;

use crate::ast::{Equation, Expr, ExprKind};
use crate::deps::{analyze_dependencies, DependencyError};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct VariableModel {
    pub name: String,
    pub max_delay: usize,
    pub initial_value: f64,
}

#[derive(Debug, Clone)]
pub struct ParameterModel {
    pub name: String,
    pub default_value: f64,
}

#[derive(Debug, Clone)]
pub struct SystemModel {
    pub eval_order: Vec<String>,
    pub equations: HashMap<String, Equation>,
    pub variables: HashMap<String, VariableModel>,
    pub parameters: Vec<ParameterModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodegenError {
    Dependency(DependencyError),
    NoEquations,
    UnsupportedExpr(String),
}

impl From<DependencyError> for CodegenError {
    fn from(err: DependencyError) -> Self {
        CodegenError::Dependency(err)
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Dependency(d) => write!(f, "Dependency error: {d}"),
            CodegenError::NoEquations => write!(f, "Cannot generate code without equations"),
            CodegenError::UnsupportedExpr(msg) => write!(f, "Unsupported expression: {msg}"),
        }
    }
}

impl std::error::Error for CodegenError {}

impl SystemModel {
    pub fn from_equations(
        equations: &[Equation],
        initial_conditions: &HashMap<String, f64>,
        parameters: &HashMap<String, f64>,
    ) -> Result<Self, CodegenError> {
        if equations.is_empty() {
            return Err(CodegenError::NoEquations);
        }

        let dep_info = analyze_dependencies(equations)?;

        let mut eq_map = HashMap::new();
        for eq in equations {
            eq_map.insert(eq.target_var.clone(), eq.clone());
        }

        let mut delays: HashMap<String, usize> = HashMap::new();
        for var in &dep_info.eval_order {
            delays.insert(var.clone(), 0);
        }

        for eq in equations {
            scan_expr_delays(&eq.rhs, &mut delays);
        }

        let mut variables = HashMap::new();
        for var in &dep_info.eval_order {
            let max_delay = delays.get(var).copied().unwrap_or(0);
            let init = initial_conditions.get(var).copied().unwrap_or(0.0);
            variables.insert(
                var.clone(),
                VariableModel {
                    name: var.clone(),
                    max_delay,
                    initial_value: init,
                },
            );
        }

        let mut param_models = Vec::new();
        for p in &dep_info.referenced_parameters {
            if p == "n" || p == "pi" || p == "PI" || p == "e" || p == "E" {
                continue;
            }
            if variables.contains_key(p) {
                // Bare identifier used as state variable
                continue;
            }
            let val = parameters.get(p).copied().unwrap_or(0.0);
            param_models.push(ParameterModel {
                name: p.clone(),
                default_value: val,
            });
        }
        param_models.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self {
            eval_order: dep_info.eval_order,
            equations: eq_map,
            variables,
            parameters: param_models,
        })
    }
}

fn scan_expr_delays(expr: &Expr, delays: &mut HashMap<String, usize>) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Parameter(_) => {}
        ExprKind::StateRef { variable, offset } => {
            if *offset < 0 {
                let delay = (-*offset) as usize;
                let entry = delays.entry(variable.clone()).or_insert(0);
                if delay > *entry {
                    *entry = delay;
                }
            }
        }
        ExprKind::Unary { expr, .. } => scan_expr_delays(expr, delays),
        ExprKind::Binary { left, right, .. } => {
            scan_expr_delays(left, delays);
            scan_expr_delays(right, delays);
        }
        ExprKind::Cond {
            condition,
            true_val,
            false_val,
        } => {
            scan_expr_delays(condition, delays);
            scan_expr_delays(true_val, delays);
            scan_expr_delays(false_val, delays);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                scan_expr_delays(arg, delays);
            }
        }
    }
}
