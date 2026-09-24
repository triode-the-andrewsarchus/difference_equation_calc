use difference_equation_calc::ast::ExprKind;
use difference_equation_calc::deps::{analyze_dependencies, DependencyError};
use difference_equation_calc::document::{Document, DocumentState, ParameterDoc};
use difference_equation_calc::eval::{eval_expr, SlicedContext};
use difference_equation_calc::measure::{calculate_measurement, MeasurementSpec, MeasurementType};
use difference_equation_calc::parser::{parse_equation_str, parse_expression_str};
use difference_equation_calc::query::execute_search;
use difference_equation_calc::sim::run_simulation;
use difference_equation_calc::sweep::{execute_parameter_sweep, SweepConfig};
use std::collections::HashMap;

// 1. Arithmetic precedence
#[test]
fn test_arithmetic_precedence() {
    let empty_history = HashMap::new();
    let params = HashMap::new();
    let ctx = SlicedContext {
        history: &empty_history,
        current_step: 0,
        parameters: &params,
    };

    // Multiplication before addition: 1 + 2 * 3 == 7
    let expr1 = parse_expression_str("1 + 2 * 3").unwrap();
    assert_eq!(eval_expr(&expr1, &ctx).unwrap(), 7.0);

    // Parentheses override: (1 + 2) * 3 == 9
    let expr2 = parse_expression_str("(1 + 2) * 3").unwrap();
    assert_eq!(eval_expr(&expr2, &ctx).unwrap(), 9.0);

    // Right-associative exponentiation: 2 ^ 3 ^ 2 == 2 ^ 9 == 512
    let expr3 = parse_expression_str("2 ^ 3 ^ 2").unwrap();
    assert_eq!(eval_expr(&expr3, &ctx).unwrap(), 512.0);

    // Multiple terms: 2 * 3 + 4 * 5 == 26
    let expr4 = parse_expression_str("2 * 3 + 4 * 5").unwrap();
    assert_eq!(eval_expr(&expr4, &ctx).unwrap(), 26.0);

    // Left associativity of subtraction: 10 - 4 - 2 == 4
    let expr5 = parse_expression_str("10 - 4 - 2").unwrap();
    assert_eq!(eval_expr(&expr5, &ctx).unwrap(), 4.0);

    // Relational and logical precedence: 1 < 2 and 3 + 1 == 4
    let expr6 = parse_expression_str("1 < 2 and 3 + 1 == 4").unwrap();
    assert_eq!(eval_expr(&expr6, &ctx).unwrap(), 1.0);
}

// 2. State reference parsing: a[n], a[n+1], foo[n-12]
#[test]
fn test_state_reference_parsing() {
    // a[n]
    let expr1 = parse_expression_str("a[n]").unwrap();
    match expr1.kind {
        ExprKind::StateRef { variable, offset } => {
            assert_eq!(variable, "a");
            assert_eq!(offset, 0);
        }
        _ => panic!("Expected StateRef"),
    }

    // a[n+1]
    let expr2 = parse_expression_str("a[n+1]").unwrap();
    match expr2.kind {
        ExprKind::StateRef { variable, offset } => {
            assert_eq!(variable, "a");
            assert_eq!(offset, 1);
        }
        _ => panic!("Expected StateRef"),
    }

    // foo[n-12]
    let expr3 = parse_expression_str("foo[n-12]").unwrap();
    match expr3.kind {
        ExprKind::StateRef { variable, offset } => {
            assert_eq!(variable, "foo");
            assert_eq!(offset, -12);
        }
        _ => panic!("Expected StateRef"),
    }

    // Equation parsing with state references
    let eq = parse_equation_str("a[n+1] = a[n] - 0.094 * b[n]").unwrap();
    assert_eq!(eq.target_var, "a");
    assert_eq!(eq.target_offset, 1);
}

