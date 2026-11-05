//! The dependency model: the relationships of the analyzed files, their
//! resolution outcomes and the file-to-file edges they give.
//!
//! Each relationship has one outcome: `resolved` to one or more workspace
//! files, `external` only when its family's rule places the target outside
//! the workspace by a positive test, else `unresolved` with a reason. All
//! resolved relationships from one file to another form one edge.
//!
//! Probes compare names byte for byte by listing each directory on the path,
//! so the result does not depend on the file system's case rules. A symlink
//! is resolved: a target inside the workspace is used at its own path, a
//! target outside it is `Probe::Outside`, a broken one is no match.

pub mod go;
pub mod js;
pub mod python;
pub mod rust;

use super::discovery::{Includes, walk_workspace};
use super::envelope::{AnalyzeError, Unresolved};
use super::read::{DirEntry, EntryKind, FileSource, Parsed, RemovedRegion, SymlinkTarget};
use super::run::RunContext;
use act_lite_core::CompactRange;
use act_lite_parser::Grammar;
use act_lite_parser::deps::{Relationship, RelationshipKind, extract};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde_json::{Map, Value, json};
use std::any::{Any, TypeId};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// The most relationships an edge lists in `evidence`.
const MAX_EDGE_EVIDENCE: usize = 5;
/// The most entries of `details.resolution_inputs`.
const MAX_RESOLUTION_INPUTS: usize = 100;
/// The most symlinks one probe follows.
const MAX_SYMLINK_HOPS: usize = 40;

/// A group of languages that share one resolution rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// JavaScript, JSX, TypeScript and TSX.
    JsTs,
    Python,
    Go,
    Rust,
}

impl Family {
    /// The family of `grammar`; `None` for a language without resolution.
    pub fn of(grammar: Grammar) -> Option<Family> {
        match grammar {
            Grammar::JavaScript | Grammar::Jsx | Grammar::TypeScript | Grammar::Tsx => {
                Some(Family::JsTs)
            }
            Grammar::Python => Some(Family::Python),
            Grammar::Go => Some(Family::Go),
            Grammar::Rust => Some(Family::Rust),
            _ => None,
        }
    }
}

/// Whether the dependency analyses read files of `grammar`.
pub fn supports(grammar: Grammar) -> bool {
    Family::of(grammar).is_some()
}

/// The outcome of one relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The workspace-relative target files; more than one for a Go package
    /// or a Python case with a wildcard import; none for a target that loads
    /// no file (a namespace package).
    Resolved(Vec<String>),
    /// Outside the workspace, by the family's positive test.
    External,
    /// The reason text.
    Unresolved(String),
}

/// The resolution rule of one family.
pub trait Resolver: Send + Sync {
    fn family(&self) -> Family;

    /// The outcome of `rel`, a relationship of the analyzed file `file`.
    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome;
}

/// The resolvers of the four families.
pub fn resolvers() -> [Box<dyn Resolver>; 4] {
    [
        Box::new(js::JsResolver),
        Box::new(python::PythonResolver),
        Box::new(go::GoResolver),
        Box::new(rust::RustResolver),
    ]
}

/// What a probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// A file, at its own workspace-relative path.
    File(String),
    /// A directory, at its own workspace-relative path (`""` for the root).
    Dir(String),
    None,
    /// The path, or a symlink on it, leads outside the workspace.
    Outside,
}

/// The kind of a resolution input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    PackageJson,
    /// `tsconfig.json`, `jsconfig.json` and the files they extend.
    Tsconfig,
    GoMod,
    CargoToml,
    PyprojectToml,
    /// `requirements*.txt`.
    Requirements,
    SetupCfg,
}

impl InputKind {
    /// The `kind` value in `details.resolution_inputs`.
    pub fn as_str(self) -> &'static str {
        match self {
            InputKind::PackageJson => "package.json",
            InputKind::Tsconfig => "tsconfig",
            InputKind::GoMod => "go.mod",
            InputKind::CargoToml => "Cargo.toml",
            InputKind::PyprojectToml => "pyproject.toml",
            InputKind::Requirements => "requirements",
            InputKind::SetupCfg => "setup.cfg",
        }
    }

    /// The family whose rules read inputs of this kind.
    pub fn family(self) -> Family {
        match self {
            InputKind::PackageJson | InputKind::Tsconfig => Family::JsTs,
            InputKind::GoMod => Family::Go,
            InputKind::CargoToml => Family::Rust,
            InputKind::PyprojectToml | InputKind::Requirements | InputKind::SetupCfg => {
                Family::Python
            }
        }
    }
}

