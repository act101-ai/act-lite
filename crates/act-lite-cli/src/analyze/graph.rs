//! The `graph` analysis: the dependency edges of a scope, or the neighbourhood of focus files.
//!
//! Without focus files every edge of the scope is reported, without a depth.
//! With focus files a breadth-first traversal starts from them at distance 0
//! and visits files in distance order, then byte order of path: `out` follows
//! edges from a file to its targets, `in` from the sources to a file, `both`
//! runs the two and reports the union. An edge is reported when its near end
//! is at a distance below `max_depth`; its depth is that distance plus one.

use super::deps::{self, DepGraph, Edge, Family, UnresolvedRel};
use super::envelope::{AnalyzeError, Finding, LimitHit, Location};
use super::focus::{FocusSet, check_focus, focus_files};
use super::read::{FileSource, ReadOutcome};
use super::run::{Analysis, Analyzer, Request, RunContext};
use act_lite_parser::Grammar;
use act_lite_parser::deps::RelationshipKind;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde_json::{Map, Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// The traversal bound when the request gives none.
pub const DEFAULT_MAX_DEPTH: u32 = 1;
/// The largest `max_depth` a request can give.
pub const MAX_MAX_DEPTH: u32 = 1000;
/// The most entries of each focus list in `details`.
pub(crate) const MAX_FOCUS: usize = 30;
/// The most entries of `details.not_followed`.
const MAX_NOT_FOLLOWED: usize = 100;
/// The most focus files the no-focus error lists.
const MAX_NO_FOCUS_ENTRIES: usize = 10;

/// Which edges a traversal follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From the sources to a visited file: what depends on the focus files.
    In,
    /// From a visited file to its targets: what the focus files depend on.
    Out,
    Both,
}

impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Both => "both",
        }
    }

    fn follows_out(self) -> bool {
        matches!(self, Direction::Out | Direction::Both)
    }

    fn follows_in(self) -> bool {
        matches!(self, Direction::In | Direction::Both)
    }
}

impl FromStr for Direction {
    /// The request error text.
    type Err = String;

    fn from_str(s: &str) -> Result<Direction, String> {
        match s {
            "in" => Ok(Direction::In),
            "out" => Ok(Direction::Out),
            "both" => Ok(Direction::Both),
            _ => Err(format!("direction must be one of in, out, both, got {s}")),
        }
    }
}

/// A request error in a parameter other than a path or the limit.
fn request_error(message: String) -> AnalyzeError {
    AnalyzeError::InvalidRequest(message)
}

/// The failure of a focus state lock that a panic poisoned.
fn poisoned() -> AnalyzeError {
    AnalyzeError::Failed {
        analysis: GraphAnalyzer::NAME,
        cause: "focus state poisoned".to_string(),
    }
}

/// Reports the dependency edges of a scope, or the edges a traversal from
/// focus files reaches.
pub struct GraphAnalyzer {
    /// The checked focus paths, inside the workspace root; empty without focus.
    focus: Vec<PathBuf>,
    direction: Direction,
    max_depth: u32,
    /// The focus set found by `analyze_in`, completed by `after_scope`.
    focus_set: Mutex<Option<FocusSet>>,
}

impl GraphAnalyzer {
    /// Checks, in order: the `max_depth` range, the `direction` value, the
    /// parameters that need a focus path, then each focus path.
    pub fn new(
        request: &Request,
        focus: &[String],
        direction: Option<&str>,
        max_depth: Option<u32>,
    ) -> Result<GraphAnalyzer, AnalyzeError> {
        if let Some(n) = max_depth
            && !(1..=MAX_MAX_DEPTH).contains(&n)
        {
            return Err(request_error(format!(
                "max_depth must be between 1 and {MAX_MAX_DEPTH}, got {n}"
            )));
        }
        let parsed = direction
            .map(str::parse::<Direction>)
            .transpose()
            .map_err(request_error)?;
        if focus.is_empty() {
            if max_depth.is_some() {
                return Err(request_error("max_depth needs a focus path".to_string()));
            }
            if direction.is_some() {
                return Err(request_error("direction needs a focus path".to_string()));
            }
        }
        let focus = check_focus(&request.root, &request.scope, focus)?;
        Ok(GraphAnalyzer {
            focus,
            direction: parsed.unwrap_or(Direction::Both),
            max_depth: max_depth.unwrap_or(DEFAULT_MAX_DEPTH),
            focus_set: Mutex::new(None),
        })
    }
}

