use crate::ast::Expr;
use crate::eval::{eval_expr, SlicedContext};
use crate::sim::SimulationResult;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub mask: Vec<bool>,
    pub regions: Vec<Range<usize>>,
    pub total_matches: usize,
}

impl SearchResult {
    pub fn empty(sample_count: usize) -> Self {
        Self {
            mask: vec![false; sample_count],
            regions: Vec::new(),
            total_matches: 0,
        }
    }
}

pub fn execute_search(
    query_expr: &Expr,
    sim_result: &SimulationResult,
    parameters: &HashMap<String, f64>,
) -> SearchResult {
    let sample_count = sim_result.sample_count;
    if sample_count == 0 {
        return SearchResult::default();
    }

    // Prepare history map for SlicedContext
    let mut history_map: HashMap<String, Vec<f64>> = HashMap::new();
    for (name, s) in &sim_result.series {
        history_map.insert(name.clone(), s.values.clone());
    }

    let mut mask = vec![false; sample_count];
    let mut total_matches = 0;

    for (n, slot) in mask.iter_mut().enumerate() {
        let ctx = SlicedContext {
            history: &history_map,
            current_step: n,
            parameters,
        };

        if let Ok(val) = eval_expr(query_expr, &ctx) {
            let is_match = val != 0.0;
            *slot = is_match;
            if is_match {
                total_matches += 1;
            }
        } else {
            *slot = false;
        }
    }

    // Extract contiguous runs of true samples as regions
    let mut regions = Vec::new();
    let mut in_region = false;
    let mut region_start = 0;

    for (i, &is_true) in mask.iter().enumerate() {
        if is_true {
            if !in_region {
                in_region = true;
                region_start = i;
            }
        } else if in_region {
            in_region = false;
            regions.push(region_start..i);
        }
    }

    if in_region {
        regions.push(region_start..sample_count);
    }

    SearchResult {
        mask,
        regions,
        total_matches,
    }
}