// 3. The oscillator:
//    a[n+1] = a[n] - 0.094*b[n]
//    b[n+1] = b[n] + 0.094*a[n+1]
#[test]
fn test_victor_oscillator() {
    let eq1 = parse_equation_str("a[n+1] = a[n] - 0.094 * b[n]").unwrap();
    let eq2 = parse_equation_str("b[n+1] = b[n] + 0.094 * a[n+1]").unwrap();

    let mut inits = HashMap::new();
    inits.insert("a".to_string(), 1.0);
    inits.insert("b".to_string(), 0.0);

    let params = HashMap::new();
    let result = run_simulation(&[eq1, eq2], &inits, &params, 100).unwrap();

    assert!(result.failure.is_none());
    let a_vals = &result.series.get("a").unwrap().values;
    let b_vals = &result.series.get("b").unwrap().values;

    assert_eq!(a_vals.len(), 100);
    assert_eq!(b_vals.len(), 100);

    // Initial step
    assert_eq!(a_vals[0], 1.0);
    assert_eq!(b_vals[0], 0.0);

    // Step 1:
    // a[1] = a[0] - 0.094 * b[0] = 1.0 - 0.0 = 1.0
    // b[1] = b[0] + 0.094 * a[1] = 0.0 + 0.094 * 1.0 = 0.094
    assert_eq!(a_vals[1], 1.0);
    assert!((b_vals[1] - 0.094).abs() < 1e-9);

    // Step 2:
    // a[2] = 1.0 - 0.094 * 0.094 = 0.991164
    // b[2] = 0.094 + 0.094 * 0.991164 = 0.187169416
    assert!((a_vals[2] - 0.991164).abs() < 1e-6);

    // The symplectic Euler oscillator maintains bounded orbit
    for &v in a_vals {
        assert!(v.abs() < 1.5, "Oscillator exploded: {v}");
    }
    for &v in b_vals {
        assert!(v.abs() < 1.5, "Oscillator exploded: {v}");
    }
}

// 4. Same-step dependency ordering
#[test]
fn test_same_step_dependency_ordering() {
    // Put b first in array, but b[n+1] depends on a[n+1]!
    let eq_b = parse_equation_str("b[n+1] = b[n] + 0.094 * a[n+1]").unwrap();
    let eq_a = parse_equation_str("a[n+1] = a[n] - 0.094 * b[n]").unwrap();

    let info = analyze_dependencies(&[eq_b, eq_a]).unwrap();
    let pos_a = info.eval_order.iter().position(|x| x == "a").unwrap();
    let pos_b = info.eval_order.iter().position(|x| x == "b").unwrap();

    assert!(
        pos_a < pos_b,
        "a must be evaluated before b at timestep n+1"
    );
}

// 5. Same-step dependency cycle detection
#[test]
fn test_same_step_cycle_detection() {
    let eq1 = parse_equation_str("a[n+1] = b[n+1]").unwrap();
    let eq2 = parse_equation_str("b[n+1] = a[n+1]").unwrap();

    let err = analyze_dependencies(&[eq1, eq2]).unwrap_err();
    match err {
        DependencyError::Cycle(cycle) => {
            assert!(cycle.contains(&"a".to_string()));
            assert!(cycle.contains(&"b".to_string()));
        }
        _ => panic!("Expected cycle error, got {:?}", err),
    }
}

// 6. Delayed cycles being legal
#[test]
fn test_delayed_cycles_legal() {
    // Delayed cycle: a[n+1] depends on b[n], b[n+1] depends on a[n]
    let eq1 = parse_equation_str("a[n+1] = b[n]").unwrap();
    let eq2 = parse_equation_str("b[n+1] = a[n]").unwrap();

    let info = analyze_dependencies(&[eq1.clone(), eq2.clone()]).unwrap();
    assert_eq!(info.eval_order.len(), 2);

    let mut inits = HashMap::new();
    inits.insert("a".to_string(), 10.0);
    inits.insert("b".to_string(), 20.0);

    let res = run_simulation(&[eq1, eq2], &inits, &HashMap::new(), 4).unwrap();
    let a_vals = &res.series.get("a").unwrap().values;
    let b_vals = &res.series.get("b").unwrap().values;

    // Ping-ponging values
    assert_eq!(a_vals, &[10.0, 20.0, 10.0, 20.0]);
    assert_eq!(b_vals, &[20.0, 10.0, 20.0, 10.0]);
}