/// A resolution input as read.
struct Input {
    kind: InputKind,
    text: Option<String>,
    error: Option<String>,
}

/// What resolution keeps for the whole build.
#[derive(Default)]
struct State {
    /// Listings by the directory's own path; `None` when it cannot be listed.
    dirs: HashMap<String, Option<Vec<DirEntry>>>,
    inputs: BTreeMap<String, Input>,
    sources: HashMap<String, Result<Parsed, String>>,
    workspace: OnceCell<Vec<String>>,
    cache: HashMap<TypeId, Box<dyn Any + Send>>,
}

/// What a resolver can read while it resolves one relationship.
pub struct ResolveCx<'a> {
    ctx: &'a RunContext,
    state: &'a mut State,
    analyzable: &'a HashSet<String>,
    file: &'a str,
    removed: &'a [RemovedRegion],
    cancel: &'a CancellationToken,
}

impl ResolveCx<'_> {
    /// Whether the workspace-relative path `rel` exists, each name on it
    /// compared byte for byte. Symlinks are resolved; `..` that leaves the
    /// workspace is `Probe::Outside`.
    pub fn probe_file(&mut self, rel: &str) -> Probe {
        let Some(path) = lexical("", rel) else {
            return Probe::Outside;
        };
        let mut parts: VecDeque<String> = segments(&path);
        let mut at = String::new();
        let mut is_dir = true;
        let mut hops = 0;
        while let Some(name) = parts.pop_front() {
            if !is_dir {
                return Probe::None;
            }
            let Some(entry) = self
                .listing(&at)
                .and_then(|entries| entries.iter().find(|e| e.name == name))
                .cloned()
            else {
                return Probe::None;
            };
            match entry.kind {
                EntryKind::Dir => {
                    at = join(&at, &name);
                    is_dir = true;
                }
                EntryKind::File => {
                    at = join(&at, &name);
                    is_dir = false;
                }
                EntryKind::Symlink => match entry.target {
                    Some(SymlinkTarget::Inside(target)) => {
                        hops += 1;
                        if hops > MAX_SYMLINK_HOPS {
                            return Probe::None;
                        }
                        let mut next = segments(own_path(&target));
                        next.extend(parts);
                        parts = next;
                        at = String::new();
                        is_dir = true;
                    }
                    Some(SymlinkTarget::Outside) => return Probe::Outside,
                    _ => return Probe::None,
                },
            }
        }
        if is_dir {
            Probe::Dir(at)
        } else {
            Probe::File(at)
        }
    }

    /// The entries of the directory `rel`, found as `probe_file` finds it, in
    /// byte order of name; `None` when it is no directory or cannot be read.
    pub fn list_dir(&mut self, rel: &str) -> Option<&[DirEntry]> {
        let Probe::Dir(dir) = self.probe_file(rel) else {
            return None;
        };
        self.listing(&dir)
    }

    /// The listing of a directory given by its own path, read once.
    fn listing(&mut self, dir: &str) -> Option<&[DirEntry]> {
        let support = &self.ctx.support;
        self.state
            .dirs
            .entry(dir.to_string())
            .or_insert_with(|| support.list_dir(dir).ok())
            .as_deref()
    }

    /// The text of the resolution input `rel`, read once and listed in
    /// `details.resolution_inputs`. A read failure is listed with its
    /// `error`; the `Err` is the reason `<rel> could not be read`.
    pub fn input(&mut self, rel: &str, kind: InputKind) -> Result<&str, String> {
        if !self.state.inputs.contains_key(rel) {
            let (text, error) = match self.ctx.support.text(rel) {
                Ok(text) => (Some(text), None),
                Err(err) => (None, Some(err.to_string())),
            };
            self.state
                .inputs
                .insert(rel.to_string(), Input { kind, text, error });
        }
        match &self.state.inputs[rel].text {
            Some(text) => Ok(text),
            None => Err(format!("{rel} could not be read")),
        }
    }

    /// Records that the input `rel`, read with `input`, does not parse: its
    /// `resolution_inputs` entry gets `error`, and its family counts as
    /// failed. The first error of an input is kept; an input not read with
    /// `input` is not recorded.
    pub fn input_error(&mut self, rel: &str, error: String) {
        if let Some(input) = self.state.inputs.get_mut(rel) {
            input.error.get_or_insert(error);
        }
    }

    /// The first input of `family`, in path order, that could not be read
    /// or parsed.
    pub fn input_failed(&self, family: Family) -> Option<&str> {
        first_failed(&self.state.inputs, family)
    }

    /// Every regular file of the workspace, workspace-relative, in byte
    /// order: `.git` pruned, ignored paths and vendored and generated
    /// directories left out. Walked once per build, on the first call.
    pub fn workspace_files(&self) -> &[String] {
        self.state.workspace.get_or_init(|| {
            walk_workspace(&self.ctx.root, &Includes::default(), self.cancel)
                .map(|(files, _)| files)
                .unwrap_or_default()
        })
    }

    /// Whether `rel` is a file of the scope in a supported language, so that
    /// its own relationships are read.
    pub fn analyzed(&self, rel: &str) -> bool {
        self.analyzable.contains(rel)
    }

    /// `relative` joined to `base_dir` with `.` and `..` removed; `None`
    /// when the path leaves the workspace root.
    pub fn normalize(&self, base_dir: &str, relative: &str) -> Option<String> {
        lexical(base_dir, relative)
    }

    /// The source file `rel`, read and parsed once, with its test regions
    /// blanked as an analyzed read does; the `Err` is the reason
    /// `<rel> could not be read`.
    pub fn source(&mut self, rel: &str) -> Result<&Parsed, String> {
        let support = &self.ctx.support;
        self.state
            .sources
            .entry(rel.to_string())
            .or_insert_with(|| {
                support
                    .source(rel)
                    .map_err(|_| format!("{rel} could not be read"))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// The removed test regions of `file`: the file being resolved, or a
    /// file read with `source`; empty for any other.
    pub fn removed(&self, file: &str) -> &[RemovedRegion] {
        if file == self.file {
            return self.removed;
        }
        match self.state.sources.get(file) {
            Some(Ok(parsed)) => &parsed.removed,
            _ => &[],
        }
    }

    /// One value of type `T` for the whole build.
    pub fn cache<T: Default + Send + 'static>(&mut self) -> &mut T {
        self.state
            .cache
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut::<T>()
            .expect("the cache is keyed by type")
    }
}

/// A relationship and its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRel {
    pub relationship: Relationship,
    pub outcome: Outcome,
}

