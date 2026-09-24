use crate::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    UndefinedVariable(String),
    UndefinedParameter(String),
    MissingHistory {
        variable: String,
        requested_index: i64,
    },
    DivisionByZero,
    DomainError(String),
    NonFiniteResult(String),
    UnknownFunction(String),
    WrongArgCount {
        func: String,
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::UndefinedVariable(v) => write!(f, "Undefined variable: '{v}'"),
            EvalError::UndefinedParameter(p) => write!(f, "Undefined parameter: '{p}'"),
            EvalError::MissingHistory {
                variable,
                requested_index,
            } => write!(
                f,
                "Variable '{variable}' has no value at timestep {requested_index}"
            ),
            EvalError::DivisionByZero => write!(f, "Division by zero"),
            EvalError::DomainError(msg) => write!(f, "Math domain error: {msg}"),
            EvalError::NonFiniteResult(msg) => {
                write!(f, "Calculation produced non-finite result: {msg}")
            }
            EvalError::UnknownFunction(fn_name) => write!(f, "Unknown function: '{fn_name}'"),
            EvalError::WrongArgCount {
                func,
                expected,
                actual,
            } => write!(
                f,
                "Function '{func}' expects {expected} arguments, but got {actual}"
            ),
        }
    }
}

impl std::error::Error for EvalError {}

pub trait EvalContext {
    fn get_state(&self, variable: &str, offset: i64) -> Result<f64, EvalError>;
    fn get_parameter(&self, name: &str) -> Option<f64>;
}

/// Simple context for evaluating search queries or standalone expressions over a history slice
pub struct SlicedContext<'a> {
    pub history: &'a HashMap<String, Vec<f64>>,
    pub current_step: usize,
    pub parameters: &'a HashMap<String, f64>,
}

impl<'a> EvalContext for SlicedContext<'a> {
    fn get_state(&self, variable: &str, offset: i64) -> Result<f64, EvalError> {
        let series = self
            .history
            .get(variable)
            .ok_or_else(|| EvalError::UndefinedVariable(variable.to_string()))?;
        let target_idx = (self.current_step as i64) + offset;
        if target_idx < 0 || (target_idx as usize) >= series.len() {
            return Err(EvalError::MissingHistory {
                variable: variable.to_string(),
                requested_index: target_idx,
            });
        }
        Ok(series[target_idx as usize])
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

pub fn eval_expr<C: EvalContext>(expr: &Expr, ctx: &C) -> Result<f64, EvalError> {
    let result = match &expr.kind {
        ExprKind::Literal(val) => *val,
        ExprKind::Parameter(name) => {
            if let Some(val) = ctx.get_parameter(name) {
                val
            } else if let Ok(val) = ctx.get_state(name, 0) {
                // If it's a bare identifier matching a variable name, treat as var[n]
                val
            } else {
                return Err(EvalError::UndefinedParameter(name.clone()));
            }
        }
        ExprKind::StateRef { variable, offset } => ctx.get_state(variable, *offset)?,
        ExprKind::Unary { op, expr } => {
            let val = eval_expr(expr, ctx)?;
            match op {
                UnaryOp::Neg => -val,
                UnaryOp::Not => {
                    if val == 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
            }
        }
        ExprKind::Binary { op, left, right } => {
            let l = eval_expr(left, ctx)?;
            let r = eval_expr(right, ctx)?;
            eval_binary_op(*op, l, r)?
        }
        ExprKind::Cond {
            condition,
            true_val,
            false_val,
        } => {
            let cond_res = eval_expr(condition, ctx)?;
            if cond_res != 0.0 {
                eval_expr(true_val, ctx)?
            } else {
                eval_expr(false_val, ctx)?
            }
        }
        ExprKind::Call { func, args } => eval_call(func, args, ctx)?,
    };

    if result.is_nan() {
        return Err(EvalError::NonFiniteResult("NaN encountered".to_string()));
    }
    if result.is_infinite() {
        return Err(EvalError::NonFiniteResult(
            "Infinity encountered (overflow/divergence)".to_string(),
        ));
    }

    Ok(result)
}

fn eval_binary_op(op: BinaryOp, l: f64, r: f64) -> Result<f64, EvalError> {
    let res = match op {
        BinaryOp::Add => l + r,
        BinaryOp::Sub => l - r,
        BinaryOp::Mul => l * r,
        BinaryOp::Div => {
            if r.abs() == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            l / r
        }
        BinaryOp::Rem => {
            if r.abs() == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            l % r
        }
        BinaryOp::Pow => l.powf(r),
        BinaryOp::Eq => {
            if (l - r).abs() < 1e-12 {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::Ne => {
            if (l - r).abs() >= 1e-12 {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::Lt => {
            if l < r {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::Le => {
            if l <= r {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::Gt => {
            if l > r {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::Ge => {
            if l >= r {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::And => {
            if l != 0.0 && r != 0.0 {
                1.0
            } else {
                0.0
            }
        }
        BinaryOp::Or => {
            if l != 0.0 || r != 0.0 {
                1.0
            } else {
                0.0
            }
        }
    };
    Ok(res)
}

fn eval_call<C: EvalContext>(func: &str, args: &[Expr], ctx: &C) -> Result<f64, EvalError> {
    match func {
        "abs" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.abs())
        }
        "sqrt" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            if v < 0.0 {
                return Err(EvalError::DomainError(format!(
                    "sqrt of negative value {v}"
                )));
            }
            Ok(v.sqrt())
        }
        "sin" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.sin())
        }
        "cos" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.cos())
        }
        "tan" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.tan())
        }
        "exp" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.exp())
        }
        "log" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            if v <= 0.0 {
                return Err(EvalError::DomainError(format!(
                    "log of non-positive value {v}"
                )));
            }
            Ok(v.ln())
        }
        "floor" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.floor())
        }
        "ceil" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.ceil())
        }
        "round" => {
            check_arg_count(func, args, 1)?;
            let v = eval_expr(&args[0], ctx)?;
            Ok(v.round())
        }
        "min" => {
            check_arg_count(func, args, 2)?;
            let a = eval_expr(&args[0], ctx)?;
            let b = eval_expr(&args[1], ctx)?;
            Ok(a.min(b))
        }
        "max" => {
            check_arg_count(func, args, 2)?;
            let a = eval_expr(&args[0], ctx)?;
            let b = eval_expr(&args[1], ctx)?;
            Ok(a.max(b))
        }
        "cond" => {
            check_arg_count(func, args, 3)?;
            let cond_res = eval_expr(&args[0], ctx)?;
            if cond_res != 0.0 {
                eval_expr(&args[1], ctx)
            } else {
                eval_expr(&args[2], ctx)
            }
        }
        _ => Err(EvalError::UnknownFunction(func.to_string())),
    }
}

