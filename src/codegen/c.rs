use super::{CodegenError, SystemModel};
use crate::ast::{BinaryOp, Equation, Expr, ExprKind, UnaryOp};

const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while", "main",
];

pub fn c_ident(name: &str) -> String {
    if C_KEYWORDS.contains(&name) {
        format!("var_{name}")
    } else {
        name.to_string()
    }
}

pub fn format_c_float(val: f64) -> String {
    if val.is_nan() {
        "NAN".to_string()
    } else if val.is_infinite() {
        if val.is_sign_positive() {
            "INFINITY".to_string()
        } else {
            "(-INFINITY)".to_string()
        }
    } else if val.fract() == 0.0 && val.abs() < 1e14 {
        format!("{:.1}", val)
    } else {
        format!("{val}")
    }
}

fn var_access(var: &str, offset: i64, model: &SystemModel) -> String {
    let ident = c_ident(var);
    let max_delay = model.variables.get(var).map(|v| v.max_delay).unwrap_or(0);

    if max_delay == 0 {
        format!("sys->{ident}")
    } else if offset == 0 {
        format!("sys->{ident}_0")
    } else {
        let delay_idx = (-offset) as usize;
        format!("sys->{ident}_{delay_idx}")
    }
}

pub fn expr_to_c(
    expr: &Expr,
    model: &SystemModel,
    current_eq: &Equation,
) -> Result<String, CodegenError> {
    match &expr.kind {
        ExprKind::Literal(v) => Ok(format_c_float(*v)),
        ExprKind::Parameter(name) => match name.as_str() {
            "n" => Ok("((double)sys->n)".to_string()),
            "pi" | "PI" => Ok("3.14159265358979323846".to_string()),
            "e" | "E" => Ok("2.71828182845904523536".to_string()),
            _ => {
                if model.variables.contains_key(name) {
                    Ok(var_access(name, 0, model))
                } else {
                    Ok(format!("sys->{}", c_ident(name)))
                }
            }
        },
        ExprKind::StateRef { variable, offset } => {
            if *offset == current_eq.target_offset {
                Ok(format!("{}_next", c_ident(variable)))
            } else {
                Ok(var_access(variable, *offset, model))
            }
        }
        ExprKind::Unary { op, expr } => {
            let inner = expr_to_c(expr, model, current_eq)?;
            match op {
                UnaryOp::Neg => Ok(format!("(-({inner}))")),
                UnaryOp::Not => Ok(format!("(({inner}) == 0.0 ? 1.0 : 0.0)")),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let l = expr_to_c(left, model, current_eq)?;
            let r = expr_to_c(right, model, current_eq)?;
            let code = match op {
                BinaryOp::Add => format!("(({l}) + ({r}))"),
                BinaryOp::Sub => format!("(({l}) - ({r}))"),
                BinaryOp::Mul => format!("(({l}) * ({r}))"),
                BinaryOp::Div => format!("(({l}) / ({r}))"),
                BinaryOp::Rem => format!("fmod(({l}), ({r}))"),
                BinaryOp::Pow => format!("pow(({l}), ({r}))"),
                BinaryOp::Eq => format!("(fabs(({l}) - ({r})) < 1e-12 ? 1.0 : 0.0)"),
                BinaryOp::Ne => format!("(fabs(({l}) - ({r})) >= 1e-12 ? 1.0 : 0.0)"),
                BinaryOp::Lt => format!("(({l}) < ({r}) ? 1.0 : 0.0)"),
                BinaryOp::Le => format!("(({l}) <= ({r}) ? 1.0 : 0.0)"),
                BinaryOp::Gt => format!("(({l}) > ({r}) ? 1.0 : 0.0)"),
                BinaryOp::Ge => format!("(({l}) >= ({r}) ? 1.0 : 0.0)"),
                BinaryOp::And => format!("((({l}) != 0.0 && ({r}) != 0.0) ? 1.0 : 0.0)"),
                BinaryOp::Or => format!("((({l}) != 0.0 || ({r}) != 0.0) ? 1.0 : 0.0)"),
            };
            Ok(code)
        }
        ExprKind::Cond {
            condition,
            true_val,
            false_val,
        } => {
            let c = expr_to_c(condition, model, current_eq)?;
            let t = expr_to_c(true_val, model, current_eq)?;
            let f = expr_to_c(false_val, model, current_eq)?;
            Ok(format!("(({c}) != 0.0 ? ({t}) : ({f}))"))
        }
        ExprKind::Call { func, args } => {
            let c_args: Result<Vec<String>, CodegenError> = args
                .iter()
                .map(|a| expr_to_c(a, model, current_eq))
                .collect();
            let c_args = c_args?;

            let code = match func.as_str() {
                "abs" => format!("fabs({})", c_args[0]),
                "sqrt" => format!("sqrt({})", c_args[0]),
                "sin" => format!("sin({})", c_args[0]),
                "cos" => format!("cos({})", c_args[0]),
                "tan" => format!("tan({})", c_args[0]),
                "tanh" => format!("tanh({})", c_args[0]),
                "sinh" => format!("sinh({})", c_args[0]),
                "cosh" => format!("cosh({})", c_args[0]),
                "atan" => format!("atan({})", c_args[0]),
                "exp" => format!("exp({})", c_args[0]),
                "log" => format!("log({})", c_args[0]),
                "floor" => format!("floor({})", c_args[0]),
                "ceil" => format!("ceil({})", c_args[0]),
                "round" => format!("round({})", c_args[0]),
                "min" => format!("fmin({}, {})", c_args[0], c_args[1]),
                "max" => format!("fmax({}, {})", c_args[0], c_args[1]),
                "cond" => format!(
                    "(({}) != 0.0 ? ({}) : ({}))",
                    c_args[0], c_args[1], c_args[2]
                ),
                "clamp" => format!("fmin(fmax({}, {}), {})", c_args[0], c_args[1], c_args[2]),
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

pub fn generate_c_code(model: &SystemModel) -> Result<String, CodegenError> {
    let mut out = String::new();

    out.push_str("/* Auto-generated by difference_equation_calc */\n");
    out.push_str("#ifndef DIFFEQ_SYSTEM_H\n");
    out.push_str("#define DIFFEQ_SYSTEM_H\n\n");
    out.push_str("#include <math.h>\n");
    out.push_str("#include <stdint.h>\n");
    out.push_str("#include <stddef.h>\n\n");
    out.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");

    // DiffEqSystem Struct
    out.push_str("typedef struct {\n");
    out.push_str("    uint64_t n;\n");

    if !model.parameters.is_empty() {
        out.push_str("    /* Parameters */\n");
        for p in &model.parameters {
            out.push_str(&format!("    double {};\n", c_ident(&p.name)));
        }
    }

    out.push_str("    /* State Registers */\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = c_ident(var_name);
        if v.max_delay == 0 {
            out.push_str(&format!("    double {ident};\n"));
        } else {
            for d in 0..=v.max_delay {
                out.push_str(&format!("    double {ident}_{d};\n"));
            }
        }
    }
    out.push_str("} DiffEqSystem;\n\n");

    // Output Struct
    out.push_str("typedef struct {\n");
    for var_name in &model.eval_order {
        out.push_str(&format!("    double {};\n", c_ident(var_name)));
    }
    out.push_str("} DiffEqOutputs;\n\n");

    // Function signatures
    out.push_str("void diffeq_init(DiffEqSystem* sys);\n");
    out.push_str("void diffeq_reset(DiffEqSystem* sys);\n");
    out.push_str("DiffEqOutputs diffeq_step(DiffEqSystem* sys);\n\n");

    // Implementation
    out.push_str("void diffeq_init(DiffEqSystem* sys) {\n");
    out.push_str("    sys->n = 0;\n");
    for p in &model.parameters {
        out.push_str(&format!(
            "    sys->{} = {};\n",
            c_ident(&p.name),
            format_c_float(p.default_value)
        ));
    }
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = c_ident(var_name);
        let init_str = format_c_float(v.initial_value);
        if v.max_delay == 0 {
            out.push_str(&format!("    sys->{ident} = {init_str};\n"));
        } else {
            for d in 0..=v.max_delay {
                out.push_str(&format!("    sys->{ident}_{d} = {init_str};\n"));
            }
        }
    }
    out.push_str("}\n\n");

    out.push_str("void diffeq_reset(DiffEqSystem* sys) {\n");
    out.push_str("    sys->n = 0;\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = c_ident(var_name);
        let init_str = format_c_float(v.initial_value);
        if v.max_delay == 0 {
            out.push_str(&format!("    sys->{ident} = {init_str};\n"));
        } else {
            for d in 0..=v.max_delay {
                out.push_str(&format!("    sys->{ident}_{d} = {init_str};\n"));
            }
        }
    }
    out.push_str("}\n\n");

    out.push_str("DiffEqOutputs diffeq_step(DiffEqSystem* sys) {\n");
    for var_name in &model.eval_order {
        let eq = &model.equations[var_name];
        let rhs_c = expr_to_c(&eq.rhs, model, eq)?;
        out.push_str(&format!(
            "    double {}_next = {rhs_c};\n",
            c_ident(var_name)
        ));
    }

    out.push_str("\n    /* Update state delay registers */\n");
    for var_name in &model.eval_order {
        let v = &model.variables[var_name];
        let ident = c_ident(var_name);
        if v.max_delay == 0 {
            out.push_str(&format!("    sys->{ident} = {ident}_next;\n"));
        } else {
            for d in (1..=v.max_delay).rev() {
                out.push_str(&format!("    sys->{ident}_{d} = sys->{ident}_{};\n", d - 1));
            }
            out.push_str(&format!("    sys->{ident}_0 = {ident}_next;\n"));
        }
    }
    out.push_str("    sys->n++;\n\n");

    out.push_str("    DiffEqOutputs out;\n");
    for var_name in &model.eval_order {
        out.push_str(&format!(
            "    out.{} = {}_next;\n",
            c_ident(var_name),
            c_ident(var_name)
        ));
    }
    out.push_str("    return out;\n");
    out.push_str("}\n\n");

    out.push_str("#ifdef __cplusplus\n}\n#endif\n\n");
    out.push_str("#endif /* DIFFEQ_SYSTEM_H */\n");

    Ok(out)
}

pub fn generate_c_standalone_runner(
    model: &SystemModel,
    sample_count: usize,
) -> Result<String, CodegenError> {
    let mut out = generate_c_code(model)?;

    out.push_str("\n/* Standalone Test / CLI Execution Harness */\n");
    out.push_str("#include <stdio.h>\n\n");
    out.push_str("int main(void) {\n");
    out.push_str("    DiffEqSystem sys;\n");
    out.push_str("    diffeq_init(&sys);\n\n");

    out.push_str("    /* Header */\n");
    out.push_str("    printf(\"# step");
    for var in &model.eval_order {
        out.push_str(&format!("\\t{}", c_ident(var)));
    }
    out.push_str("\\n\");\n\n");

    out.push_str("    /* Initial condition at step 0 */\n");
    out.push_str("    printf(\"0");
    for _ in &model.eval_order {
        out.push_str("\\t%.17g");
    }
    out.push_str("\\n\"");
    for var in &model.eval_order {
        let v = &model.variables[var];
        let ident = c_ident(var);
        if v.max_delay == 0 {
            out.push_str(&format!(", sys.{ident}"));
        } else {
            out.push_str(&format!(", sys.{ident}_0"));
        }
    }
    out.push_str(");\n\n");

    out.push_str(&format!(
        "    for (size_t step = 0; step < {}; step++) {{\n",
        sample_count.saturating_sub(1)
    ));
    out.push_str("        DiffEqOutputs out = diffeq_step(&sys);\n");
    out.push_str("        printf(\"%zu\"");
    for _ in &model.eval_order {
        out.push_str(" \"\\t%.17g\"");
    }
    out.push_str(", sys.n");
    for var in &model.eval_order {
        out.push_str(&format!(", out.{}", c_ident(var)));
    }
    out.push_str(");\n");
    out.push_str("        printf(\"\\n\");\n");
    out.push_str("    }\n");
    out.push_str("    return 0;\n");
    out.push_str("}\n");

    Ok(out)
}
