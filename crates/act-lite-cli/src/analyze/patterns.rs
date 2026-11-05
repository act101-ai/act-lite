//! The `patterns` analyzer: functions whose metrics exceed a fixed threshold.

use super::envelope::{AnalyzeError, Finding, LanguageCoverage, Location};
use super::read::FileSource;
use super::run::{Analysis, Analyzer};
use act_lite_parser::Grammar;
use act_lite_parser::function_model::{self, Metrics};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::cmp::Ordering;
use tokio_util::sync::CancellationToken;

/// The rules in byte order of their names: name, metric and threshold. A
/// rule is exceeded when the metric is strictly greater than the threshold.
const RULES: [(&str, &str, u32); 4] = [
    ("deep_nesting", "nesting", 4),
    ("excessive_branching", "complexity", 10),
    ("long_parameter_list", "parameters", 5),
    ("oversized_function", "lines", 50),
];

/// The most entries of `details.not_applicable`.
const MAX_NOT_APPLICABLE: usize = 30;

/// Reports each function of a supported language once per rule it exceeds.
pub struct PatternsAnalyzer;

/// One function over one threshold.
#[derive(Debug, Clone, Serialize)]
pub struct PatternFinding {
    #[serde(flatten)]
    pub location: Location,
    pub rule: &'static str,
    pub function: Option<String>,
    pub metric: &'static str,
    pub value: u32,
    pub threshold: u32,
    pub metrics: MetricValues,
}

/// The four metrics of the function, `parameters` null where it does not apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MetricValues {
    pub lines: u32,
    pub nesting: u32,
    pub complexity: u32,
    pub parameters: Option<u32>,
}

impl MetricValues {
    fn get(&self, metric: &str) -> Option<u32> {
        match metric {
            "lines" => Some(self.lines),
            "nesting" => Some(self.nesting),
            "complexity" => Some(self.complexity),
            _ => self.parameters,
        }
    }
}

impl From<Metrics> for MetricValues {
    fn from(m: Metrics) -> Self {
        MetricValues {
            lines: m.lines,
            nesting: m.nesting,
            complexity: m.complexity,
            parameters: m.parameters,
        }
    }
}

impl Finding for PatternFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    fn order(&self, other: &Self) -> Ordering {
        self.location
            .cmp(&other.location)
            .then_with(|| self.rule.as_bytes().cmp(other.rule.as_bytes()))
    }
}

impl Analyzer for PatternsAnalyzer {
    const NAME: &'static str = "patterns";
    const HINT: &'static str = "Narrow the result with paths, include or exclude, or change limit.";
    type Finding = PatternFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        function_model::supports(grammar)
    }

    fn analyze(
        &self,
        files: &mut dyn FileSource,
        cancel: &CancellationToken,
    ) -> Result<Analysis<PatternFinding>, AnalyzeError> {
        let mut findings = Vec::new();
        let mut counts = [0u64; RULES.len()];
        let mut measured = 0u64;
        let checkpoint = || {
            if cancel.is_cancelled() {
                Err(AnalyzeError::Cancelled)
            } else {
                Ok(())
            }
        };
        loop {
            checkpoint()?;
            let Some(file) = files.next_file()? else {
                break;
            };
            let mut functions = function_model::functions(&file.source);
            loop {
                checkpoint()?;
                let Some(function) = functions.next() else {
                    break;
                };
                measured += 1;
                let metrics = MetricValues::from(function.metrics);
                for (i, (rule, metric, threshold)) in RULES.into_iter().enumerate() {
                    let Some(value) = metrics.get(metric).filter(|&v| v > threshold) else {
                        continue;
                    };
                    counts[i] += 1;
                    findings.push(PatternFinding {
                        location: Location {
                            file: file.rel.clone(),
                            range: Some(function.range.clone()),
                        },
                        rule,
                        function: function.name.clone(),
                        metric,
                        value,
                        threshold,
                        metrics,
                    });
                }
            }
        }
        checkpoint()?;
        let mut details = Map::new();
        details.insert(
            "thresholds".to_string(),
            json!({
                "oversized_function": 50,
                "deep_nesting": 4,
                "excessive_branching": 10,
                "long_parameter_list": 5
            }),
        );
        let counts_by_rule: Map<String, Value> = RULES
            .iter()
            .zip(counts)
            .map(|((rule, _, _), n)| (rule.to_string(), n.into()))
            .collect();
        details.insert("counts_by_rule".to_string(), counts_by_rule.into());
        details.insert("functions".to_string(), measured.into());
        Ok(Analysis {
            findings,
            unresolved: Vec::new(),
            limits_hit: Vec::new(),
            details: Some(details),
        })
    }

    /// Lists each supported language in scope that has a rule that does not
    /// apply to it: Bash, whose functions declare no parameters. Each list
    /// has its `<name>_total`, as every list in `details` does.
    fn finish_details(&self, details: &mut Map<String, Value>, languages: &[LanguageCoverage]) {
        let listed: Vec<Value> = languages
            .iter()
            .filter(|l| l.supported && l.language == Grammar::Bash.name())
            .map(|l| {
                json!({"language": l.language, "rules": ["long_parameter_list"], "rules_total": 1})
            })
            .collect();
        let total = listed.len();
        let listed: Vec<Value> = listed.into_iter().take(MAX_NOT_APPLICABLE).collect();
        details.insert("not_applicable".to_string(), listed.into());
        details.insert("not_applicable_total".to_string(), total.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::read::{FileSource, Parsed};
    use act_lite_parser::{Grammar, SourceFile};

    /// Gives one Rust file and cancels the token as it does.
    struct CancelOnPull {
        cancel: CancellationToken,
        pulls: usize,
    }

    impl FileSource for CancelOnPull {
        fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
            self.pulls += 1;
            self.cancel.cancel();
            let source =
                SourceFile::from_content("a.rs", "fn a() {}\n".to_string(), Grammar::Rust).unwrap();
            Ok(Some(Parsed {
                rel: "a.rs".to_string(),
                source,
            }))
        }

        fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
            panic!("unexpected reread of {rel}")
        }
    }

    #[test]
    fn cancel_while_a_file_is_pulled_stops_before_its_functions() {
        let cancel = CancellationToken::new();
        let mut files = CancelOnPull {
            cancel: cancel.clone(),
            pulls: 0,
        };
        let result = PatternsAnalyzer.analyze(&mut files, &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
        assert_eq!(files.pulls, 1);
    }
}
