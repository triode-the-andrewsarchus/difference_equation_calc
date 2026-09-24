use crate::ast::Equation;
use crate::parser::{parse_equation_str, ParseError};
use crate::sweep::SweepConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquationDoc {
    pub id: u64,
    pub source: String,
    pub color_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterDoc {
    pub name: String,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentState {
    pub equations: Vec<EquationDoc>,
    pub initial_conditions: HashMap<String, f64>,
    pub parameters: Vec<ParameterDoc>,
    pub sample_count: usize,
    pub selection: Option<(usize, usize)>,
    pub search_query: String,
    pub sweep_config: SweepConfig,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::victor_oscillator_demo()
    }
}

impl DocumentState {
    pub fn victor_oscillator_demo() -> Self {
        let equations = vec![
            EquationDoc {
                id: 1,
                source: "a[n+1] = a[n] - k * b[n]".to_string(),
                color_idx: 0,
            },
            EquationDoc {
                id: 2,
                source: "b[n+1] = b[n] + k * a[n+1]".to_string(),
                color_idx: 1,
            },
            EquationDoc {
                id: 3,
                source: "c[n+1] = cond(a[n] > 0, 1, -1)".to_string(),
                color_idx: 2,
            },
            EquationDoc {
                id: 4,
                source: "d[n+1] = d[n] + 0.08 * c[n]".to_string(),
                color_idx: 3,
            },
        ];

        let mut initial_conditions = HashMap::new();
        initial_conditions.insert("a".to_string(), 1.0);
        initial_conditions.insert("b".to_string(), 0.0);
        initial_conditions.insert("c".to_string(), 0.0);
        initial_conditions.insert("d".to_string(), 0.0);

        let parameters = vec![ParameterDoc {
            name: "k".to_string(),
            value: 0.094,
            min: 0.0,
            max: 0.25,
            step: 0.001,
        }];

        Self {
            equations,
            initial_conditions,
            parameters,
            sample_count: 600,
            selection: Some((50, 150)),
            search_query: "a[n+1] > a[n]".to_string(),
            sweep_config: SweepConfig::default(),
        }
    }

    pub fn get_parsed_equations(&self) -> (Vec<Equation>, HashMap<u64, ParseError>) {
        let mut parsed = Vec::new();
        let mut errors = HashMap::new();

        for eq_doc in &self.equations {
            let trimmed = eq_doc.source.trim();
            if trimmed.is_empty() {
                continue;
            }
            match parse_equation_str(trimmed) {
                Ok(eq) => parsed.push(eq),
                Err(err) => {
                    errors.insert(eq_doc.id, err);
                }
            }
        }

        (parsed, errors)
    }

    pub fn get_parameters_map(&self) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for p in &self.parameters {
            map.insert(p.name.clone(), p.value);
        }
        map
    }

    pub fn set_parameter_value(&mut self, name: &str, val: f64) -> bool {
        if let Some(p) = self.parameters.iter_mut().find(|p| p.name == name) {
            p.value = val.clamp(p.min, p.max);
            true
        } else {
            false
        }
    }
}

pub struct Document {
    pub state: DocumentState,
    history: Vec<DocumentState>,
    history_idx: usize,
    next_eq_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(DocumentState::default())
    }
}

impl Document {
    pub fn new(state: DocumentState) -> Self {
        let next_eq_id = state.equations.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        Self {
            history: vec![state.clone()],
            history_idx: 0,
            state,
            next_eq_id,
        }
    }

    pub fn checkpoint(&mut self) {
        // If current state matches latest history entry, don't duplicate
        if self.history.get(self.history_idx) == Some(&self.state) {
            return;
        }

        self.history.truncate(self.history_idx + 1);
        self.history.push(self.state.clone());
        // Cap history to 50 entries
        if self.history.len() > 50 {
            self.history.remove(0);
        } else {
            self.history_idx += 1;
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history_idx > 0
    }

    pub fn can_redo(&self) -> bool {
        self.history_idx + 1 < self.history.len()
    }

    pub fn undo(&mut self) -> bool {
        if self.can_undo() {
            self.history_idx -= 1;
            self.state = self.history[self.history_idx].clone();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if self.can_redo() {
            self.history_idx += 1;
            self.state = self.history[self.history_idx].clone();
            true
        } else {
            false
        }
    }

    pub fn next_equation_id(&mut self) -> u64 {
        let id = self.next_eq_id;
        self.next_eq_id += 1;
        id
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.state)
    }

    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let state: DocumentState = serde_json::from_str(json_str)?;
        Ok(Self::new(state))
    }
}
