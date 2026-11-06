//! The `cycles` analyzer: dependency cycles between files.

use super::deps::{DepGraph, Edge, Step, supports};
use super::envelope::{AnalyzeError, Finding, Location, MAX_EVIDENCE_ENTRIES};
use super::read::FileSource;
use super::run::{Analysis, Analyzer, RunContext};
use act_lite_parser::Grammar;
use act_lite_parser::deps::RelationshipKind;
use serde::Serialize;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeSet, VecDeque};
use tokio_util::sync::CancellationToken;

/// The most members or steps a finding lists.
const MAX_LISTED: usize = 20;

/// Finds the sets of analyzed files in which each file reaches every other
/// one, with one shortest cycle through the first file of each set.
pub struct CyclesAnalyzer;

/// One set of mutually reachable files.
#[derive(Debug, Clone, Serialize)]
pub struct CycleFinding {
    #[serde(flatten)]
    pub location: Location,
    /// In byte order of path; at most 20.
    pub members: Vec<Location>,
    pub members_total: u64,
    pub edges_total: u64,
    /// A shortest cycle from the first member back to it; at most 20 steps.
    pub path: Vec<Step>,
    pub path_total: u64,
}

impl Finding for CycleFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    fn order(&self, other: &Self) -> Ordering {
        other
            .members_total
            .cmp(&self.members_total)
            .then_with(|| self.location.cmp(&other.location))
    }
}

impl Analyzer for CyclesAnalyzer {
    const NAME: &'static str = "cycles";
    const HINT: &'static str = "Start with the first cycle: its path is one way around, and its members are every file that takes part; break the cycle by removing one step.";
    type Finding = CycleFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        supports(grammar)
    }

    fn analyze(
        &self,
        _files: &mut dyn FileSource,
        _cancel: &CancellationToken,
    ) -> Result<Analysis<CycleFinding>, AnalyzeError> {
        Err(AnalyzeError::Failed {
            analysis: Self::NAME,
            cause: "the analysis needs the run context".to_string(),
        })
    }

    fn analyze_in(
        &self,
        files: &mut dyn FileSource,
        ctx: &RunContext,
        cancel: &CancellationToken,
    ) -> Result<Analysis<CycleFinding>, AnalyzeError> {
        let graph = DepGraph::build(files, ctx, cancel)?;
        let nodes: Vec<&String> = graph.analyzed().iter().collect();
        let edges: Vec<&Edge> = graph
            .edges()
            .iter()
            .filter(|e| {
                e.to_analyzed
                    && !e
                        .kinds
                        .iter()
                        .all(|kind| matches!(kind, RelationshipKind::Module))
            })
            .collect();
        let index = |file: &str| nodes.binary_search_by(|n| n.as_bytes().cmp(file.as_bytes()));
        let mut out: Vec<Vec<(usize, usize)>> = vec![Vec::new(); nodes.len()];
        for (i, edge) in edges.iter().enumerate() {
            if let (Ok(from), Ok(to)) = (index(&edge.file), index(&edge.to)) {
                out[from].push((to, i));
            }
        }
        let components = components(&out, cancel)?;
        let mut findings = Vec::new();
        let mut cyclic_files = 0;
        let mut component_of = vec![usize::MAX; nodes.len()];
        for (c, members) in components.iter().enumerate() {
            for &m in members {
                component_of[m] = c;
            }
        }
        for (c, members) in components.iter().enumerate() {
            cyclic_files += members.len();
            let subject = members[0];
            let path = witness(&out, &edges, members, cancel)?;
            let edges_total = members
                .iter()
                .flat_map(|&m| &out[m])
                .filter(|(to, _)| component_of[*to] == c)
                .count() as u64;
            findings.push(CycleFinding {
                location: Location {
                    file: nodes[subject].clone(),
                    range: None,
                },
                members: members
                    .iter()
                    .take(MAX_LISTED)
                    .map(|&m| Location {
                        file: nodes[m].clone(),
                        range: None,
                    })
                    .collect(),
                members_total: members.len() as u64,
                edges_total,
                path_total: path.len() as u64,
                path: path.into_iter().take(MAX_LISTED).collect(),
            });
        }
        if cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        findings.sort_by(|a, b| a.order(b));
        let not_followed: BTreeSet<&str> = graph
            .edges()
            .iter()
            .filter(|e| !e.to_analyzed)
            .map(|e| e.to.as_str())
            .collect();
        let mut details = graph.common_details();
        details.insert("cyclic_files".to_string(), json!(cyclic_files));
        details.insert(
            "not_followed".to_string(),
            Value::Array(
                not_followed
                    .iter()
                    .take(MAX_EVIDENCE_ENTRIES)
                    .map(|file| json!({ "file": file }))
                    .collect(),
            ),
        );
        details.insert("not_followed_total".to_string(), json!(not_followed.len()));
        Ok(Analysis {
            findings,
            unresolved: graph.unresolved(&|u| u.kind != RelationshipKind::Module),
            limits_hit: graph.limits_hit().to_vec(),
            details: Some(details),
            input_failed: graph.input_failed(),
        })
    }
}

