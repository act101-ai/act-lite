//! A test analyzer: one finding per Rust function of 5 or more lines.

use super::envelope::{AnalyzeError, Finding, Location};
use super::read::FileSource;
use super::run::{Analysis, Analyzer};
use act_lite_core::CompactRange;
use act_lite_parser::Grammar;
use serde::Serialize;
use std::cmp::Ordering;
use tokio_util::sync::CancellationToken;
use tree_sitter::Node;

/// The smallest function, in lines, that the fixture analyzer reports.
const MIN_LINES: usize = 5;

/// Reports each Rust `function_item` that spans [`MIN_LINES`] or more lines.
pub struct FixtureAnalyzer;

/// A reported function: its file and the range of its node.
#[derive(Debug, Clone, Serialize)]
pub struct FixtureFinding {
    #[serde(flatten)]
    pub location: Location,
}

impl Finding for FixtureFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    fn order(&self, other: &Self) -> Ordering {
        self.location.cmp(&other.location)
    }
}

impl Analyzer for FixtureAnalyzer {
    const NAME: &'static str = "fixture";
    const HINT: &'static str = "Narrow the result with paths, include or exclude, or change limit.";
    type Finding = FixtureFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        grammar == Grammar::Rust
    }

    fn analyze(
        &self,
        files: &mut dyn FileSource,
        cancel: &CancellationToken,
    ) -> Result<Analysis<FixtureFinding>, AnalyzeError> {
        let mut findings = Vec::new();
        while let Some(file) = files.next_file()? {
            if cancel.is_cancelled() {
                return Err(AnalyzeError::Cancelled);
            }
            long_functions(file.source.tree().root_node(), &file.rel, &mut findings);
        }
        Ok(Analysis {
            findings,
            unresolved: Vec::new(),
            limits_hit: Vec::new(),
            details: None,
            failed_inputs: None,
        })
    }
}

/// Adds each `function_item` below `root` of [`MIN_LINES`] or more lines.
fn long_functions(root: Node, rel: &str, out: &mut Vec<FixtureFinding>) {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        let (start, end) = (node.start_position(), node.end_position());
        if node.kind() == "function_item" && end.row + 1 - start.row >= MIN_LINES {
            out.push(FixtureFinding {
                location: Location {
                    file: rel.to_string(),
                    range: Some(CompactRange::new(
                        start.row as u32 + 1,
                        start.column as u32 + 1,
                        end.row as u32 + 1,
                        end.column as u32 + 1,
                    )),
                },
            });
        }
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}