// 7. cond()
#[test]
fn test_cond_function() {
    let eq_a = parse_equation_str("a[n+1] = a[n] - 0.5").unwrap();
    let eq_c = parse_equation_str("c[n+1] = cond(a[n] > 0, 1, -1)").unwrap();

    let mut inits = HashMap::new();
    inits.insert("a".to_string(), 1.0);
    inits.insert("c".to_string(), 0.0);

    let res = run_simulation(&[eq_a, eq_c], &inits, &HashMap::new(), 5).unwrap();
    let a_vals = &res.series.get("a").unwrap().values;
    let c_vals = &res.series.get("c").unwrap().values;

    // a starts at 1.0, then 0.5, then 0.0, then -0.5, -1.0
    assert_eq!(a_vals[0], 1.0);
    assert_eq!(a_vals[1], 0.5);
    assert_eq!(a_vals[2], 0.0);
    assert_eq!(a_vals[3], -0.5);

    // c[n+1] = cond(a[n] > 0, 1, -1)
    // At n=0: a[0] = 1.0 > 0 -> c[1] = 1.0
    // At n=1: a[1] = 0.5 > 0 -> c[2] = 1.0
    // At n=2: a[2] = 0.0 <= 0 -> c[3] = -1.0
    // At n=3: a[3] = -0.5 <= 0 -> c[4] = -1.0
    assert_eq!(c_vals[1], 1.0);
    assert_eq!(c_vals[2], 1.0);
    assert_eq!(c_vals[3], -1.0);
    assert_eq!(c_vals[4], -1.0);
}

// 8. Search query: a[n+1] > a[n] produces expected mask
#[test]
fn test_behavioral_search() {
    let eq1 = parse_equation_str("a[n+1] = a[n] - 0.094 * b[n]").unwrap();
    let eq2 = parse_equation_str("b[n+1] = b[n] + 0.094 * a[n+1]").unwrap();

    let mut inits = HashMap::new();
    inits.insert("a".to_string(), 1.0);
    inits.insert("b".to_string(), 0.0);

    let res = run_simulation(&[eq1, eq2], &inits, &HashMap::new(), 20).unwrap();
    let a_vals = &res.series.get("a").unwrap().values;

    let query = parse_expression_str("a[n+1] > a[n]").unwrap();
    let search_res = execute_search(&query, &res, &HashMap::new());

    assert_eq!(search_res.mask.len(), 20);
    for n in 0..19 {
        let expected = a_vals[n + 1] > a_vals[n];
        assert_eq!(
            search_res.mask[n],
            expected,
            "Mismatch at step {n}: a[n]={val_n}, a[n+1]={val_next}",
            val_n = a_vals[n],
            val_next = a_vals[n + 1]
        );
    }
}

// 9. Selection measurements
#[test]
fn test_selection_measurements() {
    let eq1 = parse_equation_str("a[n+1] = a[n] + 2").unwrap();
    let mut inits = HashMap::new();
    inits.insert("a".to_string(), 0.0);

    let res = run_simulation(&[eq1], &inits, &HashMap::new(), 10).unwrap();
    // a values: [0, 2, 4, 6, 8, 10, 12, 14, 16, 18]

    // Select indices 1..=4: [2, 4, 6, 8]
    let mean_spec = MeasurementSpec::new("a", MeasurementType::Mean);
    assert_eq!(calculate_measurement(&mean_spec, &res, 1, 4).unwrap(), 5.0);

    let min_spec = MeasurementSpec::new("a", MeasurementType::Min);
    assert_eq!(calculate_measurement(&min_spec, &res, 1, 4).unwrap(), 2.0);

    let max_spec = MeasurementSpec::new("a", MeasurementType::Max);
    assert_eq!(calculate_measurement(&max_spec, &res, 1, 4).unwrap(), 8.0);

    let range_spec = MeasurementSpec::new("a", MeasurementType::Range);
    assert_eq!(calculate_measurement(&range_spec, &res, 1, 4).unwrap(), 6.0);

    let sum_spec = MeasurementSpec::new("a", MeasurementType::Sum);
    assert_eq!(calculate_measurement(&sum_spec, &res, 1, 4).unwrap(), 20.0);

    let delta_spec = MeasurementSpec::new("a", MeasurementType::Delta);
    assert_eq!(
        calculate_measurement(&delta_spec, &res, 1, 4).unwrap(),
        6.0 // 8 - 2
    );

    let duration_spec = MeasurementSpec::new("a", MeasurementType::Duration);
    assert_eq!(
        calculate_measurement(&duration_spec, &res, 1, 4).unwrap(),
        4.0 // 4 - 1 + 1
    );
}

