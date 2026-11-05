//! The `coupling` analyzer: files ranked by the dependency edges they have
//! with the other files of the scope.

use super::deps::{DepGraph, Outcome, supports};
use super::envelope::{AnalyzeError, Finding, Location, decimal};
use super::read::FileSource;
use super::run::{Analysis, Analyzer, RunContext};
use act_lite_parser::Grammar;
use serde::Serialize;
use serde_json::{Map, Number, Value, json};
use std::cmp::Ordering;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

/// Counts, for each analyzed file, the files it depends on and the analyzed
/// files that depend on it.
pub struct CouplingAnalyzer;

/// One analyzed file with at least one edge.
#[derive(Debug, Clone, Serialize)]
pub struct CouplingFinding {
    #[serde(flatten)]
    pub location: Location,
    pub fan_in: u32,
    pub fan_out: u32,
    pub degree: u32,
    pub instability: Number,
    pub external: u32,
    pub unresolved: u32,
}

impl Finding for CouplingFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    fn order(&self, other: &Self) -> Ordering {
        other
            .degree
            .cmp(&self.degree)
            .then_with(|| other.fan_in.cmp(&self.fan_in))
            .then_with(|| self.location.cmp(&other.location))
    }
}

impl Analyzer for CouplingAnalyzer {
    const NAME: &'static str = "coupling";
    const HINT: &'static str = "Start with the first file: its edges are the files it imports and the files that import it; analyze_impact lists what depends on a file.";
    type Finding = CouplingFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        supports(grammar)
    }

    fn analyze(
        &self,
        _files: &mut dyn FileSource,
        _cancel: &CancellationToken,
    ) -> Result<Analysis<CouplingFinding>, AnalyzeError> {
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
    ) -> Result<Analysis<CouplingFinding>, AnalyzeError> {
        let graph = DepGraph::build(files, ctx, cancel)?;
        let mut fan_in: HashMap<&str, u32> = HashMap::new();
        let mut fan_out: HashMap<&str, u32> = HashMap::new();
        for edge in graph.edges() {
            *fan_out.entry(&edge.file).or_default() += 1;
            *fan_in.entry(&edge.to).or_default() += 1;
        }
        let mut findings = Vec::new();
        for file in graph.analyzed() {
            let fan_in = fan_in.get(file.as_str()).copied().unwrap_or(0);
            let fan_out = fan_out.get(file.as_str()).copied().unwrap_or(0);
            let degree = fan_in + fan_out;
            if degree == 0 {
                continue;
            }
            let outcomes = graph.relationships_of(file);
            let count = |wanted: fn(&Outcome) -> bool| {
                outcomes.iter().filter(|r| wanted(&r.outcome)).count() as u32
            };
            findings.push(CouplingFinding {
                location: Location {
                    file: file.clone(),
                    range: None,
                },
                fan_in,
                fan_out,
                degree,
                instability: decimal(f64::from(fan_out) / f64::from(degree))?,
                external: count(|o| matches!(o, Outcome::External)),
                unresolved: count(|o| matches!(o, Outcome::Unresolved(_))),
            });
        }
        if cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        findings.sort_by(|a, b| a.order(b));
        Ok(Analysis {
            findings,
            unresolved: graph.unresolved(&|_| true),
            limits_hit: Vec::new(),
            details: Some(details(&graph)),
        })
    }
}

/// The common dependency details, the formulas and the edge count.
fn details(graph: &DepGraph) -> Map<String, Value> {
    let mut details = graph.common_details();
    details.insert(
        "formulas".to_string(),
        json!({
            "fan_in": "distinct analyzed files with an edge to the file",
            "fan_out": "distinct workspace files the file has an edge to",
            "degree": "fan_in + fan_out",
            "instability": "fan_out / (fan_in + fan_out)",
        }),
    );
    details.insert("edges".to_string(), json!(graph.edges().len()));
    details
}