/// One reported edge: the edge object plus `depth`, `null` without focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphFinding {
    pub edge: Edge,
    pub depth: Option<u32>,
    location: Location,
}

impl GraphFinding {
    pub fn new(edge: Edge, depth: Option<u32>) -> GraphFinding {
        let location = Location {
            file: edge.file.clone(),
            range: None,
        };
        GraphFinding {
            edge,
            depth,
            location,
        }
    }
}

impl Serialize for GraphFinding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let edge = &self.edge;
        let mut kinds: Vec<&str> = edge.kinds.iter().map(|k| k.as_str()).collect();
        kinds.sort_unstable();
        let mut s = serializer.serialize_struct("GraphFinding", 7)?;
        s.serialize_field("file", &edge.file)?;
        s.serialize_field("to", &edge.to)?;
        s.serialize_field("to_analyzed", &edge.to_analyzed)?;
        s.serialize_field("depth", &self.depth)?;
        s.serialize_field("kinds", &kinds)?;
        s.serialize_field("evidence", &edge.evidence)?;
        s.serialize_field("evidence_total", &edge.evidence_total)?;
        s.end()
    }
}

impl Finding for GraphFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    /// `depth` ascending, then `file`, then `to`.
    fn order(&self, other: &Self) -> Ordering {
        self.depth
            .cmp(&other.depth)
            .then_with(|| self.location.cmp(&other.location))
            .then_with(|| self.edge.to.as_bytes().cmp(other.edge.to.as_bytes()))
    }
}

/// An edge a traversal reported, with its depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraversedEdge {
    pub edge: Edge,
    pub depth: u32,
}

/// What a traversal from focus files found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Traversal {
    /// The reported edges, by depth, then file, then target.
    pub edges: Vec<TraversedEdge>,
    /// Each visited file and its distance; focus files are at 0. With
    /// `both`, the smaller of the two distances.
    pub distance: BTreeMap<String, u32>,
    /// The edge through which each file at distance 1 or more was first
    /// reached (with `both`, by the traversal that gives its distance, `out`
    /// first on a tie).
    pub first_edge: BTreeMap<String, Edge>,
    /// The number of files at distance `max_depth` with an edge the traversal
    /// would follow that is not in the result; `None` when there is none.
    pub edge_limit_hit: Option<u32>,
    /// For `in` and `both`: the number of files at distance `max_depth` with
    /// a dependent that is neither a focus file nor visited; `None` when
    /// there is none, and always for `out`.
    pub file_limit_hit: Option<u32>,
    /// The targets outside the analyzed set that the `out` traversal reached.
    pub not_followed: BTreeSet<String>,
}

/// One direction's breadth-first traversal.
struct Walk<'g> {
    /// The reported edges, keyed by (source, target), with their depth.
    reported: BTreeMap<(&'g str, &'g str), (u32, &'g Edge)>,
    distance: BTreeMap<String, u32>,
    first_edge: BTreeMap<String, &'g Edge>,
    /// The files at distance `max_depth` and the edges they would follow.
    frontier: Vec<(&'g str, &'g [&'g Edge])>,
    not_followed: BTreeSet<String>,
}

/// The edges of `graph` by their source (`out`) or their target (`in`).
fn index(graph: &DepGraph, out: bool) -> HashMap<&str, Vec<&Edge>> {
    let mut by: HashMap<&str, Vec<&Edge>> = HashMap::new();
    for edge in graph.edges() {
        let key = if out { &edge.file } else { &edge.to };
        by.entry(key.as_str()).or_default().push(edge);
    }
    by
}

fn walk<'g>(
    near: &'g HashMap<&'g str, Vec<&'g Edge>>,
    focus: &'g FocusSet,
    out: bool,
    max_depth: u32,
    cancel: &CancellationToken,
    on_visit: &mut dyn FnMut(&str),
) -> Result<Walk<'g>, AnalyzeError> {
    let mut walk = Walk {
        reported: BTreeMap::new(),
        distance: BTreeMap::new(),
        first_edge: BTreeMap::new(),
        frontier: Vec::new(),
        not_followed: BTreeSet::new(),
    };
    let mut level: Vec<&str> = focus.files.iter().map(|f| f.rel.as_str()).collect();
    for file in &level {
        walk.distance.insert(file.to_string(), 0);
    }
    let mut depth = 0;
    while !level.is_empty() {
        let mut next: Vec<&str> = Vec::new();
        for file in &level {
            if cancel.is_cancelled() {
                return Err(AnalyzeError::Cancelled);
            }
            on_visit(file);
            let Some((key, edges)) = near.get_key_value(file) else {
                continue;
            };
            if depth == max_depth {
                walk.frontier.push((key, edges.as_slice()));
                continue;
            }
            for edge in edges {
                walk.reported
                    .entry((edge.file.as_str(), edge.to.as_str()))
                    .or_insert((depth + 1, edge));
                let far = if out { &edge.to } else { &edge.file };
                if out && !edge.to_analyzed {
                    walk.not_followed.insert(far.clone());
                    continue;
                }
                if !walk.distance.contains_key(far) {
                    walk.distance.insert(far.clone(), depth + 1);
                    walk.first_edge.insert(far.clone(), edge);
                    next.push(far);
                }
            }
        }
        next.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        level = next;
        depth += 1;
    }
    Ok(walk)
}