// 10. Parameter sweep doesn't mutate the document's current parameter value
#[test]
fn test_parameter_sweep_does_not_mutate_document() {
    let mut doc_state = DocumentState::victor_oscillator_demo();
    doc_state.parameters = vec![ParameterDoc {
        name: "k".to_string(),
        value: 0.094,
        min: 0.01,
        max: 0.20,
        step: 0.001,
    }];

    let doc = Document::new(doc_state);
    let original_k = doc.state.parameters[0].value;
    assert_eq!(original_k, 0.094);

    let (eqs, _) = doc.state.get_parsed_equations();
    let params_map = doc.state.get_parameters_map();

    let sweep_config = SweepConfig {
        parameter_name: "k".to_string(),
        min_val: 0.05,
        max_val: 0.15,
        steps: 20,
        measurement: MeasurementSpec::new("a", MeasurementType::Mean),
    };

    let sweep_res = execute_parameter_sweep(
        &sweep_config,
        &eqs,
        &doc.state.initial_conditions,
        &params_map,
        doc.state.sample_count,
        50,
        150,
    );

    assert_eq!(sweep_res.points.len(), 20);
    // CRITICAL: verify document state is untouched
    assert_eq!(
        doc.state.parameters[0].value, original_k,
        "Parameter sweep mutated the document parameter!"
    );
}

// 11. Presets test: all preset systems parse and simulate cleanly
#[test]
fn test_all_presets() {
    use difference_equation_calc::presets::ALL_PRESETS;

    for preset in ALL_PRESETS {
        let state = (preset.state_builder)();
        let (eqs, parse_errors) = state.get_parsed_equations();
        assert!(
            parse_errors.is_empty(),
            "Preset '{}' had parse errors: {:?}",
            preset.name,
            parse_errors
        );

        let dep_analysis = analyze_dependencies(&eqs);
        assert!(
            dep_analysis.is_ok(),
            "Preset '{}' had dependency cycle error: {:?}",
            preset.name,
            dep_analysis.err()
        );
        let _eval_order = dep_analysis.unwrap();

        let params_map = state.get_parameters_map();
        let sim = run_simulation(
            &eqs,
            &state.initial_conditions,
            &params_map,
            state.sample_count,
        )
        .expect("Simulation setup failed");

        assert!(
            sim.failure.is_none(),
            "Preset '{}' failed during simulation: {:?}",
            preset.name,
            sim.failure
        );
        assert_eq!(sim.sample_count, state.sample_count);
    }
}

// 12. Inline term evaluations at cursor step n
#[test]
fn test_inline_term_evaluations_at_step() {
    use difference_equation_calc::eval::collect_equation_term_evaluations;
    use difference_equation_calc::sim::SimResultContext;

    let doc = Document::default();
    let (eqs, _) = doc.state.get_parsed_equations();
    let params = doc.state.get_parameters_map();
    let sim = run_simulation(
        &eqs,
        &doc.state.initial_conditions,
        &params,
        doc.state.sample_count,
    )
    .unwrap();

    let ctx = SimResultContext {
        sim_result: &sim,
        current_step: 10,
        parameters: &params,
    };

    // First equation: a[n+1] = a[n] - k * b[n]
    let (rhs_val, terms) = collect_equation_term_evaluations(&eqs[0].rhs, &ctx);
    assert!(rhs_val.is_some());

    let a_term = terms.iter().find(|t| t.label == "a[n]");
    assert!(a_term.is_some(), "Expected a[n] term in evaluation trace");
    assert_eq!(a_term.unwrap().value, sim.get_value("a", 10).unwrap());

    let k_term = terms.iter().find(|t| t.label == "k");
    assert!(k_term.is_some(), "Expected k parameter in evaluation trace");
    assert_eq!(k_term.unwrap().value, 0.094);

    let b_term = terms.iter().find(|t| t.label == "b[n]");
    assert!(b_term.is_some(), "Expected b[n] term in evaluation trace");
    assert_eq!(b_term.unwrap().value, sim.get_value("b", 10).unwrap());

    let expected_rhs = a_term.unwrap().value - k_term.unwrap().value * b_term.unwrap().value;
    assert!((rhs_val.unwrap() - expected_rhs).abs() < 1e-9);
}

