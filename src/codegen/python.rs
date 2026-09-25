use super::{CodegenError, SystemModel};
use crate::ast::{BinaryOp, Equation, Expr, ExprKind, UnaryOp};

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

pub fn py_ident(name: &str) -> String {
    if PYTHON_KEYWORDS.contains(&name) {
        format!("var_{name}")
    } else {
        name.to_string()
    }
}

pub fn format_py_float(val: f64) -> String {
    if val.is_nan() {
        "float('nan')".to_string()
    } else if val.is_infinite() {
        if val.is_sign_positive() {
            "float('inf')".to_string()
        } else {
            "float('-inf')".to_string()
        }
    } else if val.fract() == 0.0 && val.abs() < 1e14 {
        format!("{:.1}", val)
    } else {
        format!("{val}")
    }
}

fn var_access(var: &str, offset: i64, model: &SystemModel) -> String {
    let ident = py_ident(var);
    let max_delay = model.variables.get(var).map(|v| v.max_delay).unwrap_or(0);

    if max_delay == 0 {
        format!("self.{ident}")
    } else if offset == 0 {
        format!("self.{ident}_0")
    } else {
        let delay_idx = (-offset) as usize;
        format!("self.{ident}_{delay_idx}")
    }
}

pub fn expr_to_py(
    expr: &Expr,
    model: &SystemModel,
    current_eq: &Equation,
) -> Result<String, CodegenError> {
    match &expr.kind {
        ExprKind::Literal(v) => Ok(format_py_float(*v)),
        ExprKind::Parameter(name) => match name.as_str() {
            "n" => Ok("float(self.n)".to_string()),
            "pi" | "PI" => Ok("math.pi".to_string()),
            "e" | "E" => Ok("math.e".to_string()),
            _ => {
                if model.variables.contains_key(name) {
                    Ok(var_access(name, 0, model))
                } else {
                    Ok(format!("self.{}", py_ident(name)))
                }
            }
        },
        ExprKind::StateRef { variable, offset } => {
            if *offset == current_eq.target_offset {
                Ok(format!("{}_next", py_ident(variable)))
            } else {
                Ok(var_access(variable, *offset, model))
            }
        }
        ExprKind::Unary { op, expr } => {
            let inner = expr_to_py(expr, model, current_eq)?;
            match op {
                UnaryOp::Neg => Ok(format!("(-({inner}))")),
                UnaryOp::Not => Ok(format!("(1.0 if ({inner}) == 0.0 else 0.0)")),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let l = expr_to_py(left, model, current_eq)?;
            let r = expr_to_py(right, model, current_eq)?;
            let code = match op {
                BinaryOp::Add => format!("(({l}) + ({r}))"),
                BinaryOp::Sub => format!("(({l}) - ({r}))"),
                BinaryOp::Mul => format!("(({l}) * ({r}))"),
                BinaryOp::Div => format!("(({l}) / ({r}))"),
                BinaryOp::Rem => format!("math.fmod(({l}), ({r}))"),
                BinaryOp::Pow => format!("math.pow(({l}), ({r}))"),
                BinaryOp::Eq => {
                    format!("(1.0 if abs(({l}) - ({r})) < 1e-12 else 0.0)")
                }
                BinaryOp::Ne => {
                    format!("(1.0 if abs(({l}) - ({r})) >= 1e-12 else 0.0)")
                }
                BinaryOp::Lt => format!("(1.0 if ({l}) < ({r}) else 0.0)"),
                BinaryOp::Le => format!("(1.0 if ({l}) <= ({r}) else 0.0)"),
                BinaryOp::Gt => format!("(1.0 if ({l}) > ({r}) else 0.0)"),
                BinaryOp::Ge => format!("(1.0 if ({l}) >= ({r}) else 0.0)"),
                BinaryOp::And => {
                    format!("(1.0 if (({l}) != 0.0 and ({r}) != 0.0) else 0.0)")
                }
                BinaryOp::Or => {
                    format!("(1.0 if (({l}) != 0.0 or ({r}) != 0.0) else 0.0)")
                }
            };
            Ok(code)
        }
        ExprKind::Cond {
            condition,
            true_val,
            false_val,
        } => {
            let c = expr_to_py(condition, model, current_eq)?;
            let t = expr_to_py(true_val, model, current_eq)?;
            let f = expr_to_py(false_val, model, current_eq)?;
            Ok(format!("(({t}) if ({c}) != 0.0 else ({f}))"))
        }
        ExprKind::Call { func, args } => {
            let py_args: Result<Vec<String>, CodegenError> = args
                .iter()
                .map(|a| expr_to_py(a, model, current_eq))
                .collect();
            let py_args = py_args?;

            let code = match func.as_str() {
                "abs" => format!("abs({})", py_args[0]),
                "sqrt" => format!("math.sqrt({})", py_args[0]),
                "sin" => format!("math.sin({})", py_args[0]),
                "cos" => format!("math.cos({})", py_args[0]),
                "tan" => format!("math.tan({})", py_args[0]),
                "tanh" => format!("math.tanh({})", py_args[0]),
                "sinh" => format!("math.sinh({})", py_args[0]),
                "cosh" => format!("math.cosh({})", py_args[0]),
                "atan" => format!("math.atan({})", py_args[0]),
                "exp" => format!("math.exp({})", py_args[0]),
                "log" => format!("math.log({})", py_args[0]),
                "floor" => format!("float(math.floor({}))", py_args[0]),
                "ceil" => format!("float(math.ceil({}))", py_args[0]),
                "round" => format!("float(round({}))", py_args[0]),
                "min" => format!("min({}, {})", py_args[0], py_args[1]),
                "max" => format!("max({}, {})", py_args[0], py_args[1]),
                "cond" => format!(
                    "(({}) if ({}) != 0.0 else ({}))",
                    py_args[1], py_args[0], py_args[2]
                ),
                "clamp" => format!("min(max({}, {}), {})", py_args[0], py_args[1], py_args[2]),
                _ => {
                    return Err(CodegenError::UnsupportedExpr(format!(
                        "Unknown function '{func}'"
                    )))
                }
            };
            Ok(code)
        }
    }
}

pub fn generate_python_code(model: &SystemModel) -> Result<String, CodegenError> {
    let mut out = String::new();

    out.push_str("\"\"\"Auto-generated by difference_equation_calc\"\"\"\n\n");
    out.push_str("import math\n");
    out.push_str("from typing import NamedTuple\n\n\n");

    // Output NamedTuple
    out.push_str("class DiffEqOutputs(NamedTuple):\n");
    for var_name in &model.eval_order {
        out.push_str(&format!("    {}: float\n", py_ident(var_name)));
    }
    out.push_str("\n\n");

    // System Class
    out.push_str("class DiffEqSystem:\n");
    out.push_str("    def __init__(self):\n");
    out.push_str("        self.n: int = 0\n");

    if !model.parameters.is_empty() {
        out.push_str("        # Parameters\n");
        for p in &model.parameters {
            out.push_str(&format!(
                "        self.{}: float = {}\n",
                py_ident(&p.name),
                format_py_float(p.default_value)
            ));
        }
    }

    out.push_str("        # State Registers\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = py_ident(var_name);
        let init_str = format_py_float(v.initial_value);
        if v.max_delay == 0 {
            out.push_str(&format!("        self.{ident}: float = {init_str}\n"));
        } else {
            for d in 0..=v.max_delay {
                out.push_str(&format!("        self.{ident}_{d}: float = {init_str}\n"));
            }
        }
    }
    out.push('\n');

    out.push_str("    def reset(self):\n");
    out.push_str("        self.__init__()\n\n");

    out.push_str("    def step(self) -> DiffEqOutputs:\n");
    for var_name in &model.eval_order {
        let eq = &model.equations[var_name];
        let rhs_py = expr_to_py(&eq.rhs, model, eq)?;
        out.push_str(&format!("        {}_next = {rhs_py}\n", py_ident(var_name)));
    }

    out.push_str("\n        # Update state delay registers\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = py_ident(var_name);
        if v.max_delay == 0 {
            out.push_str(&format!("        self.{ident} = {ident}_next\n"));
        } else {
            for d in (1..=v.max_delay).rev() {
                out.push_str(&format!(
                    "        self.{ident}_{d} = self.{ident}_{}\n",
                    d - 1
                ));
            }
            out.push_str(&format!("        self.{ident}_0 = {ident}_next\n"));
        }
    }
    out.push_str("        self.n += 1\n\n");

    out.push_str("        return DiffEqOutputs(\n");
    for var_name in &model.eval_order {
        let ident = py_ident(var_name);
        out.push_str(&format!("            {ident}={ident}_next,\n"));
    }
    out.push_str("        )\n");

    Ok(out)
}

pub fn generate_python_standalone_runner(
    model: &SystemModel,
    sample_count: usize,
) -> Result<String, CodegenError> {
    let mut out = generate_python_code(model)?;

    out.push_str("\n\n# Standalone Test / CLI Execution Harness\n");
    out.push_str("if __name__ == '__main__':\n");
    out.push_str("    sys = DiffEqSystem()\n\n");

    out.push_str("    # Header\n");
    out.push_str("    header = ['# step'] + [");
    let header_items: Vec<String> = model
        .eval_order
        .iter()
        .map(|v| format!("'{}'", py_ident(v)))
        .collect();
    out.push_str(&header_items.join(", "));
    out.push_str("]\n");
    out.push_str("    print('\\t'.join(header))\n\n");

    out.push_str("    # Initial condition at step 0\n");
    out.push_str("    init_vals = ['0'] + [\n");
    for var in &model.eval_order {
        let v = &model.variables[var];
        let ident = py_ident(var);
        if v.max_delay == 0 {
            out.push_str(&format!("        f'{{sys.{ident}:.17g}}',\n"));
        } else {
            out.push_str(&format!("        f'{{sys.{ident}_0:.17g}}',\n"));
        }
    }
    out.push_str("    ]\n");
    out.push_str("    print('\\t'.join(init_vals))\n\n");

    out.push_str(&format!(
        "    for _step in range({}):\n",
        sample_count.saturating_sub(1)
    ));
    out.push_str("        out = sys.step()\n");
    out.push_str("        row = [str(sys.n)] + [\n");
    for var in &model.eval_order {
        let ident = py_ident(var);
        out.push_str(&format!("            f'{{out.{ident}:.17g}}',\n"));
    }
    out.push_str("        ]\n");
    out.push_str("        print('\\t'.join(row))\n");

    Ok(out)
}