/// The two traversals a direction runs, `out` first.
fn walks<'g>(
    out_index: &'g HashMap<&'g str, Vec<&'g Edge>>,
    in_index: &'g HashMap<&'g str, Vec<&'g Edge>>,
    focus: &'g FocusSet,
    direction: Direction,
    max_depth: u32,
    cancel: &CancellationToken,
    on_visit: &mut dyn FnMut(&str),
) -> Result<(Option<Walk<'g>>, Option<Walk<'g>>), AnalyzeError> {
    let out = if direction.follows_out() {
        Some(walk(out_index, focus, true, max_depth, cancel, on_visit)?)
    } else {
        None
    };
    let inward = if direction.follows_in() {
        Some(walk(in_index, focus, false, max_depth, cancel, on_visit)?)
    } else {
        None
    };
    Ok((out, inward))
}

/// The union of the traversals, with the smaller depth for an edge both
/// found, and the two limit counts. The token is checked before the sort.
fn combine(
    out: Option<&Walk<'_>>,
    inward: Option<&Walk<'_>>,
    cancel: &CancellationToken,
) -> Result<Traversal, AnalyzeError> {
    let mut reported: BTreeMap<(&str, &str), (u32, &Edge)> = BTreeMap::new();
    let mut distance: BTreeMap<String, u32> = BTreeMap::new();
    let mut first_edge: BTreeMap<String, Edge> = BTreeMap::new();
    for walk in out.into_iter().chain(inward) {
        for (key, (depth, edge)) in &walk.reported {
            let entry = reported.entry(*key).or_insert((*depth, edge));
            if *depth < entry.0 {
                *entry = (*depth, edge);
            }
        }
        for (file, d) in &walk.distance {
            let known = distance.get(file).copied();
            if known.is_none_or(|k| *d < k) {
                distance.insert(file.clone(), *d);
                match walk.first_edge.get(file) {
                    Some(edge) => first_edge.insert(file.clone(), (*edge).clone()),
                    None => first_edge.remove(file),
                };
            }
        }
    }
    let mut cut: BTreeSet<&str> = BTreeSet::new();
    for walk in out.into_iter().chain(inward) {
        for (file, edges) in &walk.frontier {
            if edges
                .iter()
                .any(|e| !reported.contains_key(&(e.file.as_str(), e.to.as_str())))
            {
                cut.insert(file);
            }
        }
    }
    let file_limit_hit = inward.and_then(|walk| {
        let n = walk
            .frontier
            .iter()
            .filter(|(_, edges)| edges.iter().any(|e| !walk.distance.contains_key(&e.file)))
            .count() as u32;
        (n > 0).then_some(n)
    });
    let mut edges: Vec<TraversedEdge> = reported
        .into_values()
        .map(|(depth, edge)| TraversedEdge {
            edge: edge.clone(),
            depth,
        })
        .collect();
    if cancel.is_cancelled() {
        return Err(AnalyzeError::Cancelled);
    }
    edges.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| a.edge.file.as_bytes().cmp(b.edge.file.as_bytes()))
            .then_with(|| a.edge.to.as_bytes().cmp(b.edge.to.as_bytes()))
    });
    Ok(Traversal {
        edges,
        distance,
        first_edge,
        edge_limit_hit: (!cut.is_empty()).then_some(cut.len() as u32),
        file_limit_hit,
        not_followed: out.map(|w| w.not_followed.clone()).unwrap_or_default(),
    })
}