/// One relationship of an edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub range: CompactRange,
    pub kind: RelationshipKind,
    pub specifier: String,
}

impl Serialize for Evidence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Evidence", 3)?;
        s.serialize_field("range", &self.range)?;
        s.serialize_field("kind", self.kind.as_str())?;
        s.serialize_field("specifier", &self.specifier)?;
        s.end()
    }
}

/// All resolved relationships from one analyzed file to one other file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub file: String,
    pub to: String,
    /// Whether `to` is an analyzed file, so its own relationships were read.
    pub to_analyzed: bool,
    pub kinds: BTreeSet<RelationshipKind>,
    /// At most 5, in location order, then kind, then specifier.
    pub evidence: Vec<Evidence>,
    pub evidence_total: u32,
}

impl Edge {
    /// The edge's first relationship, in the compact form of paths.
    pub fn step(&self) -> Step {
        let first = &self.evidence[0];
        Step {
            file: self.file.clone(),
            range: first.range.clone(),
            kind: first.kind,
            specifier: first.specifier.clone(),
            to: self.to.clone(),
        }
    }
}

impl Serialize for Edge {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut kinds: Vec<&str> = self.kinds.iter().map(|k| k.as_str()).collect();
        kinds.sort_unstable();
        let mut s = serializer.serialize_struct("Edge", 6)?;
        s.serialize_field("file", &self.file)?;
        s.serialize_field("to", &self.to)?;
        s.serialize_field("to_analyzed", &self.to_analyzed)?;
        s.serialize_field("kinds", &kinds)?;
        s.serialize_field("evidence", &self.evidence)?;
        s.serialize_field("evidence_total", &self.evidence_total)?;
        s.end()
    }
}

/// An edge in the compact form used inside paths and chains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub file: String,
    pub range: CompactRange,
    pub kind: RelationshipKind,
    pub specifier: String,
    pub to: String,
}

impl Serialize for Step {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Step", 5)?;
        s.serialize_field("file", &self.file)?;
        s.serialize_field("range", &self.range)?;
        s.serialize_field("kind", self.kind.as_str())?;
        s.serialize_field("specifier", &self.specifier)?;
        s.serialize_field("to", &self.to)?;
        s.end()
    }
}

/// An unresolved relationship, as a relevance filter sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedRel {
    pub file: String,
    pub family: Family,
    pub kind: RelationshipKind,
}