// 13. Attractor bifurcation sweep
#[test]
fn test_attractor_bifurcation_sweep() {
    use difference_equation_calc::presets::{get_preset_by_kind, PresetKind};
    use difference_equation_calc::sweep::{execute_attractor_sweep, AttractorSweepConfig};

    let preset = get_preset_by_kind(PresetKind::LogisticMap);
    let state = (preset.state_builder)();
    let (eqs, _) = state.get_parsed_equations();
    let params = state.get_parameters_map();

    let config = AttractorSweepConfig {
        parameter_name: "r".to_string(),
        variable_name: "x".to_string(),
        min_val: 2.8,
        max_val: 3.9,
        slices: 50,
        total_steps: 200,
        tail_steps: 50,
    };
    let sweep = execute_attractor_sweep(&config, &eqs, &state.initial_conditions, &params);

    assert_eq!(sweep.parameter_name, "r");
    assert_eq!(sweep.variable_name, "x");
    assert!(
        !sweep.points.is_empty(),
        "Attractor sweep generated no points"
    );
    assert!(
        sweep.state_min >= 0.0,
        "Logistic map values should stay non-negative"
    );
    assert!(
        sweep.state_max <= 1.0,
        "Logistic map values should stay <= 1.0"
    );
}

// 14. Biquad filter coefficient calculation & frequency response
#[test]
fn test_biquad_coefficients_and_frequency_response() {
    use difference_equation_calc::filter::{BiquadCoeffs, FilterType};
    use std::f64::consts::PI;

    // Test Lowpass
    let lp = BiquadCoeffs::from_filter_params(FilterType::Lowpass, 0.10, 2.0);
    let (dc_gain_db, _) = lp.eval_frequency_response(0.0001);
    let (nyquist_gain_db, _) = lp.eval_frequency_response(PI);
    assert!(
        (dc_gain_db - 0.0).abs() < 0.2,
        "Lowpass DC gain should be close to 0 dB, got {}",
        dc_gain_db
    );
    assert!(
        nyquist_gain_db < -20.0,
        "Lowpass gain at Nyquist should be strongly attenuated, got {}",
        nyquist_gain_db
    );

    // Test Highpass
    let hp = BiquadCoeffs::from_filter_params(FilterType::Highpass, 0.10, 2.0);
    let (hp_dc_db, _) = hp.eval_frequency_response(0.0001);
    let (hp_nyquist_db, _) = hp.eval_frequency_response(PI);
    assert!(
        hp_dc_db < -20.0,
        "Highpass DC gain should be strongly attenuated, got {}",
        hp_dc_db
    );
    assert!(
        (hp_nyquist_db - 0.0).abs() < 0.2,
        "Highpass Nyquist gain should be close to 0 dB, got {}",
        hp_nyquist_db
    );

    // Test Notch Filter: deep null at center frequency
    let fc = 0.15;
    let notch = BiquadCoeffs::from_filter_params(FilterType::Notch, fc, 8.0);
    let (notch_null_db, _) = notch.eval_frequency_response(2.0 * PI * fc);
    assert!(
        notch_null_db < -35.0,
        "Notch should have deep null at center frequency, got {} dB",
        notch_null_db
    );

    // Test Poles stability
    let (p1, p2) = lp.poles();
    assert!(
        p1.mag() < 1.0 && p2.mag() < 1.0,
        "Biquad poles must lie strictly inside the unit circle for stability"
    );
}

// 15. Chamberlin SVF Preset Simulation
#[test]
fn test_chamberlin_svf_preset_simulation() {
    use difference_equation_calc::presets::{get_preset_by_kind, PresetKind};
    use difference_equation_calc::sim::run_simulation;

    let preset = get_preset_by_kind(PresetKind::ChamberlinSvf);
    let state = (preset.state_builder)();
    let (eqs, errors) = state.get_parsed_equations();
    assert!(
        errors.is_empty(),
        "Chamberlin SVF equations failed to parse: {:?}",
        errors
    );

    let params = state.get_parameters_map();
    let sim = run_simulation(&eqs, &state.initial_conditions, &params, state.sample_count)
        .expect("Simulation of Chamberlin SVF failed");

    assert!(sim.failure.is_none());
    assert!(sim.series.contains_key("lp"));
    assert!(sim.series.contains_key("hp"));
    assert!(sim.series.contains_key("bp"));

    let lp_vals = &sim.series.get("lp").unwrap().values;
    assert_eq!(lp_vals.len(), state.sample_count);
    for v in lp_vals {
        assert!(v.is_finite(), "Simulation produced non-finite values");
    }
}