/// The breadth-first traversal of `graph` from the files of `focus`. The
/// token is checked before each visit.
pub fn traverse(
    graph: &DepGraph,
    focus: &FocusSet,
    direction: Direction,
    max_depth: u32,
    cancel: &CancellationToken,
) -> Result<Traversal, AnalyzeError> {
    traverse_observed(graph, focus, direction, max_depth, cancel, &mut |_| {})
}

/// As [`traverse`], calling `on_visit` with each file as it is visited,
/// after the token check.
pub(crate) fn traverse_observed(
    graph: &DepGraph,
    focus: &FocusSet,
    direction: Direction,
    max_depth: u32,
    cancel: &CancellationToken,
    on_visit: &mut dyn FnMut(&str),
) -> Result<Traversal, AnalyzeError> {
    let out_index = index(graph, true);
    let in_index = index(graph, false);
    let (out, inward) = walks(
        &out_index, &in_index, focus, direction, max_depth, cancel, on_visit,
    )?;
    combine(out.as_ref(), inward.as_ref(), cancel)
}

impl Analyzer for GraphAnalyzer {
    const NAME: &'static str = "graph";
    const HINT: &'static str =
        "Narrow the result with paths, include, exclude or focus, or change limit.";
    type Finding = GraphFinding;

    /// With a focus, the hint also names `max_depth`.
    fn hint(&self) -> &'static str {
        if self.focus.is_empty() {
            Self::HINT
        } else {
            "Narrow the result with paths, include, exclude or focus, or change limit; max_depth sets how far the traversal goes."
        }
    }

    fn narrowing(&self) -> &'static [&'static str] {
        if self.focus.is_empty() {
            &["focus"]
        } else {
            &["focus", "max_depth"]
        }
    }

    fn supports(&self, grammar: Grammar) -> bool {
        deps::supports(grammar)
    }

    fn analyze(
        &self,
        _files: &mut dyn FileSource,
        _cancel: &CancellationToken,
    ) -> Result<Analysis<GraphFinding>, AnalyzeError> {
        Err(AnalyzeError::Failed {
            analysis: Self::NAME,
            cause: "the graph analysis needs the run context".to_string(),
        })
    }

    fn analyze_in(
        &self,
        files: &mut dyn FileSource,
        ctx: &RunContext,
        cancel: &CancellationToken,
    ) -> Result<Analysis<GraphFinding>, AnalyzeError> {
        let graph = DepGraph::build(files, ctx, cancel)?;
        let mut details = graph.common_details();
        let (findings, unresolved, limits_hit, not_followed) = if self.focus.is_empty() {
            details.insert("direction".to_string(), Value::Null);
            details.insert("max_depth".to_string(), Value::Null);
            let findings: Vec<GraphFinding> = graph
                .edges()
                .iter()
                .map(|e| GraphFinding::new(e.clone(), None))
                .collect();
            (
                findings,
                graph.unresolved(&|_| true),
                Vec::new(),
                BTreeSet::new(),
            )
        } else {
            details.insert("direction".to_string(), json!(self.direction.as_str()));
            details.insert("max_depth".to_string(), json!(self.max_depth));
            let set = focus_files(
                &ctx.root,
                &self.focus,
                &ctx.discovered,
                ctx.support.includes(),
                &deps::supports,
                cancel,
            )?;
            let out_index = index(&graph, true);
            let in_index = index(&graph, false);
            let (out, inward) = walks(
                &out_index,
                &in_index,
                &set,
                self.direction,
                self.max_depth,
                cancel,
                &mut |_| {},
            )?;
            let read: HashSet<&str> = out.as_ref().map_or_else(HashSet::new, |w| {
                w.distance
                    .iter()
                    .filter(|(_, d)| **d < self.max_depth)
                    .map(|(f, _)| f.as_str())
                    .collect()
            });
            let families: HashSet<Family> = set
                .files
                .iter()
                .filter_map(|f| graph.family_of(&f.rel))
                .collect();
            let follows_in = self.direction.follows_in();
            let relevant = |u: &UnresolvedRel| {
                read.contains(u.file.as_str())
                    || (follows_in
                        && (families.contains(&u.family)
                            || u.family == Family::JsTs
                            || (u.family == Family::Rust && u.kind == RelationshipKind::Include)))
            };
            let unresolved = graph.unresolved(&relevant);
            let traversal = combine(out.as_ref(), inward.as_ref(), cancel)?;
            let limits_hit: Vec<LimitHit> = traversal
                .edge_limit_hit
                .map(|n| LimitHit {
                    limit: "max_depth".to_string(),
                    value: u64::from(self.max_depth),
                    detail: format!(
                        "{n} files at depth {} have relationships that were not followed",
                        self.max_depth
                    ),
                })
                .into_iter()
                .collect();
            let findings = traversal
                .edges
                .into_iter()
                .map(|t| GraphFinding::new(t.edge, Some(t.depth)))
                .collect();
            *self.focus_set.lock().map_err(|_| poisoned())? = Some(set);
            (findings, unresolved, limits_hit, traversal.not_followed)
        };
        let files: BTreeSet<&str> = findings
            .iter()
            .flat_map(|f: &GraphFinding| [f.edge.file.as_str(), f.edge.to.as_str()])
            .collect();
        details.insert("files".to_string(), json!(files.len()));
        let listed: Vec<Value> = not_followed
            .iter()
            .take(MAX_NOT_FOLLOWED)
            .map(|file| json!({ "file": file }))
            .collect();
        details.insert("not_followed".to_string(), Value::Array(listed));
        details.insert("not_followed_total".to_string(), json!(not_followed.len()));
        insert_focus(&mut details, None);
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

    /// Settles the focus set with what the read found, raises the error for
    /// an `out` traversal without an analyzed focus file, and fills the focus
    /// keys of `details`.
    fn after_scope(
        &self,
        _ctx: &RunContext,
        read: &ReadOutcome,
        analysis: &mut Analysis<GraphFinding>,
    ) -> Result<(), AnalyzeError> {
        let Some(mut set) = self.focus_set.lock().map_err(|_| poisoned())?.take() else {
            return Ok(());
        };
        set.settle(read);
        if self.direction == Direction::Out && !set.files.iter().any(|f| f.analyzed) {
            return Err(no_focus_error(&set));
        }
        if let Some(details) = analysis.details.as_mut() {
            insert_focus(details, Some(&set));
        }
        Ok(())
    }
}