/// The relationships of the analyzed files, their outcomes and edges.
#[derive(Debug, Default)]
pub struct DepGraph {
    analyzed: BTreeSet<String>,
    relationships: BTreeMap<String, Vec<ResolvedRel>>,
    edges: Vec<Edge>,
    /// The resolution inputs read: kind and error, by path.
    inputs: BTreeMap<String, (InputKind, Option<String>)>,
}

impl DepGraph {
    /// Builds the graph with the resolvers of the four families.
    pub fn build(
        files: &mut dyn FileSource,
        ctx: &RunContext,
        cancel: &CancellationToken,
    ) -> Result<DepGraph, AnalyzeError> {
        let all = resolvers();
        let refs: Vec<&dyn Resolver> = all.iter().map(|r| r.as_ref()).collect();
        DepGraph::build_with(files, ctx, &refs, cancel)
    }

    /// Pulls each file, extracts its relationships, drops its tree and
    /// resolves them with the resolver of the file's family. The token is
    /// checked before each file's resolution.
    pub fn build_with(
        files: &mut dyn FileSource,
        ctx: &RunContext,
        resolvers: &[&dyn Resolver],
        cancel: &CancellationToken,
    ) -> Result<DepGraph, AnalyzeError> {
        let analyzable: HashSet<String> = ctx
            .discovered
            .files
            .iter()
            .filter(|c| supports(c.grammar))
            .map(|c| c.rel.clone())
            .collect();
        let mut state = State::default();
        let mut graph = DepGraph::default();
        loop {
            if cancel.is_cancelled() {
                return Err(AnalyzeError::Cancelled);
            }
            let Some(parsed) = files.next_file()? else {
                break;
            };
            let Parsed {
                rel: file,
                source,
                removed,
            } = parsed;
            let family = Family::of(source.grammar());
            let found = extract(&source);
            drop(source);
            if cancel.is_cancelled() {
                return Err(AnalyzeError::Cancelled);
            }
            let resolver = family.and_then(|f| resolvers.iter().find(|r| r.family() == f));
            let mut resolved = Vec::with_capacity(found.len());
            for relationship in found {
                let outcome = match resolver {
                    Some(resolver) => {
                        let mut cx = ResolveCx {
                            ctx,
                            state: &mut state,
                            analyzable: &analyzable,
                            file: &file,
                            removed: &removed,
                            cancel,
                        };
                        resolver.resolve(&file, &relationship, &mut cx)
                    }
                    None => Outcome::Unresolved("no resolver for the file's language".to_string()),
                };
                resolved.push(ResolvedRel {
                    relationship,
                    outcome,
                });
            }
            graph.analyzed.insert(file.clone());
            graph.relationships.insert(file, resolved);
        }
        for (file, rels) in &mut graph.relationships {
            let failed = family_of(file).and_then(|f| first_failed(&state.inputs, f));
            if let Some(input) = failed {
                for rel in rels.iter_mut() {
                    if rel.outcome == Outcome::External {
                        rel.outcome = Outcome::Unresolved(format!("{input} could not be read"));
                    }
                }
            }
        }
        graph.edges = edges(&graph.relationships, &graph.analyzed);
        graph.inputs = state
            .inputs
            .into_iter()
            .map(|(path, input)| (path, (input.kind, input.error)))
            .collect();
        Ok(graph)
    }

    /// Sorted by `file`, then `to`.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The files whose relationships were read.
    pub fn analyzed(&self) -> &BTreeSet<String> {
        &self.analyzed
    }

    /// The family of any workspace file, by its grammar.
    pub fn family_of(&self, file: &str) -> Option<Family> {
        family_of(file)
    }

