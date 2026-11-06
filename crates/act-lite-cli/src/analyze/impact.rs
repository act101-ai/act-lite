//! The `impact` analysis: the direct and transitive dependents of focus files.
//!
//! A dependent is an analyzed file that imports a focus file, directly or
//! through other files. The analysis is `graph`'s `in` traversal from the
//! focus files; each file keeps the edge through which it was first reached,
//! and those edges give its chain back to a focus file.

use super::deps::{self, DepGraph, Family, Step, UnresolvedRel};
use super::envelope::{AnalyzeError, Finding, LimitHit, Location};
use super::focus::{FocusSet, check_focus, focus_files};
use super::graph::{Direction, MAX_MAX_DEPTH, Traversal, insert_focus, traverse};
use super::read::{FileSource, ReadOutcome};
use super::run::{Analysis, Analyzer, Request, RunContext};
use act_lite_parser::Grammar;
use act_lite_parser::deps::RelationshipKind;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde_json::json;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// The most steps of a chain.
const MAX_CHAIN: usize = 20;

/// A request error in a parameter other than a path or the limit.
fn request_error(message: String) -> AnalyzeError {
    AnalyzeError::InvalidRequest(message)
}

/// The failure of a focus state lock that a panic poisoned.
fn poisoned() -> AnalyzeError {
    AnalyzeError::Failed {
        analysis: ImpactAnalyzer::NAME,
        cause: "focus state poisoned".to_string(),
    }
}

/// Reports the files that depend on the focus files.
pub struct ImpactAnalyzer {
    /// The checked focus paths, inside the workspace root.
    focus: Vec<PathBuf>,
    max_depth: u32,
    /// The focus set found by `analyze_in`, completed by `after_scope`.
    focus_set: Mutex<Option<FocusSet>>,
}

impl ImpactAnalyzer {
    /// Checks, in order: the `max_depth` range, that a focus path is given,
    /// then each focus path.
    pub fn new(
        request: &Request,
        focus: &[String],
        max_depth: Option<u32>,
    ) -> Result<ImpactAnalyzer, AnalyzeError> {
        if let Some(n) = max_depth
            && !(1..=MAX_MAX_DEPTH).contains(&n)
        {
            return Err(request_error(format!(
                "max_depth must be between 1 and {MAX_MAX_DEPTH}, got {n}"
            )));
        }
        if focus.is_empty() {
            return Err(request_error(
                "impact needs at least one focus path".to_string(),
            ));
        }
        let focus = check_focus(&request.root, &request.scope, focus)?;
        Ok(ImpactAnalyzer {
            focus,
            max_depth: max_depth.unwrap_or(MAX_MAX_DEPTH),
            focus_set: Mutex::new(None),
        })
    }
}

/// One dependent: its distance to the nearest focus file and the steps that
/// lead there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactFinding {
    pub file: String,
    pub depth: u32,
    /// The focus file at the end of the chain.
    pub reaches: String,
    /// The first steps of the chain, at most 20.
    pub chain: Vec<Step>,
    location: Location,
}

impl Serialize for ImpactFinding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("ImpactFinding", 5)?;
        s.serialize_field("file", &self.file)?;
        s.serialize_field("depth", &self.depth)?;
        s.serialize_field("reaches", &self.reaches)?;
        s.serialize_field("chain", &self.chain)?;
        s.serialize_field("chain_total", &self.depth)?;
        s.end()
    }
}

impl Finding for ImpactFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    /// `depth` ascending, then `file`.
    fn order(&self, other: &Self) -> Ordering {
        self.depth
            .cmp(&other.depth)
            .then_with(|| self.location.cmp(&other.location))
    }
}

/// The dependents a traversal reached: every file at distance 1 or more.
/// The token is checked before each dependent's chain.
fn dependents(
    traversal: &Traversal,
    cancel: &CancellationToken,
) -> Result<Vec<ImpactFinding>, AnalyzeError> {
    traversal
        .distance
        .iter()
        .filter(|(_, depth)| **depth >= 1)
        .map(|(file, depth)| {
            if cancel.is_cancelled() {
                return Err(AnalyzeError::Cancelled);
            }
            let mut chain = Vec::new();
            let mut at = file;
            while let Some(edge) = traversal.first_edge.get(at) {
                if chain.len() < MAX_CHAIN {
                    chain.push(edge.step());
                }
                at = &edge.to;
            }
            Ok(ImpactFinding {
                file: file.clone(),
                depth: *depth,
                reaches: at.clone(),
                chain,
                location: Location {
                    file: file.clone(),
                    range: None,
                },
            })
        })
        .collect()
}

impl Analyzer for ImpactAnalyzer {
    const NAME: &'static str = "impact";
    const HINT: &'static str = "Narrow the result with paths, include, exclude or focus, or change limit; max_depth sets how far the traversal goes.";
    type Finding = ImpactFinding;