/// The focus keys of `details`, empty without a focus set.
pub(crate) fn insert_focus(details: &mut Map<String, Value>, set: Option<&FocusSet>) {
    let (paths, files, not_analyzed, files_not_analyzed): (
        &[String],
        usize,
        &[String],
        Vec<Value>,
    ) = match set {
        None => (&[], 0, &[], Vec::new()),
        Some(set) => (
            &set.paths,
            set.files.len(),
            &set.paths_not_analyzed,
            set.files
                .iter()
                .filter(|f| !f.analyzed)
                .map(|f| json!({"file": f.rel, "reason": f.reason}))
                .collect(),
        ),
    };
    let capped = |list: &[String]| json!(list.iter().take(MAX_FOCUS).collect::<Vec<_>>());
    details.insert("focus".to_string(), capped(paths));
    details.insert("focus_total".to_string(), json!(paths.len()));
    details.insert("focus_files".to_string(), json!(files));
    details.insert("focus_not_analyzed".to_string(), capped(not_analyzed));
    details.insert(
        "focus_not_analyzed_total".to_string(),
        json!(not_analyzed.len()),
    );
    details.insert(
        "focus_files_not_analyzed_total".to_string(),
        json!(files_not_analyzed.len()),
    );
    details.insert(
        "focus_files_not_analyzed".to_string(),
        Value::Array(files_not_analyzed.into_iter().take(MAX_FOCUS).collect()),
    );
}

