use crate::document::{ClampingMode, DocumentState, EquationDoc, ParameterDoc};
use crate::measure::{MeasurementSpec, MeasurementType};
use crate::sweep::SweepConfig;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetKind {
    VictorOscillator,
    VanDerPol,
    LogisticMap,
    HenonMap,
    LotkaVolterra,
    PidController,
    ChamberlinSvf,
    ResonantBiquad,
    NotchFilter,
}

pub struct PresetSystem {
    pub kind: PresetKind,
    pub name: &'static str,
    pub description: &'static str,
    pub state_builder: fn() -> DocumentState,
    pub default_phase_pair: (&'static str, &'static str),
}

pub const ALL_PRESETS: &[PresetSystem] = &[
    PresetSystem {
        kind: PresetKind::VictorOscillator,
        name: "Harmonic Oscillator (Bret Victor Demo)",
        description:
            "Coupled linear oscillator with same-step dependency, comparator, and integrator.",
        state_builder: DocumentState::victor_oscillator_demo,
        default_phase_pair: ("a", "b"),
    },
    PresetSystem {
        kind: PresetKind::VanDerPol,
        name: "Van der Pol Oscillator (Limit Cycle)",
        description:
            "Non-linear damping creating an isolated closed trajectory (limit cycle attractor).",
        state_builder: van_der_pol_preset,
        default_phase_pair: ("x", "y"),
    },
    PresetSystem {
        kind: PresetKind::LogisticMap,
        name: "Logistic Map (Period-Doubling & Chaos)",
        description: "Quadratic recurrence showing period-doubling cascade and chaotic orbits.",
        state_builder: logistic_map_preset,
        default_phase_pair: ("x", "x"),
    },
    PresetSystem {
        kind: PresetKind::HenonMap,
        name: "Hénon Strange Attractor",
        description: "2D discrete chaotic dynamical system showing a fractal strange attractor.",
        state_builder: henon_map_preset,
        default_phase_pair: ("x", "y"),
    },
    PresetSystem {
        kind: PresetKind::LotkaVolterra,
        name: "Lotka-Volterra (Predator-Prey)",
        description: "Non-linear ecological coupling producing closed periodic population orbits.",
        state_builder: lotka_volterra_preset,
        default_phase_pair: ("prey", "pred"),
    },
    PresetSystem {
        kind: PresetKind::PidController,
        name: "PID Feedback Control Loop",
        description: "Proportional-integral-derivative closed-loop target tracking and damping.",
        state_builder: pid_controller_preset,
        default_phase_pair: ("pos", "vel"),
    },
    PresetSystem {
        kind: PresetKind::ChamberlinSvf,
        name: "Chamberlin State Variable Filter (SVF)",
        description:
            "Simultaneous Lowpass, Bandpass, and Highpass outputs with live resonance damping.",
        state_builder: chamberlin_svf_preset,
        default_phase_pair: ("lp", "bp"),
    },
    PresetSystem {
        kind: PresetKind::ResonantBiquad,
        name: "Resonant Biquad Lowpass (Direct Form II)",
        description:
            "2nd-order audio biquad showing step response overshoot, ringing, and cutoffs.",
        state_builder: resonant_biquad_preset,
        default_phase_pair: ("x", "y"),
    },
    PresetSystem {
        kind: PresetKind::NotchFilter,
        name: "Notch Filter (Interference Rejection)",
        description: "Zero-pair on the unit circle eliminating interfering frequency components.",
        state_builder: notch_filter_preset,
        default_phase_pair: ("x", "y"),
    },
];

pub fn get_preset_by_kind(kind: PresetKind) -> &'static PresetSystem {
    ALL_PRESETS
        .iter()
        .find(|p| p.kind == kind)
        .unwrap_or(&ALL_PRESETS[0])
}

