//! The `hotspots` analyzer: files and functions ranked by complexity and size.

use super::envelope::{AnalyzeError, Finding, Location};
use super::read::FileSource;
use super::run::{Analysis, Analyzer};
use act_lite_core::CompactRange;
use act_lite_parser::{Grammar, function_model};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::cmp::Ordering;
use tokio_util::sync::CancellationToken;

/// The most top functions one file item lists.
const TOP_FUNCTIONS: usize = 5;

/// Ranks each analyzed file that has a function by the sum, over its
/// functions, of complexity × lines.
pub struct HotspotsAnalyzer;

/// One file: its score, the sums the score uses, and its top functions.
#[derive(Debug, Clone, Serialize)]
pub struct HotspotFinding {
    #[serde(flatten)]
    pub location: Location,
    pub score: u64,
    pub functions: u32,
    pub complexity: u64,
    pub lines: u64,
    pub top_functions: Vec<TopFunction>,
    pub top_functions_total: u32,
}

/// One function of a file item, with its score and raw metrics.
#[derive(Debug, Clone, Serialize)]
pub struct TopFunction {
    pub function: Option<String>,
    pub range: CompactRange,
    pub score: u64,
    pub complexity: u32,
    pub lines: u32,
    pub nesting: u32,
    pub parameters: Option<u32>,
}

impl Finding for HotspotFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    fn order(&self, other: &Self) -> Ordering {
        other
            .score
            .cmp(&self.score)
            .then_with(|| self.location.cmp(&other.location))
    }
}

impl Analyzer for HotspotsAnalyzer {
    const NAME: &'static str = "hotspots";
    const HINT: &'static str = "Start with the first file's top functions; analyze_patterns reports the thresholds they exceed.";
    type Finding = HotspotFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        function_model::supports(grammar)
    }

    fn reports_regions(&self) -> bool {
        true
    }

    fn analyze(
        &self,
        files: &mut dyn FileSource,
        cancel: &CancellationToken,
    ) -> Result<Analysis<HotspotFinding>, AnalyzeError> {
        let mut findings = Vec::new();
        let mut measured: u64 = 0;
        while let Some(file) = files.next_file()? {
            let mut functions = Vec::new();
            let code = file.code();
            let mut iter = code.iter().flat_map(|unit| {
                function_model::functions_in(unit.grammar, unit.tree, unit.source)
            });
            loop {
                if cancel.is_cancelled() {
                    return Err(AnalyzeError::Cancelled);
                }
                let Some(function) = iter.next() else { break };
                let m = function.metrics;
                functions.push(TopFunction {
                    function: function.name,
                    range: function.range,
                    score: u64::from(m.complexity) * u64::from(m.lines),
                    complexity: m.complexity,
                    lines: m.lines,
                    nesting: m.nesting,
                    parameters: m.parameters,
                });
            }
            if let Some(finding) = file_item(file.rel, functions) {
                measured += u64::from(finding.functions);
                findings.push(finding);
            }
        }
        if cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        findings.sort_by(|a, b| a.order(b));
        Ok(Analysis {
            findings,
            unresolved: Vec::new(),
            limits_hit: Vec::new(),
            details: Some(details(measured)),
            failed_inputs: None,
        })
    }
}

/// The item of one file, or `None` when the file has no function.
fn file_item(rel: String, mut functions: Vec<TopFunction>) -> Option<HotspotFinding> {
    if functions.is_empty() {
        return None;
    }
    let total = functions.len() as u32;
    let score = functions.iter().map(|f| f.score).sum();
    let complexity = functions.iter().map(|f| u64::from(f.complexity)).sum();
    let lines = functions.iter().map(|f| u64::from(f.lines)).sum();
    functions.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| range_key(&a.range).cmp(&range_key(&b.range)))
    });
    functions.truncate(TOP_FUNCTIONS);
    Some(HotspotFinding {
        location: Location {
            file: rel,
            range: None,
        },
        score,
        functions: total,
        complexity,
        lines,
        top_functions: functions,
        top_functions_total: total,
    })
}

/// Location order within one file: start line, start column, end line, end column.
fn range_key(range: &CompactRange) -> (u32, u32, u32, u32) {
    (
        range.start_line(),
        range.start_col(),
        range.end_line(),
        range.end_col(),
    )
}

/// The ranking formula and the definitions of its inputs.
fn details(functions: u64) -> Map<String, Value> {
    let Value::Object(map) = json!({
        "formula": "score(file) = sum over functions f in file of complexity(f) * lines(f)",
        "complexity": "cyclomatic: 1 + decision points in the function's own body",
        "lines": "lines with a non-comment token of the function's own body, nested functions and Python docstrings excluded",
        "dependency_information": false,
        "functions": functions,
    }) else {
        unreachable!("a JSON object literal")
    };
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::read::Parsed;
    use act_lite_parser::SourceFile;

    /// Hands out two Rust files and cancels the token when the first is pulled.
    struct CancellingSource {
        cancel: CancellationToken,
        pulled: usize,
    }

    impl FileSource for CancellingSource {
        fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
            self.pulled += 1;
            self.cancel.cancel();
            if self.pulled > 2 {
                return Ok(None);
            }
            let source = SourceFile::from_content(
                "src/a.rs",
                "fn f() {}\nfn g() {}\n".to_string(),
                Grammar::Rust,
            )
            .unwrap();
            Ok(Some(Parsed {
                rel: "src/a.rs".to_string(),
                source,
                removed: Vec::new(),
            }))
        }

        fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
            unreachable!("hotspots reads {rel} once")
        }
    }

    #[test]
    fn a_cancelled_token_stops_before_the_next_function() {
        let cancel = CancellationToken::new();
        let mut files = CancellingSource {
            cancel: cancel.clone(),
            pulled: 0,
        };
        let result = HotspotsAnalyzer.analyze(&mut files, &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
        assert_eq!(files.pulled, 1);
    }
}