/// The strongly connected components of two or more nodes, each in node
/// order, by Tarjan's algorithm with an explicit stack.
fn components(
    out: &[Vec<(usize, usize)>],
    cancel: &CancellationToken,
) -> Result<Vec<Vec<usize>>, AnalyzeError> {
    let n = out.len();
    let mut order = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut found = Vec::new();
    for root in 0..n {
        if order[root] != usize::MAX {
            continue;
        }
        let mut work = vec![(root, 0usize)];
        while let Some(&(node, child)) = work.last() {
            if child == 0 {
                if cancel.is_cancelled() {
                    return Err(AnalyzeError::Cancelled);
                }
                order[node] = next;
                low[node] = next;
                next += 1;
                stack.push(node);
                on_stack[node] = true;
            }
            if let Some(&(to, _)) = out[node].get(child) {
                work.last_mut().unwrap().1 += 1;
                if order[to] == usize::MAX {
                    work.push((to, 0));
                } else if on_stack[to] {
                    low[node] = low[node].min(order[to]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == order[node] {
                let mut members = Vec::new();
                while let Some(top) = stack.pop() {
                    on_stack[top] = false;
                    members.push(top);
                    if top == node {
                        break;
                    }
                }
                if members.len() >= 2 {
                    members.sort_unstable();
                    found.push(members);
                }
            }
        }
    }
    Ok(found)
}

/// The steps of the shortest cycle through the first of `members`, one
/// component in ascending node order. `out[n]` lists node `n`'s targets in
/// byte order, each with the index of its edge in `edges`. A breadth-first
/// search inside the component stops at the first edge back to that file; its
/// buffers are sized to the component, not to the graph.
pub fn witness(
    out: &[Vec<(usize, usize)>],
    edges: &[&Edge],
    members: &[usize],
    cancel: &CancellationToken,
) -> Result<Vec<Step>, AnalyzeError> {
    let local = |node: usize| members.binary_search(&node).ok();
    let mut reached_by: Vec<Option<(usize, usize)>> = vec![None; members.len()];
    let mut seen = vec![false; members.len()];
    seen[0] = true;
    let mut queue = VecDeque::from([0usize]);
    while let Some(at_local) = queue.pop_front() {
        if cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        for &(to, edge) in &out[members[at_local]] {
            let Some(to_local) = local(to) else {
                continue;
            };
            if to_local == 0 {
                let mut steps = vec![edges[edge].step()];
                let mut at = at_local;
                while let Some((from, via)) = reached_by[at] {
                    steps.push(edges[via].step());
                    at = from;
                }
                steps.reverse();
                return Ok(steps);
            }
            if !seen[to_local] {
                seen[to_local] = true;
                reached_by[to_local] = Some((at_local, edge));
                queue.push_back(to_local);
            }
        }
    }
    Err(AnalyzeError::Failed {
        analysis: CyclesAnalyzer::NAME,
        cause: "a component has no cycle through its first file".to_string(),
    })
}
