//! Cross-file prop-drilling *depth*. `prop-drilling` (in `react`) flags a single
//! forwarding hop but can't tell a 1-level pass from a 10-level drill. This module
//! stitches the per-file forwarding edges into a graph and measures how far a prop is
//! handed down.
//!
//! An [`Edge`] means: component `from_component` forwards its own prop `from_param`
//! into child `to_component` under the prop name `to_param`, unchanged. Nodes are
//! `(component, prop)` pairs; a path through the graph is a drill chain, and its
//! length is the depth. The graph is name-based (attribute name ↔ child param name),
//! resolving child components by name across the whole tree.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Edge {
    pub from_component: String,
    pub from_param: String,
    pub to_component: String,
    pub to_param: String,
    pub file: String,
    pub line: usize,
}

type Node = (String, String);

/// Maximal drill chains (as edge-index sequences), longest first. A chain of `k`
/// edges means the value was forwarded `k` times, through `k + 1` components.
pub fn chains(edges: &[Edge]) -> Vec<Vec<usize>> {
    let mut out: HashMap<Node, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        out.entry((e.from_component.clone(), e.from_param.clone()))
            .or_default()
            .push(i);
    }
    let targets: HashSet<Node> = edges
        .iter()
        .map(|e| (e.to_component.clone(), e.to_param.clone()))
        .collect();

    // Roots = nodes that forward but are never forwarded *into* (they receive their
    // value from an origin, so they start a maximal chain).
    let roots: Vec<Node> = out
        .keys()
        .filter(|n| !targets.contains(*n))
        .cloned()
        .collect();

    let mut memo: HashMap<Node, Vec<usize>> = HashMap::new();
    let mut chains: Vec<Vec<usize>> = roots
        .iter()
        .map(|r| longest(r, &out, edges, &mut memo, &mut HashSet::new()))
        .filter(|c| !c.is_empty())
        .collect();
    chains.sort_by_key(|c| Reverse(c.len()));
    chains
}

/// Longest chain of edge indices starting at `node`, memoized, cycle-guarded.
fn longest(
    node: &Node,
    out: &HashMap<Node, Vec<usize>>,
    edges: &[Edge],
    memo: &mut HashMap<Node, Vec<usize>>,
    stack: &mut HashSet<Node>,
) -> Vec<usize> {
    if let Some(hit) = memo.get(node) {
        return hit.clone();
    }
    if !stack.insert(node.clone()) {
        return Vec::new(); // cycle
    }
    let mut best: Vec<usize> = Vec::new();
    for &ei in out.get(node).map(Vec::as_slice).unwrap_or(&[]) {
        let child = (edges[ei].to_component.clone(), edges[ei].to_param.clone());
        let mut chain = vec![ei];
        chain.extend(longest(&child, out, edges, memo, stack));
        if chain.len() > best.len() {
            best = chain;
        }
    }
    stack.remove(node);
    memo.insert(node.clone(), best.clone());
    best
}