fn check_arg_count(func: &str, args: &[Expr], expected: usize) -> Result<(), EvalError> {
    if args.len() != expected {
        Err(EvalError::WrongArgCount {
            func: func.to_string(),
            expected,
            actual: args.len(),
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TermEvaluation {
    pub label: String,
    pub value: f64,
}

/// Recursively inspects an expression and collects concrete values for all referenced
/// state variables and parameters at a specific evaluation context / time step.
pub fn collect_equation_term_evaluations<C: EvalContext>(
    expr: &Expr,
    ctx: &C,
) -> (Option<f64>, Vec<TermEvaluation>) {
    let mut terms = Vec::new();
    let mut seen = std::collections::HashSet::new();

    fn walk<C: EvalContext>(
        e: &Expr,
        ctx: &C,
        terms: &mut Vec<TermEvaluation>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        match &e.kind {
            ExprKind::StateRef { variable, offset } => {
                let label = if *offset == 0 {
                    format!("{variable}[n]")
                } else if *offset > 0 {
                    format!("{variable}[n+{offset}]")
                } else {
                    format!("{variable}[n{offset}]")
                };
                if seen.insert(label.clone()) {
                    if let Ok(v) = ctx.get_state(variable, *offset) {
                        terms.push(TermEvaluation { label, value: v });
                    }
                }
            }
            ExprKind::Parameter(name) => {
                let label = name.clone();
                if seen.insert(label.clone()) {
                    if let Some(v) = ctx.get_parameter(name) {
                        terms.push(TermEvaluation { label, value: v });
                    } else if let Ok(v) = ctx.get_state(name, 0) {
                        terms.push(TermEvaluation { label, value: v });
                    }
                }
            }
            ExprKind::Unary { expr, .. } => walk(expr, ctx, terms, seen),
            ExprKind::Binary { left, right, .. } => {
                walk(left, ctx, terms, seen);
                walk(right, ctx, terms, seen);
            }
            ExprKind::Cond {
                condition,
                true_val,
                false_val,
            } => {
                walk(condition, ctx, terms, seen);
                walk(true_val, ctx, terms, seen);
                walk(false_val, ctx, terms, seen);
            }
            ExprKind::Call { args, .. } => {
                for a in args {
                    walk(a, ctx, terms, seen);
                }
            }
            ExprKind::Literal(_) => {}
        }
    }

    walk(expr, ctx, &mut terms, &mut seen);
    let overall_val = eval_expr(expr, ctx).ok();
    (overall_val, terms)
}