// 16. Resonant Biquad Preset Simulation
#[test]
fn test_resonant_biquad_preset_simulation() {
    use difference_equation_calc::presets::{get_preset_by_kind, PresetKind};
    use difference_equation_calc::sim::run_simulation;

    let preset = get_preset_by_kind(PresetKind::ResonantBiquad);
    let state = (preset.state_builder)();
    let (eqs, errors) = state.get_parsed_equations();
    assert!(errors.is_empty(), "Resonant biquad equations parse error");

    let params = state.get_parameters_map();
    let sim = run_simulation(&eqs, &state.initial_conditions, &params, state.sample_count)
        .expect("Simulation of Resonant Biquad failed");

    assert!(sim.failure.is_none());
    assert!(sim.series.contains_key("x"));
    assert!(sim.series.contains_key("y"));

    let y_vals = &sim.series.get("y").unwrap().values;
    assert_eq!(y_vals.len(), state.sample_count);
    for v in y_vals {
        assert!(v.is_finite());
    }
}

// 17. Notch Filter Preset Simulation
#[test]
fn test_notch_filter_preset_simulation() {
    use difference_equation_calc::presets::{get_preset_by_kind, PresetKind};
    use difference_equation_calc::sim::run_simulation;

    let preset = get_preset_by_kind(PresetKind::NotchFilter);
    let state = (preset.state_builder)();
    let (eqs, errors) = state.get_parsed_equations();
    assert!(errors.is_empty(), "Notch filter equations parse error");

    let params = state.get_parameters_map();
    let sim = run_simulation(&eqs, &state.initial_conditions, &params, state.sample_count)
        .expect("Simulation of Notch Filter failed");

    assert!(sim.failure.is_none());
    assert!(sim.series.contains_key("y"));
}

// 18. Higher pole count cascade filter frequency response & poles
#[test]
fn test_cascade_filter_higher_poles() {
    use difference_equation_calc::filter::{CascadeFilter, FilterType};
    use std::f64::consts::PI;

    for pole_count in [2, 4, 6, 8] {
        let cascade = CascadeFilter::new(FilterType::Lowpass, pole_count, 0.08, 0.707, false);
        assert_eq!(cascade.pole_count, pole_count);
        assert_eq!(cascade.stages.len(), pole_count / 2);

        let poles = cascade.all_poles();
        assert_eq!(poles.len(), pole_count / 2);
        assert!(
            cascade.max_pole_mag() < 1.0,
            "Stable cascade poles must lie inside unit circle"
        );

        // DC gain should be close to 0 dB
        let (dc_db, _) = cascade.eval_frequency_response(0.0001);
        assert!(
            (dc_db - 0.0).abs() < 0.5,
            "DC gain should be ~0 dB, got {}",
            dc_db
        );
    }

    // Compare roll-off slope at 2 octaves above cutoff (f = 0.32 vs fc = 0.08)
    let c2 = CascadeFilter::new(FilterType::Lowpass, 2, 0.08, 0.707, false);
    let c4 = CascadeFilter::new(FilterType::Lowpass, 4, 0.08, 0.707, false);
    let c8 = CascadeFilter::new(FilterType::Lowpass, 8, 0.08, 0.707, false);

    let (gain2, _) = c2.eval_frequency_response(2.0 * PI * 0.32);
    let (gain4, _) = c4.eval_frequency_response(2.0 * PI * 0.32);
    let (gain8, _) = c8.eval_frequency_response(2.0 * PI * 0.32);

    // 8-pole must attenuate significantly steeper than 4-pole, which attenuates steeper than 2-pole
    assert!(
        gain4 < gain2 - 15.0,
        "4-pole should attenuate at least 15 dB more than 2-pole at 4*fc: {} vs {}",
        gain4,
        gain2
    );
    assert!(
        gain8 < gain4 - 20.0,
        "8-pole should attenuate at least 20 dB more than 4-pole at 4*fc: {} vs {}",
        gain8,
        gain4
    );
}

