use super::{CodegenError, SystemModel};
use crate::ast::{BinaryOp, Equation, Expr, ExprKind, UnaryOp};

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "main",
];

pub fn rust_ident(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("var_{name}")
    } else {
        name.to_string()
    }
}

pub fn format_rust_float(val: f64) -> String {
    if val.is_nan() {
        "f64::NAN".to_string()
    } else if val.is_infinite() {
        if val.is_sign_positive() {
            "f64::INFINITY".to_string()
        } else {
            "f64::NEG_INFINITY".to_string()
        }
    } else if val.fract() == 0.0 && val.abs() < 1e14 {
        format!("{:.1}", val)
    } else {
        format!("{val}")
    }
}

fn var_access(var: &str, offset: i64, model: &SystemModel) -> String {
    let ident = rust_ident(var);
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

pub fn expr_to_rust(
    expr: &Expr,
    model: &SystemModel,
    current_eq: &Equation,
) -> Result<String, CodegenError> {
    match &expr.kind {
        ExprKind::Literal(v) => Ok(format_rust_float(*v)),
        ExprKind::Parameter(name) => match name.as_str() {
            "n" => Ok("(self.n as f64)".to_string()),
            "pi" | "PI" => Ok("std::f64::consts::PI".to_string()),
            "e" | "E" => Ok("std::f64::consts::E".to_string()),
            _ => {
                if model.variables.contains_key(name) {
                    Ok(var_access(name, 0, model))
                } else {
                    Ok(format!("self.{}", rust_ident(name)))
                }
            }
        },
        ExprKind::StateRef { variable, offset } => {
            if *offset == current_eq.target_offset {
                Ok(format!("{}_next", rust_ident(variable)))
            } else {
                Ok(var_access(variable, *offset, model))
            }
        }
        ExprKind::Unary { op, expr } => {
            let inner = expr_to_rust(expr, model, current_eq)?;
            match op {
                UnaryOp::Neg => Ok(format!("(-({inner}))")),
                UnaryOp::Not => Ok(format!("(if ({inner}) == 0.0 {{ 1.0 }} else {{ 0.0 }})")),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let l = expr_to_rust(left, model, current_eq)?;
            let r = expr_to_rust(right, model, current_eq)?;
            let code = match op {
                BinaryOp::Add => format!("(({l}) + ({r}))"),
                BinaryOp::Sub => format!("(({l}) - ({r}))"),
                BinaryOp::Mul => format!("(({l}) * ({r}))"),
                BinaryOp::Div => format!("(({l}) / ({r}))"),
                BinaryOp::Rem => format!("(({l}) % ({r}))"),
                BinaryOp::Pow => format!("(({l}).powf({r}))"),
                BinaryOp::Eq => {
                    format!("(if (({l}) - ({r})).abs() < 1e-12 {{ 1.0 }} else {{ 0.0 }})")
                }
                BinaryOp::Ne => {
                    format!("(if (({l}) - ({r})).abs() >= 1e-12 {{ 1.0 }} else {{ 0.0 }})")
                }
                BinaryOp::Lt => format!("(if ({l}) < ({r}) {{ 1.0 }} else {{ 0.0 }})"),
                BinaryOp::Le => format!("(if ({l}) <= ({r}) {{ 1.0 }} else {{ 0.0 }})"),
                BinaryOp::Gt => format!("(if ({l}) > ({r}) {{ 1.0 }} else {{ 0.0 }})"),
                BinaryOp::Ge => format!("(if ({l}) >= ({r}) {{ 1.0 }} else {{ 0.0 }})"),
                BinaryOp::And => {
                    format!("(if ({l}) != 0.0 && ({r}) != 0.0 {{ 1.0 }} else {{ 0.0 }})")
                }
                BinaryOp::Or => {
                    format!("(if ({l}) != 0.0 || ({r}) != 0.0 {{ 1.0 }} else {{ 0.0 }})")
                }
            };
            Ok(code)
        }
        ExprKind::Cond {
            condition,
            true_val,
            false_val,
        } => {
            let c = expr_to_rust(condition, model, current_eq)?;
            let t = expr_to_rust(true_val, model, current_eq)?;
            let f = expr_to_rust(false_val, model, current_eq)?;
            Ok(format!("(if ({c}) != 0.0 {{ {t} }} else {{ {f} }})"))
        }
        ExprKind::Call { func, args } => {
            let rust_args: Result<Vec<String>, CodegenError> = args
                .iter()
                .map(|a| expr_to_rust(a, model, current_eq))
                .collect();
            let rust_args = rust_args?;

            let code = match func.as_str() {
                "abs" => format!("({}).abs()", rust_args[0]),
                "sqrt" => format!("({}).sqrt()", rust_args[0]),
                "sin" => format!("({}).sin()", rust_args[0]),
                "cos" => format!("({}).cos()", rust_args[0]),
                "tan" => format!("({}).tan()", rust_args[0]),
                "tanh" => format!("({}).tanh()", rust_args[0]),
                "sinh" => format!("({}).sinh()", rust_args[0]),
                "cosh" => format!("({}).cosh()", rust_args[0]),
                "atan" => format!("({}).atan()", rust_args[0]),
                "exp" => format!("({}).exp()", rust_args[0]),
                "log" => format!("({}).ln()", rust_args[0]),
                "floor" => format!("({}).floor()", rust_args[0]),
                "ceil" => format!("({}).ceil()", rust_args[0]),
                "round" => format!("({}).round()", rust_args[0]),
                "min" => format!("({}).min({})", rust_args[0], rust_args[1]),
                "max" => format!("({}).max({})", rust_args[0], rust_args[1]),
                "cond" => format!(
                    "(if ({}) != 0.0 {{ {} }} else {{ {} }})",
                    rust_args[0], rust_args[1], rust_args[2]
                ),
                "clamp" => format!(
                    "({}).clamp({}, {})",
                    rust_args[0], rust_args[1], rust_args[2]
                ),
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

pub fn generate_rust_code(model: &SystemModel) -> Result<String, CodegenError> {
    let mut out = String::new();

    out.push_str("//! Auto-generated by difference_equation_calc\n\n");

    // Output Struct
    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str("pub struct DiffEqOutputs {\n");
    for var_name in &model.eval_order {
        out.push_str(&format!("    pub {}: f64,\n", rust_ident(var_name)));
    }
    out.push_str("}\n\n");

    // System Struct
    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str("pub struct DiffEqSystem {\n");
    out.push_str("    pub n: usize,\n");

    if !model.parameters.is_empty() {
        out.push_str("    // Parameters\n");
        for p in &model.parameters {
            out.push_str(&format!("    pub {}: f64,\n", rust_ident(&p.name)));
        }
    }

    out.push_str("    // State Registers\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = rust_ident(var_name);
        if v.max_delay == 0 {
            out.push_str(&format!("    pub {ident}: f64,\n"));
        } else {
            for d in 0..=v.max_delay {
                out.push_str(&format!("    pub {ident}_{d}: f64,\n"));
            }
        }
    }
    out.push_str("}\n\n");

    // Default impl
    out.push_str("impl Default for DiffEqSystem {\n");
    out.push_str("    fn default() -> Self {\n");
    out.push_str("        Self::new()\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // System impl
    out.push_str("impl DiffEqSystem {\n");
    out.push_str("    pub fn new() -> Self {\n");
    out.push_str("        Self {\n");
    out.push_str("            n: 0,\n");
    for p in &model.parameters {
        out.push_str(&format!(
            "            {}: {},\n",
            rust_ident(&p.name),
            format_rust_float(p.default_value)
        ));
    }
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = rust_ident(var_name);
        let init_str = format_rust_float(v.initial_value);
        if v.max_delay == 0 {
            out.push_str(&format!("            {ident}: {init_str},\n"));
        } else {
            for d in 0..=v.max_delay {
                out.push_str(&format!("            {ident}_{d}: {init_str},\n"));
            }
        }
    }
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    out.push_str("    pub fn reset(&mut self) {\n");
    out.push_str("        *self = Self::new();\n");
    out.push_str("    }\n\n");

    out.push_str("    pub fn step(&mut self) -> DiffEqOutputs {\n");
    for var_name in &model.eval_order {
        let eq = &model.equations[var_name];
        let rhs_rust = expr_to_rust(&eq.rhs, model, eq)?;
        out.push_str(&format!(
            "        let {}_next = {rhs_rust};\n",
            rust_ident(var_name)
        ));
    }

    out.push_str("\n        // Update state delay registers\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = rust_ident(var_name);
        if v.max_delay == 0 {
            out.push_str(&format!("        self.{ident} = {ident}_next;\n"));
        } else {
            for d in (1..=v.max_delay).rev() {
                out.push_str(&format!(
                    "        self.{ident}_{d} = self.{ident}_{};\n",
                    d - 1
                ));
            }
            out.push_str(&format!("        self.{ident}_0 = {ident}_next;\n"));
        }
    }
    out.push_str("        self.n += 1;\n\n");

    out.push_str("        DiffEqOutputs {\n");
    for var_name in &model.eval_order {
        out.push_str(&format!(
            "            {}: {}_next,\n",
            rust_ident(var_name),
            rust_ident(var_name)
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    Ok(out)
}

pub fn generate_rust_standalone_runner(
    model: &SystemModel,
    sample_count: usize,
) -> Result<String, CodegenError> {
    let mut out = generate_rust_code(model)?;

    out.push_str("\n// Standalone Test / CLI Execution Harness\n");
    out.push_str("fn main() {\n");
    out.push_str("    let mut sys = DiffEqSystem::new();\n\n");

    out.push_str("    // Header\n");
    out.push_str("    print!(\"# step\");\n");
    for var in &model.eval_order {
        out.push_str(&format!("    print!(\"\\t{}\");\n", rust_ident(var)));
    }
    out.push_str("    println!();\n\n");

    out.push_str("    // Initial condition at step 0\n");
    out.push_str("    print!(\"0\");\n");
    for var in &model.eval_order {
        let v = &model.variables[var];
        let ident = rust_ident(var);
        if v.max_delay == 0 {
            out.push_str(&format!("    print!(\"\\t{{:.17}}\", sys.{ident});\n"));
        } else {
            out.push_str(&format!("    print!(\"\\t{{:.17}}\", sys.{ident}_0);\n"));
        }
    }
    out.push_str("    println!();\n\n");

    out.push_str(&format!(
        "    for _step in 0..{} {{\n",
        sample_count.saturating_sub(1)
    ));
    out.push_str("        let out = sys.step();\n");
    out.push_str("        print!(\"{}\", sys.n);\n");
    for var in &model.eval_order {
        out.push_str(&format!(
            "        print!(\"\\t{{:.17}}\", out.{});\n",
            rust_ident(var)
        ));
    }
    out.push_str("        println!();\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    Ok(out)
}
