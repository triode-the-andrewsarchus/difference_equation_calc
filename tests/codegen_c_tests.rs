#![allow(clippy::needless_range_loop)]

use difference_equation_calc::codegen::c::generate_c_standalone_runner;
use difference_equation_calc::codegen::SystemModel;
use difference_equation_calc::document::DocumentState;
use difference_equation_calc::parser::parse_equation_str;
use difference_equation_calc::sim::run_simulation;
use std::collections::HashMap;
use std::process::Command;

fn run_c_harness(c_source: &str, sample_count: usize) -> Result<HashMap<String, Vec<f64>>, String> {
    let test_id = format!(
        "diffeq_c_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let src_path = format!("/tmp/{test_id}.c");
    let bin_path = format!("/tmp/{test_id}.bin");

    std::fs::write(&src_path, c_source).map_err(|e| format!("Failed to write source: {e}"))?;

    let gcc_output = Command::new("gcc")
        .args(["-O2", &src_path, "-lm", "-o", &bin_path])
        .output()
        .map_err(|e| format!("Failed to execute gcc: {e}"))?;

    if !gcc_output.status.success() {
        let stderr = String::from_utf8_lossy(&gcc_output.stderr);
        let _ = std::fs::remove_file(&src_path);
        return Err(format!("GCC compilation failed:\n{stderr}"));
    }

    let run_output = Command::new(&bin_path)
        .output()
        .map_err(|e| format!("Failed to run compiled C binary: {e}"))?;

    let _ = std::fs::remove_file(&src_path);
    let _ = std::fs::remove_file(&bin_path);

    if !run_output.status.success() {
        let stderr = String::from_utf8_lossy(&run_output.stderr);
        return Err(format!("C binary execution failed:\n{stderr}"));
    }

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.is_empty() {
        return Err("No output from C binary".to_string());
    }

    let header_line = lines[0];
    let col_names: Vec<String> = header_line
        .trim_start_matches('#')
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    if col_names.len() <= 1 {
        return Err(format!("Invalid header in C output: {header_line}"));
    }

    // col_names[0] is "step", following are variable names
    let var_names = &col_names[1..];
    let mut series_map: HashMap<String, Vec<f64>> = HashMap::new();
    for var in var_names {
        series_map.insert(var.clone(), Vec::with_capacity(sample_count));
    }

    for (line_idx, line) in lines.iter().enumerate().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() != col_names.len() {
            return Err(format!(
                "Line {line_idx} has {} columns, expected {}",
                parts.len(),
                col_names.len()
            ));
        }

        for (var_idx, var) in var_names.iter().enumerate() {
            let val: f64 = parts[var_idx + 1].parse().map_err(|e| {
                format!(
                    "Failed to parse float '{}' at line {line_idx}: {e}",
                    parts[var_idx + 1]
                )
            })?;
            series_map.get_mut(var).unwrap().push(val);
        }
    }

    Ok(series_map)
}

#[test]
fn test_c_codegen_sample_accuracy_victor_oscillator() {
    let doc = DocumentState::victor_oscillator_demo();
    let (equations, parse_errs) = doc.get_parsed_equations();
    assert!(parse_errs.is_empty());

    let sample_count = 150;
    let params = doc.get_parameters_map();

    // 1. Reference simulation in Rust
    let sim_result = run_simulation(&equations, &doc.initial_conditions, &params, sample_count)
        .expect("Simulation should succeed");

    // 2. Generate C standalone code
    let model = SystemModel::from_equations(&equations, &doc.initial_conditions, &params)
        .expect("Model creation should succeed");

    let c_code =
        generate_c_standalone_runner(&model, sample_count).expect("C codegen should succeed");

    // 3. Compile and execute C code
    let c_result = run_c_harness(&c_code, sample_count).expect("C test harness should succeed");

    // 4. Assert sample-by-sample equivalence
    for var in &model.eval_order {
        let sim_series = sim_result.series.get(var).expect("sim series missing");
        let c_series = c_result.get(var).expect("c series missing");

        assert_eq!(
            sim_series.values.len(),
            c_series.len(),
            "Series length mismatch for var '{var}'"
        );

        for step in 0..sample_count {
            let sim_val = sim_series.values[step];
            let c_val = c_series[step];
            let diff = (sim_val - c_val).abs();
            assert!(
                diff < 1e-10,
                "Sample accuracy failure at step {step} for '{var}': sim={sim_val}, c={c_val}, diff={diff}"
            );
        }
    }
}