/// The error for an `out` traversal from no analyzed focus file: each focus
/// file with its reason and each focus directory with no file, in byte order,
/// at most 10.
fn no_focus_error(set: &FocusSet) -> AnalyzeError {
    let mut entries: Vec<(&str, String)> = set
        .files
        .iter()
        .filter(|f| !f.analyzed)
        .map(|f| {
            let reason = f.reason.as_deref().unwrap_or("not analyzed");
            (f.rel.as_str(), format!("{} ({reason})", f.rel))
        })
        .collect();
    for path in set.paths.iter().filter(|p| p.ends_with('/')) {
        if !set.files.iter().any(|f| f.rel.starts_with(path.as_str())) {
            entries.push((path.as_str(), format!("{path} (no file)")));
        }
    }
    entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut text: Vec<String> = entries
        .iter()
        .take(MAX_NO_FOCUS_ENTRIES)
        .map(|(_, t)| t.clone())
        .collect();
    if entries.len() > MAX_NO_FOCUS_ENTRIES {
        text.push("…".to_string());
    }
    request_error(format!(
        "no focus file can be analyzed: {}",
        text.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::discovery::{Includes, discover};
    use crate::analyze::envelope::AnalyzeError;
    use crate::analyze::read::{Reader, SupportReader};
    use std::fs;
    use std::path::Path;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// `src/a.ts` imports three files; the focus is `src/a.ts`.
    fn workspace(root: &Path) -> Request {
        fs::create_dir_all(root.join(".git")).unwrap();
        write(
            root,
            "src/a.ts",
            "import './b';\nimport './c';\nimport './d';\n",
        );
        for name in ["b", "c", "d"] {
            write(root, &format!("src/{name}.ts"), "export const x = 1;\n");
        }
        Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap()
    }

    fn context(request: &Request, cancel: &CancellationToken) -> RunContext {
        let supported = deps::supports;
        RunContext {
            root: request.root.clone(),
            discovered: discover(&request.root, &request.scope, &supported, cancel).unwrap(),
            support: SupportReader::new(&request.root, &Includes::default(), cancel),
        }
    }

    #[test]
    fn a_token_cancelled_during_the_traversal_stops_before_the_next_visit() {
        let ws = tempfile::tempdir().unwrap();
        let request = workspace(ws.path());
        let cancel = CancellationToken::new();
        let ctx = context(&request, &cancel);
        let mut reader = Reader::new(&ctx.discovered.files, &deps::supports, false, &cancel);
        let graph = DepGraph::build(&mut reader, &ctx, &cancel).unwrap();
        let paths = check_focus(&request.root, &request.scope, &["src/a.ts".to_string()]).unwrap();
        let set = focus_files(
            &request.root,
            &paths,
            &ctx.discovered,
            &Includes::default(),
            &deps::supports,
            &cancel,
        )
        .unwrap();
        let mut visits = 0;
        let result = traverse_observed(&graph, &set, Direction::Out, 2, &cancel, &mut |_| {
            visits += 1;
            if visits == 2 {
                cancel.cancel();
            }
        });
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
        assert_eq!(visits, 2);
    }

    /// An analyzer whose focus state lock is poisoned.
    fn poisoned(request: &Request) -> GraphAnalyzer {
        let analyzer =
            GraphAnalyzer::new(request, &["src/a.ts".to_string()], Some("out"), None).unwrap();
        let _ = std::thread::scope(|s| {
            s.spawn(|| {
                let _held = analyzer.focus_set.lock().unwrap();
                panic!("poison the focus state");
            })
            .join()
        });
        assert!(analyzer.focus_set.is_poisoned());
        analyzer
    }

    fn poisoned_error() -> AnalyzeError {
        AnalyzeError::Failed {
            analysis: "graph",
            cause: "focus state poisoned".to_string(),
        }
    }

    #[test]
    fn a_poisoned_focus_state_fails_the_analysis() {
        let ws = tempfile::tempdir().unwrap();
        let request = workspace(ws.path());
        let analyzer = poisoned(&request);
        let cancel = CancellationToken::new();
        let ctx = context(&request, &cancel);
        let mut reader = Reader::new(&ctx.discovered.files, &deps::supports, false, &cancel);
        let result = analyzer.analyze_in(&mut reader, &ctx, &cancel);
        assert_eq!(result.err(), Some(poisoned_error()));
        let mut analysis = Analysis {
            findings: Vec::new(),
            unresolved: Vec::new(),
            limits_hit: Vec::new(),
            details: None,
        };
        let result = analyzer.after_scope(&ctx, &ReadOutcome::default(), &mut analysis);
        assert_eq!(result.err(), Some(poisoned_error()));
    }
}
