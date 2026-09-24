use crate::ast::{Equation, Expr, ExprKind};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub enum DependencyError {
    DuplicateTarget(String),
    Cycle(Vec<String>),
    FutureReference {
        variable: String,
        target_var: String,
        offset: i64,
        target_offset: i64,
    },
    SelfDependency(String),
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyError::DuplicateTarget(v) => {
                write!(f, "Variable '{v}' is defined by multiple equations")
            }
            DependencyError::Cycle(cycle) => {
                write!(
                    f,
                    "Instantaneous dependency cycle detected: {}",
                    cycle.join(" -> ")
                )
            }
            DependencyError::FutureReference {
                variable,
                target_var,
                offset,
                target_offset,
            } => {
                write!(
                    f,
                    "Equation for '{target_var}[n+{target_offset}]' references future state '{variable}[n+{offset}]'"
                )
            }
            DependencyError::SelfDependency(v) => {
                write!(
                    f,
                    "Variable '{v}' instantaneously depends on itself at the same timestep"
                )
            }
        }
    }
}

impl std::error::Error for DependencyError {}

#[derive(Debug, Clone)]
pub struct DependencyInfo {
    /// Variables in topological evaluation order for each timestep
    pub eval_order: Vec<String>,
    /// Instantaneous dependencies at step n+1: var -> set of vars it depends on at same step
    pub same_step_deps: HashMap<String, HashSet<String>>,
    /// Delayed dependencies: var -> set of (var, offset) it references from past
    pub delayed_deps: HashMap<String, HashSet<(String, i64)>>,
    /// Named parameters referenced in expressions
    pub referenced_parameters: HashSet<String>,
}

pub fn analyze_dependencies(equations: &[Equation]) -> Result<DependencyInfo, DependencyError> {
    let mut targets = HashSet::new();
    let mut target_offsets = HashMap::new();

    for eq in equations {
        if !targets.insert(eq.target_var.clone()) {
            return Err(DependencyError::DuplicateTarget(eq.target_var.clone()));
        }
        target_offsets.insert(eq.target_var.clone(), eq.target_offset);
    }

    let mut same_step_deps: HashMap<String, HashSet<String>> = HashMap::new();
    let mut delayed_deps: HashMap<String, HashSet<(String, i64)>> = HashMap::new();
    let mut referenced_parameters = HashSet::new();

    for eq in equations {
        let mut same_set = HashSet::new();
        let mut delayed_set = HashSet::new();

        collect_expr_references(
            &eq.rhs,
            &eq.target_var,
            eq.target_offset,
            &mut same_set,
            &mut delayed_set,
            &mut referenced_parameters,
        )?;

        same_step_deps.insert(eq.target_var.clone(), same_set);
        delayed_deps.insert(eq.target_var.clone(), delayed_set);
    }

    // Topological sort using Kahn's algorithm + DFS cycle extraction
    let eval_order = topological_sort(&targets, &same_step_deps)?;

    Ok(DependencyInfo {
        eval_order,
        same_step_deps,
        delayed_deps,
        referenced_parameters,
    })
}

fn collect_expr_references(
    expr: &Expr,
    target_var: &str,
    target_offset: i64,
    same_step: &mut HashSet<String>,
    delayed: &mut HashSet<(String, i64)>,
    params: &mut HashSet<String>,
) -> Result<(), DependencyError> {
    match &expr.kind {
        ExprKind::Literal(_) => {}
        ExprKind::Parameter(name) => {
            params.insert(name.clone());
        }
        ExprKind::StateRef { variable, offset } => {
            if *offset == target_offset {
                if variable == target_var {
                    return Err(DependencyError::SelfDependency(variable.clone()));
                }
                same_step.insert(variable.clone());
            } else if *offset < target_offset {
                delayed.insert((variable.clone(), *offset));
            } else {
                return Err(DependencyError::FutureReference {
                    variable: variable.clone(),
                    target_var: target_var.to_string(),
                    offset: *offset,
                    target_offset,
                });
            }
        }
        ExprKind::Unary { expr, .. } => {
            collect_expr_references(expr, target_var, target_offset, same_step, delayed, params)?;
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr_references(left, target_var, target_offset, same_step, delayed, params)?;
            collect_expr_references(right, target_var, target_offset, same_step, delayed, params)?;
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                collect_expr_references(
                    arg,
                    target_var,
                    target_offset,
                    same_step,
                    delayed,
                    params,
                )?;
            }
        }
        ExprKind::Cond {
            condition,
            true_val,
            false_val,
        } => {
            collect_expr_references(
                condition,
                target_var,
                target_offset,
                same_step,
                delayed,
                params,
            )?;
            collect_expr_references(
                true_val,
                target_var,
                target_offset,
                same_step,
                delayed,
                params,
            )?;
            collect_expr_references(
                false_val,
                target_var,
                target_offset,
                same_step,
                delayed,
                params,
            )?;
        }
    }
    Ok(())
}

fn topological_sort(
    nodes: &HashSet<String>,
    deps: &HashMap<String, HashSet<String>>,
) -> Result<Vec<String>, DependencyError> {
    // In-degree: how many other nodes MUST be computed BEFORE node v.
    // If v depends on u at same step, u -> v. So in_degree of v increases.
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for node in nodes {
        in_degree.insert(node.clone(), 0);
        dependents.insert(node.clone(), Vec::new());
    }

    for (node, node_deps) in deps {
        for dep in node_deps {
            if nodes.contains(dep) {
                // dep -> node
                *in_degree.entry(node.clone()).or_insert(0) += 1;
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(node.clone());
            }
        }
    }

    // Queue nodes with in_degree == 0
    let mut queue = VecDeque::new();
    // Deterministic ordering: sort nodes first
    let mut sorted_nodes: Vec<_> = nodes.iter().cloned().collect();
    sorted_nodes.sort();

    for node in sorted_nodes {
        if in_degree.get(&node).copied().unwrap_or(0) == 0 {
            queue.push_back(node);
        }
    }

    let mut result = Vec::new();

    while let Some(u) = queue.pop_front() {
        result.push(u.clone());

        if let Some(succs) = dependents.get(&u) {
            for v in succs {
                let deg = in_degree.get_mut(v).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(v.clone());
                }
            }
        }
    }

    if result.len() != nodes.len() {
        // Cycle detected: find a cycle path for the error message
        let cycle = find_cycle(nodes, deps);
        return Err(DependencyError::Cycle(cycle));
    }

    Ok(result)
}

fn find_cycle(nodes: &HashSet<String>, deps: &HashMap<String, HashSet<String>>) -> Vec<String> {
    let mut visited = HashMap::new(); // 0 = unvisited, 1 = visiting, 2 = visited
    let mut path = Vec::new();

    for node in nodes {
        if dfs_cycle(node, deps, &mut visited, &mut path) {
            return path;
        }
    }

    nodes.iter().cloned().collect()
}

fn dfs_cycle(
    u: &str,
    deps: &HashMap<String, HashSet<String>>,
    visited: &mut HashMap<String, u8>,
    path: &mut Vec<String>,
) -> bool {
    visited.insert(u.to_string(), 1);
    path.push(u.to_string());

    if let Some(outgoing) = deps.get(u) {
        for v in outgoing {
            match visited.get(v).copied().unwrap_or(0) {
                1 => {
                    // Found cycle!
                    path.push(v.clone());
                    return true;
                }
                0 if dfs_cycle(v, deps, visited, path) => return true,
                _ => {}
            }
        }
    }

    path.pop();
    visited.insert(u.to_string(), 2);
    false
}