// 19. Self-oscillation pole mapping and coefficient synthesis
#[test]
fn test_self_oscillation_mode() {
    use difference_equation_calc::filter::{pole_to_fc_q_ext, BiquadCoeffs, FilterType};

    // When allow_self_osc is false, max radius is strictly capped < 1.0
    let (_fc, q) = pole_to_fc_q_ext(0.8, 0.6, false); // mag = 1.0
    assert!(q <= 25.0);

    // When allow_self_osc is true, radius can reach into unstable region (r >= 1.0)
    let (fc_osc, q_osc) = pole_to_fc_q_ext(0.85, 0.65, true); // mag = 1.07
    assert!(
        q_osc > 25.0,
        "Expected Q in self-oscillation region (> 25.0), got {}",
        q_osc
    );
    assert!((fc_osc - 0.10).abs() < 0.05);

    // Biquad synthesis with allow_self_osc produces pole magnitude >= 1.0
    let coeffs = BiquadCoeffs::from_filter_params_ext(FilterType::Lowpass, 0.10, 40.0, true);
    let (p1, p2) = coeffs.poles();
    assert!(
        p1.mag() >= 1.0,
        "Self-oscillating biquad pole magnitude must be >= 1.0, got {}",
        p1.mag()
    );
    assert!(p2.mag() >= 1.0);
    assert!(
        coeffs.a2 >= 1.0,
        "Feedback coefficient a2 must be >= 1.0 for self-oscillation, got {}",
        coeffs.a2
    );
}

// 20. Cascade biquad document generation and simulation with soft saturation
#[test]
fn test_cascade_biquad_document_simulation() {
    use difference_equation_calc::filter::{create_cascade_biquad_doc, FilterType, StimulusType};
    use difference_equation_calc::sim::run_simulation;

    // 4-pole cascade with self-oscillation and soft saturation (tanh)
    let doc = create_cascade_biquad_doc(
        FilterType::Lowpass,
        4,
        0.05,
        40.0,
        StimulusType::Impulse,
        true,
        true,
    );

    let (eqs, errors) = doc.get_parsed_equations();
    assert!(
        errors.is_empty(),
        "4-pole cascade biquad equations parse error: {:?}",
        errors
    );

    let params = doc.get_parameters_map();
    let sim = run_simulation(&eqs, &doc.initial_conditions, &params, 100)
        .expect("Simulation of 4-pole cascade biquad failed");

    assert!(
        sim.failure.is_none(),
        "Cascade biquad simulation failure: {:?}",
        sim.failure
    );
    assert!(sim.series.contains_key("x"));
    assert!(sim.series.contains_key("y1"));
    assert!(sim.series.contains_key("y"));

    let y_vals = &sim.series.get("y").unwrap().values;
    assert_eq!(y_vals.len(), 100);
    for v in y_vals {
        assert!(v.is_finite());
        // Because of tanh saturation, values should stay comfortably bounded
        assert!(
            v.abs() <= 5.0,
            "Soft saturation should bound output, got {}",
            v
        );
    }
}

// 21. Cascade SVF document generation and simulation
#[test]
fn test_cascade_svf_document_simulation() {
    use difference_equation_calc::filter::{create_cascade_svf_doc, StimulusType};
    use difference_equation_calc::sim::run_simulation;

    // 4-pole SVF with self-oscillation and soft saturation
    let doc = create_cascade_svf_doc(4, 0.08, 35.0, StimulusType::Step, true, true);

    let (eqs, errors) = doc.get_parsed_equations();
    assert!(
        errors.is_empty(),
        "4-pole SVF equations parse error: {:?}",
        errors
    );

    let params = doc.get_parameters_map();
    let sim = run_simulation(&eqs, &doc.initial_conditions, &params, 100)
        .expect("Simulation of 4-pole SVF failed");

    assert!(
        sim.failure.is_none(),
        "Cascade SVF simulation failure: {:?}",
        sim.failure
    );
    assert!(sim.series.contains_key("lp1"));
    assert!(sim.series.contains_key("lp"));

    let lp_vals = &sim.series.get("lp").unwrap().values;
    assert_eq!(lp_vals.len(), 100);
    for v in lp_vals {
        assert!(v.is_finite());
    }
}