fn van_der_pol_preset() -> DocumentState {
    let equations = vec![
        EquationDoc {
            id: 1,
            source: "x[n+1] = x[n] + dt * y[n]".to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "y[n+1] = y[n] + dt * (mu * (1.0 - x[n] * x[n]) * y[n] - x[n])".to_string(),
            color_idx: 1,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.1);
    initial_conditions.insert("y".to_string(), 0.1);

    let parameters = vec![
        ParameterDoc {
            name: "mu".to_string(),
            value: 1.2,
            min: 0.0,
            max: 3.5,
            step: 0.02,
        },
        ParameterDoc {
            name: "dt".to_string(),
            value: 0.06,
            min: 0.01,
            max: 0.12,
            step: 0.002,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 700,
        selection: Some((150, 450)),
        search_query: "y[n+1] > y[n]".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "mu".to_string(),
            measurement: MeasurementSpec::new("x", MeasurementType::Rms),
            min_val: 0.0,
            max_val: 3.5,
            steps: 120,
        },
        clamping_mode: ClampingMode::None,
    }
}

fn logistic_map_preset() -> DocumentState {
    let equations = vec![EquationDoc {
        id: 1,
        source: "x[n+1] = r * x[n] * (1.0 - x[n])".to_string(),
        color_idx: 0,
    }];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.4);

    let parameters = vec![ParameterDoc {
        name: "r".to_string(),
        value: 3.65,
        min: 2.5,
        max: 4.0,
        step: 0.005,
    }];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 500,
        selection: Some((200, 450)),
        search_query: "x[n] > 0.7".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "r".to_string(),
            measurement: MeasurementSpec::new("x", MeasurementType::Mean),
            min_val: 2.5,
            max_val: 4.0,
            steps: 200,
        },
        clamping_mode: ClampingMode::None,
    }
}