    fn narrowing(&self) -> &'static [&'static str] {
        &["focus", "max_depth"]
    }

    fn supports(&self, grammar: Grammar) -> bool {
        deps::supports(grammar)
    }

    fn analyze(
        &self,
        _files: &mut dyn FileSource,
        _cancel: &CancellationToken,
    ) -> Result<Analysis<ImpactFinding>, AnalyzeError> {
        Err(AnalyzeError::Failed {
            analysis: Self::NAME,
            cause: "the impact analysis needs the run context".to_string(),
        })
    }

    fn analyze_in(
        &self,
        files: &mut dyn FileSource,
        ctx: &RunContext,
        cancel: &CancellationToken,
    ) -> Result<Analysis<ImpactFinding>, AnalyzeError> {
        let graph = DepGraph::build(files, ctx, cancel)?;
        let set = focus_files(
            &ctx.root,
            &self.focus,
            &ctx.discovered,
            ctx.support.includes(),
            &deps::supports,
            cancel,
        )?;
        let traversal = traverse(&graph, &set, Direction::In, self.max_depth, cancel)?;
        let families: HashSet<Family> = set
            .files
            .iter()
            .filter_map(|f| graph.family_of(&f.rel))
            .collect();
        let relevant = |u: &UnresolvedRel| {
            families.contains(&u.family)
                || u.family == Family::JsTs
                || (u.family == Family::Rust && u.kind == RelationshipKind::Include)
        };
        let unresolved = graph.unresolved(&relevant);
        let mut limits_hit: Vec<LimitHit> = traversal
            .file_limit_hit
            .map(|n| LimitHit {
                limit: "max_depth".to_string(),
                value: u64::from(self.max_depth),
                detail: format!(
                    "{n} files at depth {} have dependents that were not followed",
                    self.max_depth
                ),
            })
            .into_iter()
            .collect();
        limits_hit.extend_from_slice(graph.limits_hit());
        let findings = dependents(&traversal, cancel)?;
        let direct = findings.iter().filter(|f| f.depth == 1).count();
        let mut details = graph.common_details();
        details.insert("max_depth".to_string(), json!(self.max_depth));
        details.insert("direct".to_string(), json!(direct));
        details.insert("transitive".to_string(), json!(findings.len() - direct));
        insert_focus(&mut details, None);
        *self.focus_set.lock().map_err(|_| poisoned())? = Some(set);
        if cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        Ok(Analysis {
            findings,
            unresolved,
            limits_hit,
            details: Some(details),
        })
    }

    /// Settles the focus set with what the read found and fills the focus
    /// keys of `details`.
    fn after_scope(
        &self,
        _ctx: &RunContext,
        read: &ReadOutcome,
        analysis: &mut Analysis<ImpactFinding>,
    ) -> Result<(), AnalyzeError> {
        let Some(mut set) = self.focus_set.lock().map_err(|_| poisoned())?.take() else {
            return Ok(());
        };
        set.settle(read);
        if let Some(details) = analysis.details.as_mut() {
            insert_focus(details, Some(&set));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::discovery::{Includes, discover};
    use crate::analyze::graph::traverse_observed;
    use crate::analyze::read::{Reader, SupportReader};
    use std::fs;
    use std::path::Path;

    /// `src/b.ts` and `src/c.ts` import `src/a.ts`; the focus is `src/a.ts`.
    fn workspace(root: &Path) -> Request {
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.ts"), "export const a = 1;\n").unwrap();
        for name in ["b", "c"] {
            fs::write(root.join(format!("src/{name}.ts")), "import './a';\n").unwrap();
        }
        Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap()
    }

    #[test]
    fn a_token_cancelled_before_the_traversal_stops_it_before_the_first_visit() {
        let ws = tempfile::tempdir().unwrap();
        let request = workspace(ws.path());
        let live = CancellationToken::new();
        let supported = deps::supports;
        let ctx = RunContext {
            root: request.root.clone(),
            discovered: discover(&request.root, &request.scope, &supported, &live).unwrap(),
            support: SupportReader::new(&request.root, &Includes::default(), &live),
        };
        let mut reader = Reader::new(&ctx.discovered.files, &supported, false, &live);
        let graph = DepGraph::build(&mut reader, &ctx, &live).unwrap();
        let paths = check_focus(&request.root, &request.scope, &["src/a.ts".to_string()]).unwrap();
        let set = focus_files(
            &request.root,
            &paths,
            &ctx.discovered,
            &Includes::default(),
            &supported,
            &live,
        )
        .unwrap();
        let found = traverse(&graph, &set, Direction::In, 5, &live).unwrap();
        assert_eq!(dependents(&found, &live).unwrap().len(), 2);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = traverse(&graph, &set, Direction::In, 5, &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
    }

    #[test]
    fn a_token_cancelled_during_the_traversal_stops_it_before_the_next_visit() {
        let ws = tempfile::tempdir().unwrap();
        let request = workspace(ws.path());
        fs::write(ws.path().join("src/d.ts"), "import './b';\n").unwrap();
        fs::write(ws.path().join("src/e.ts"), "import './d';\n").unwrap();
        let live = CancellationToken::new();
        let supported = deps::supports;
        let ctx = RunContext {
            root: request.root.clone(),
            discovered: discover(&request.root, &request.scope, &supported, &live).unwrap(),
            support: SupportReader::new(&request.root, &Includes::default(), &live),
        };
        let mut reader = Reader::new(&ctx.discovered.files, &supported, false, &live);
        let graph = DepGraph::build(&mut reader, &ctx, &live).unwrap();
        let paths = check_focus(&request.root, &request.scope, &["src/a.ts".to_string()]).unwrap();
        let set = focus_files(
            &request.root,
            &paths,
            &ctx.discovered,
            &Includes::default(),
            &supported,
            &live,
        )
        .unwrap();
        let mut all = 0;
        traverse_observed(&graph, &set, Direction::In, 5, &live, &mut |_| all += 1).unwrap();
        assert!(all >= 4, "the traversal visits at least 4 files, got {all}");
        let cancel = CancellationToken::new();
        let mut visits = 0;
        let result = traverse_observed(&graph, &set, Direction::In, 5, &cancel, &mut |_| {
            visits += 1;
            cancel.cancel();
        });
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
        assert_eq!(visits, 1);
    }
}