    /// The `incomplete.unresolved` entries of the unresolved relationships
    /// that `relevant` keeps, sorted by file, range, kind, target, reason.
    pub fn unresolved(&self, relevant: &dyn Fn(&UnresolvedRel) -> bool) -> Vec<Unresolved> {
        let mut out = Vec::new();
        for (file, rels) in &self.relationships {
            let Some(family) = family_of(file) else {
                continue;
            };
            for rel in rels {
                let Outcome::Unresolved(reason) = &rel.outcome else {
                    continue;
                };
                let r = &rel.relationship;
                let entry = UnresolvedRel {
                    file: file.clone(),
                    family,
                    kind: r.kind,
                };
                if relevant(&entry) {
                    out.push(Unresolved {
                        file: file.clone(),
                        range: r.range.clone(),
                        kind: r.kind.as_str().to_string(),
                        target: r.specifier.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| {
            a.file
                .as_bytes()
                .cmp(b.file.as_bytes())
                .then_with(|| range_key(&a.range).cmp(&range_key(&b.range)))
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.target.cmp(&b.target))
                .then_with(|| a.reason.cmp(&b.reason))
        });
        out
    }

    /// The relationships of an analyzed file, in location order.
    pub fn relationships_of(&self, file: &str) -> &[ResolvedRel] {
        self.relationships.get(file).map_or(&[], Vec::as_slice)
    }

    /// `relationships`, `kinds`, `resolution_inputs` and
    /// `resolution_inputs_total`, over every analyzed file.
    pub fn common_details(&self) -> Map<String, Value> {
        let (mut resolved, mut external, mut unresolved) = (0u64, 0u64, 0u64);
        let mut kinds = BTreeSet::new();
        for rel in self.relationships.values().flatten() {
            kinds.insert(rel.relationship.kind.as_str());
            match rel.outcome {
                Outcome::Resolved(_) => resolved += 1,
                Outcome::External => external += 1,
                Outcome::Unresolved(_) => unresolved += 1,
            }
        }
        let inputs: Vec<Value> = self
            .inputs
            .iter()
            .take(MAX_RESOLUTION_INPUTS)
            .map(|(file, (kind, error))| {
                let mut entry = json!({"file": file, "kind": kind.as_str()});
                if let Some(error) = error {
                    entry["error"] = json!(error);
                }
                entry
            })
            .collect();
        let mut details = Map::new();
        details.insert(
            "relationships".to_string(),
            json!({
                "total": resolved + external + unresolved,
                "resolved": resolved,
                "external": external,
                "unresolved": unresolved,
            }),
        );
        details.insert("kinds".to_string(), json!(kinds));
        details.insert("resolution_inputs".to_string(), Value::Array(inputs));
        details.insert(
            "resolution_inputs_total".to_string(),
            json!(self.inputs.len()),
        );
        details
    }
}

/// One edge per (source, target) pair of resolved relationships, without
/// self-relationships.
fn edges(
    relationships: &BTreeMap<String, Vec<ResolvedRel>>,
    analyzed: &BTreeSet<String>,
) -> Vec<Edge> {
    let mut out = Vec::new();
    for (file, rels) in relationships {
        let mut by_target: BTreeMap<&str, Vec<Evidence>> = BTreeMap::new();
        for rel in rels {
            let Outcome::Resolved(targets) = &rel.outcome else {
                continue;
            };
            let targets: BTreeSet<&str> = targets.iter().map(String::as_str).collect();
            for to in targets.into_iter().filter(|to| *to != file) {
                by_target.entry(to).or_default().push(Evidence {
                    range: rel.relationship.range.clone(),
                    kind: rel.relationship.kind,
                    specifier: rel.relationship.specifier.clone(),
                });
            }
        }
        for (to, mut evidence) in by_target {
            evidence.sort_by(|a, b| {
                range_key(&a.range)
                    .cmp(&range_key(&b.range))
                    .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
                    .then_with(|| a.specifier.cmp(&b.specifier))
            });
            let kinds = evidence.iter().map(|e| e.kind).collect();
            let evidence_total = evidence.len() as u32;
            evidence.truncate(MAX_EDGE_EVIDENCE);
            out.push(Edge {
                file: file.clone(),
                to: to.to_string(),
                to_analyzed: analyzed.contains(to),
                kinds,
                evidence,
                evidence_total,
            });
        }
    }
    out
}

/// The first input of `family`, in path order, with an error.
fn first_failed(inputs: &BTreeMap<String, Input>, family: Family) -> Option<&str> {
    inputs
        .iter()
        .find(|(_, input)| input.error.is_some() && input.kind.family() == family)
        .map(|(path, _)| path.as_str())
}

fn family_of(file: &str) -> Option<Family> {
    Grammar::from_path(Path::new(file)).and_then(Family::of)
}

fn range_key(r: &CompactRange) -> (u32, u32, u32, u32) {
    (r.start_line(), r.start_col(), r.end_line(), r.end_col())
}

/// `relative` joined to `base`, both `/`-separated, with empty segments and
/// `.` removed and each `..` applied; `None` when a `..` leaves the root.
fn lexical(base: &str, relative: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for part in base.split('/').chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            name => out.push(name),
        }
    }
    Some(out.join("/"))
}

fn segments(path: &str) -> VecDeque<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// A symlink target's workspace-relative path without the directory slash;
/// `""` for the root.
fn own_path(target: &str) -> &str {
    match target.trim_end_matches('/') {
        "." => "",
        path => path,
    }
}