fn henon_map_preset() -> DocumentState {
    let equations = vec![
        EquationDoc {
            id: 1,
            source: "x[n+1] = 1.0 - a * x[n] * x[n] + y[n]".to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "y[n+1] = b * x[n]".to_string(),
            color_idx: 1,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("x".to_string(), 0.1);
    initial_conditions.insert("y".to_string(), 0.1);

    let parameters = vec![
        ParameterDoc {
            name: "a".to_string(),
            value: 1.4,
            min: 0.8,
            max: 1.5,
            step: 0.005,
        },
        ParameterDoc {
            name: "b".to_string(),
            value: 0.3,
            min: 0.1,
            max: 0.4,
            step: 0.005,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 1000,
        selection: Some((100, 800)),
        search_query: "x[n] > 0".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "a".to_string(),
            measurement: MeasurementSpec::new("x", MeasurementType::Range),
            min_val: 0.8,
            max_val: 1.5,
            steps: 140,
        },
        clamping_mode: ClampingMode::None,
    }
}

fn lotka_volterra_preset() -> DocumentState {
    let equations = vec![
        EquationDoc {
            id: 1,
            source: "prey[n+1] = prey[n] + dt * (alpha * prey[n] - beta * prey[n] * pred[n])"
                .to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "pred[n+1] = pred[n] + dt * (delta * prey[n] * pred[n] - gamma * pred[n])"
                .to_string(),
            color_idx: 1,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("prey".to_string(), 1.2);
    initial_conditions.insert("pred".to_string(), 0.8);

    let parameters = vec![
        ParameterDoc {
            name: "alpha".to_string(),
            value: 1.0,
            min: 0.2,
            max: 2.0,
            step: 0.02,
        },
        ParameterDoc {
            name: "beta".to_string(),
            value: 0.8,
            min: 0.2,
            max: 2.0,
            step: 0.02,
        },
        ParameterDoc {
            name: "delta".to_string(),
            value: 0.6,
            min: 0.1,
            max: 1.5,
            step: 0.02,
        },
        ParameterDoc {
            name: "gamma".to_string(),
            value: 0.7,
            min: 0.1,
            max: 1.5,
            step: 0.02,
        },
        ParameterDoc {
            name: "dt".to_string(),
            value: 0.04,
            min: 0.01,
            max: 0.08,
            step: 0.002,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 800,
        selection: Some((100, 400)),
        search_query: "prey[n+1] > prey[n]".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "alpha".to_string(),
            measurement: MeasurementSpec::new("prey", MeasurementType::Mean),
            min_val: 0.2,
            max_val: 2.0,
            steps: 100,
        },
        clamping_mode: ClampingMode::None,
    }
}

fn pid_controller_preset() -> DocumentState {
    let equations = vec![
        EquationDoc {
            id: 1,
            source: "err[n+1] = setpoint - pos[n]".to_string(),
            color_idx: 0,
        },
        EquationDoc {
            id: 2,
            source: "integral[n+1] = integral[n] + err[n+1] * dt".to_string(),
            color_idx: 1,
        },
        EquationDoc {
            id: 3,
            source: "deriv[n+1] = (err[n+1] - err[n]) / dt".to_string(),
            color_idx: 2,
        },
        EquationDoc {
            id: 4,
            source: "control[n+1] = kp * err[n+1] + ki * integral[n+1] + kd * deriv[n+1]"
                .to_string(),
            color_idx: 3,
        },
        EquationDoc {
            id: 5,
            source: "vel[n+1] = vel[n] * 0.95 + dt * control[n+1]".to_string(),
            color_idx: 4,
        },
        EquationDoc {
            id: 6,
            source: "pos[n+1] = pos[n] + dt * vel[n+1]".to_string(),
            color_idx: 5,
        },
    ];

    let mut initial_conditions = HashMap::new();
    initial_conditions.insert("err".to_string(), 1.0);
    initial_conditions.insert("integral".to_string(), 0.0);
    initial_conditions.insert("deriv".to_string(), 0.0);
    initial_conditions.insert("control".to_string(), 0.0);
    initial_conditions.insert("vel".to_string(), 0.0);
    initial_conditions.insert("pos".to_string(), 0.0);

    let parameters = vec![
        ParameterDoc {
            name: "setpoint".to_string(),
            value: 1.0,
            min: 0.0,
            max: 2.0,
            step: 0.05,
        },
        ParameterDoc {
            name: "kp".to_string(),
            value: 1.6,
            min: 0.1,
            max: 4.0,
            step: 0.05,
        },
        ParameterDoc {
            name: "ki".to_string(),
            value: 0.25,
            min: 0.0,
            max: 1.5,
            step: 0.02,
        },
        ParameterDoc {
            name: "kd".to_string(),
            value: 0.4,
            min: 0.0,
            max: 1.5,
            step: 0.02,
        },
        ParameterDoc {
            name: "dt".to_string(),
            value: 0.05,
            min: 0.01,
            max: 0.1,
            step: 0.002,
        },
    ];

    DocumentState {
        equations,
        initial_conditions,
        parameters,
        sample_count: 500,
        selection: Some((0, 150)),
        search_query: "pos[n] > 0.99 and pos[n] < 1.01".to_string(),
        sweep_config: SweepConfig {
            parameter_name: "kp".to_string(),
            measurement: MeasurementSpec::new("pos", MeasurementType::Max),
            min_val: 0.1,
            max_val: 4.0,
            steps: 120,
        },
        clamping_mode: ClampingMode::None,
    }
}

fn chamberlin_svf_preset() -> DocumentState {
    crate::filter::create_chamberlin_svf_doc(0.08, 4.0, crate::filter::StimulusType::Step)
}

fn resonant_biquad_preset() -> DocumentState {
    crate::filter::create_biquad_filter_doc(
        crate::filter::FilterType::Lowpass,
        0.10,
        2.5,
        crate::filter::StimulusType::Step,
    )
}

fn notch_filter_preset() -> DocumentState {
    crate::filter::create_biquad_filter_doc(
        crate::filter::FilterType::Notch,
        0.15,
        6.0,
        crate::filter::StimulusType::SineAndNoise,
    )
}