#[test]
fn test_c_codegen_sample_accuracy_with_past_delays() {
    // Difference equation referencing past delay taps: y[n] and y[n-1]
    let eq1 = parse_equation_str("y[n+1] = 1.35 * y[n] - 0.65 * y[n-1] + 0.1").unwrap();
    let equations = vec![eq1];

    let mut inits = HashMap::new();
    inits.insert("y".to_string(), 1.0);

    let params = HashMap::new();
    let sample_count = 100;

    let sim_result = run_simulation(&equations, &inits, &params, sample_count).unwrap();

    let model = SystemModel::from_equations(&equations, &inits, &params).unwrap();
    assert_eq!(model.variables["y"].max_delay, 1);

    let c_code = generate_c_standalone_runner(&model, sample_count).unwrap();
    let c_result = run_c_harness(&c_code, sample_count).unwrap();

    let sim_y = &sim_result.series["y"].values;
    let c_y = &c_result["y"];

    for step in 0..sample_count {
        let diff = (sim_y[step] - c_y[step]).abs();
        assert!(
            diff < 1e-11,
            "Delay tap mismatch at step {step}: sim={}, c={}, diff={diff}",
            sim_y[step],
            c_y[step]
        );
    }
}

#[test]
fn test_c_codegen_sample_accuracy_nonlinear_functions() {
    let eq1 = parse_equation_str("x[n+1] = sin(x[n]) + 0.2 * tanh(y[n]) + min(x[n], 0.4)").unwrap();
    let eq2 = parse_equation_str("y[n+1] = clamp(y[n] * 0.85 + cos(x[n+1]), -1.2, 1.2)").unwrap();
    let equations = vec![eq1, eq2];

    let mut inits = HashMap::new();
    inits.insert("x".to_string(), 0.5);
    inits.insert("y".to_string(), -0.3);

    let params = HashMap::new();
    let sample_count = 120;

    let sim_result = run_simulation(&equations, &inits, &params, sample_count).unwrap();

    let model = SystemModel::from_equations(&equations, &inits, &params).unwrap();
    let c_code = generate_c_standalone_runner(&model, sample_count).unwrap();
    let c_result = run_c_harness(&c_code, sample_count).unwrap();

    for var in ["x", "y"] {
        let sim_vals = &sim_result.series[var].values;
        let c_vals = &c_result[var];
        for step in 0..sample_count {
            let diff = (sim_vals[step] - c_vals[step]).abs();
            assert!(
                diff < 1e-10,
                "Nonlinear function mismatch for '{var}' at step {step}: sim={}, c={}, diff={diff}",
                sim_vals[step],
                c_vals[step]
            );
        }
    }
}

#[test]
fn test_c_codegen_preset_chamberlin_svf() {
    use difference_equation_calc::presets::{get_preset_by_kind, PresetKind};
    let preset_spec = get_preset_by_kind(PresetKind::ChamberlinSvf);
    let preset = (preset_spec.state_builder)();
    let (equations, parse_errs) = preset.get_parsed_equations();
    assert!(parse_errs.is_empty());

    let params = preset.get_parameters_map();
    let sample_count = 150;

    let sim_result = run_simulation(
        &equations,
        &preset.initial_conditions,
        &params,
        sample_count,
    )
    .unwrap();
    let model =
        SystemModel::from_equations(&equations, &preset.initial_conditions, &params).unwrap();
    let c_code = generate_c_standalone_runner(&model, sample_count).unwrap();
    let c_result = run_c_harness(&c_code, sample_count).unwrap();

    for var in &model.eval_order {
        let sim_vals = &sim_result.series[var].values;
        let c_vals = &c_result[var];
        for step in 0..sample_count {
            let diff = (sim_vals[step] - c_vals[step]).abs();
            assert!(
                diff < 1e-10,
                "Chamberlin SVF mismatch for '{var}' at step {step}: sim={}, c={}, diff={diff}",
                sim_vals[step],
                c_vals[step]
            );
        }
    }
}

#[test]
fn test_c_codegen_preset_resonant_biquad() {
    use difference_equation_calc::presets::{get_preset_by_kind, PresetKind};
    let preset_spec = get_preset_by_kind(PresetKind::ResonantBiquad);
    let preset = (preset_spec.state_builder)();
    let (equations, parse_errs) = preset.get_parsed_equations();
    assert!(parse_errs.is_empty());

    let params = preset.get_parameters_map();
    let sample_count = 150;

    let sim_result = run_simulation(
        &equations,
        &preset.initial_conditions,
        &params,
        sample_count,
    )
    .unwrap();
    let model =
        SystemModel::from_equations(&equations, &preset.initial_conditions, &params).unwrap();
    let c_code = generate_c_standalone_runner(&model, sample_count).unwrap();
    let c_result = run_c_harness(&c_code, sample_count).unwrap();

    for var in &model.eval_order {
        let sim_vals = &sim_result.series[var].values;
        let c_vals = &c_result[var];
        for step in 0..sample_count {
            let diff = (sim_vals[step] - c_vals[step]).abs();
            assert!(
                diff < 1e-10,
                "Biquad mismatch for '{var}' at step {step}: sim={}, c={}, diff={diff}",
                sim_vals[step],
                c_vals[step]
            );
        }
    }
}
