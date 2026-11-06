//! Resolution of Rust relationships: the crates of the `Cargo.toml` files,
//! the module tree of each crate root, and the rules for `mod x;`, `use`
//! paths, `extern crate` and the `include!` family.
//!
//! A crate root, a `mod.rs` file and a file loaded through `#[path]` own
//! their directory; any other module file's children live in a directory
//! named after its stem. A file that no tree of the workspace reaches gets
//! a tree of its own in which only `mod.rs` files own their directory.
//!
//! A file is in a tree when the tree's module tree reaches it, through any
//! `#[path]`, whatever package the file's directory is in, and it is
//! resolved in each module that loads it with the package of that tree. So
//! every tree of every workspace crate is read in full once per build, and
//! an index from file to trees makes each later lookup one map access.
//! Each module file is read once per build, however many trees and modules
//! load it, and the files of one level of the trees are read in parallel;
//! only what expansion needs of a file is kept, not its parse.
//! `cfg` is not evaluated, so a name with several `mod` declarations or
//! dependency entries resolves to every file it reaches. A raw identifier
//! `r#name` is the identifier `name` in every probe and comparison.

use super::toml::{first_error, is_key, key_path, toml_pair, unquote};
use super::{Family, InputKind, Outcome, Probe, ResolveCx, Resolver, join, lexical};
use crate::analyze::read::{Parsed, RemovedRegion};
use act_lite_core::CompactRange;
use act_lite_parser::deps::{InlineModule, Relationship, RelationshipKind, extract, module_names};
use act_lite_parser::{Grammar, SourceFile};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;
use tree_sitter::Node;

/// The resolver of the Rust family.
pub struct RustResolver;

/// The crates every Rust program can name without a dependency.
const BUILTIN_CRATES: [&str; 5] = ["alloc", "core", "proc_macro", "std", "test"];

/// Directories whose `Cargo.toml` adds no crate to the workspace.
const TEST_DIRS: [&str; 5] = ["test", "tests", "__tests__", "spec", "testdata"];

const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The directories whose `*.rs` files are example, bench and test roots.
const ROOT_GROUPS: [&str; 3] = ["examples", "benches", "tests"];

/// What is read of one `Cargo.toml`.
#[derive(Default)]
struct Manifest {
    /// The directory of the `Cargo.toml`.
    dir: String,
    /// `[package] name`; `None` for a manifest without `[package]`.
    package: Option<String>,
    lib_name: Option<String>,
    lib_path: Option<String>,
    no_autobins: bool,
    /// The `name` and `path` of each `[[bin]]`.
    bins: Vec<(Option<String>, Option<String>)>,
    /// Every entry of every dependency table, one per table and key.
    dependencies: Vec<Dependency>,
    /// The manifest has a `[workspace]` table.
    workspace: bool,
    workspace_dependencies: Vec<Dependency>,
}

impl Manifest {
    /// `[lib] name`, else `[package] name` with `-` replaced by `_`.
    fn lib_name(&self) -> Option<String> {
        self.lib_name
            .clone()
            .or_else(|| self.package.as_ref().map(|p| p.replace('-', "_")))
    }
}

#[derive(Clone, Default)]
struct Dependency {
    /// The key path of the table that holds the entry.
    table: Vec<String>,
    key: String,
    package: Option<String>,
    path: Option<String>,
    workspace: bool,
}

/// What a `mod` declaration of a module loads.
#[derive(Clone)]
enum Child {
    Module(usize),
    /// Its file was not found.
    Missing,
    /// Its file is outside the workspace.
    Outside,
}

/// A module's declared names and whether it has a glob `use`, shared by
/// every module of the same file.
type Names = Arc<(BTreeSet<String>, bool)>;

/// One module of a tree: a module file or an inline module.
struct Module {
    /// The file that holds the module.
    file: String,
    parent: Option<usize>,
    /// Where the module's `mod x;` declarations are probed.
    dir: String,
    /// The directory a `#[path]` attribute in the module is relative to.
    attr_dir: String,
    /// Each `mod` of the module, by name.
    children: Vec<(String, Child)>,
    /// The inline modules of the module and their ranges.
    inline: Vec<(CompactRange, usize)>,
    /// The declared names and whether a glob `use` is present; the `Err` is
    /// the reason the file could not be read.
    names: Result<Names, String>,
    /// The module's file has been read; always true for an inline module.
    expanded: bool,
}

/// The module tree of one crate root, or of a file that no root reaches.
struct Tree {
    modules: Vec<Module>,
    /// Every module of each module file: a file that several `mod`
    /// declarations load is a module once for each.
    files: HashMap<String, Vec<usize>>,
    detached: bool,
    /// Every module file of the tree has been read.
    complete: bool,
}

/// What a `mod x;` declaration loads.
enum ModTarget {
    /// The file and whether it owns its directory.
    File(String, bool),
    Outside,
    /// The reason no file was found.
    Missing(String),
}

/// Where the first segment of a path names a crate.
enum CrateStart {
    /// The lib roots of the workspace crates it names, in byte order.
    Libs(Vec<String>),
    External,
    Unresolved(String),
}

/// A crate root and whether it is the lib root.
type Root = (String, bool);

#[derive(Default)]
struct RustState {
    /// Each `Cargo.toml` read; the `Err` is the reason it could not be read.
    manifests: HashMap<String, Result<Arc<Manifest>, String>>,
    /// The crates of the workspace, outside test and vendored directories.
    crates: Option<Vec<Arc<Manifest>>>,
    /// The crate roots of each package directory, in root order.
    roots: HashMap<String, Arc<Vec<Root>>>,
    /// The trees by root file and whether the tree is detached.
    trees: HashMap<(String, bool), Tree>,
    /// For each package directory, the positions in its root order of the
    /// trees that reach each file.
    members: HashMap<String, Arc<HashMap<String, Vec<usize>>>>,
    /// For each file, the workspace crates with a tree that reaches it, by
    /// directory in byte order.
    reached: Option<Arc<HashMap<String, Vec<Arc<Manifest>>>>>,
    /// What each module file read gives, shared by every tree and module
    /// that loads it; the `Err` is the reason it could not be read.
    summaries: HashMap<String, Arc<Result<ModuleFile, String>>>,
}

impl Resolver for RustResolver {
    fn family(&self) -> Family {
        Family::Rust
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        let package = match package_of(cx, file) {
            Ok(package) => package,
            Err(reason) => return Outcome::Unresolved(reason),
        };
        if rel.kind == RelationshipKind::Include {
            return include(cx, file, rel);
        }
        // Every tree that reaches the file resolves the relationship, in root
        // order, with the package of the tree: the targets of all trees that
        // resolve it, else `external` when a tree gives it, else the reason
        // of the first tree.
        let mut targets: Option<BTreeSet<String>> = None;
        let mut first = None;
        for (owner, (root, is_lib)) in trees_of(cx, file, package.as_ref()) {
            // Each module of the file in this tree resolves it; the targets
            // are combined as across trees.
            let outcome = with_tree(cx, &root, false, |cx, tree| {
                let instances = tree.files[file].clone();
                let outcomes = instances
                    .into_iter()
                    .map(|start| outcome_in(cx, rel, Some(&owner), tree, Some(is_lib), start))
                    .collect();
                combine(outcomes)
            });
            match outcome {
                Outcome::Resolved(found) => {
                    targets.get_or_insert_default().extend(found);
                }
                Outcome::Unresolved(reason) => {
                    first.get_or_insert(Outcome::Unresolved(reason));
                }
                Outcome::External => first = Some(Outcome::External),
            }
        }
        if let Some(targets) = targets {
            return Outcome::Resolved(targets.into_iter().collect());
        }
        first.unwrap_or_else(|| {
            with_tree(cx, file, true, |cx, tree| {
                expand(cx, tree, 0);
                outcome_in(cx, rel, package.as_deref(), tree, None, 0)
            })
        })
    }
}

/// The package of `file`: the nearest `Cargo.toml` with `[package]`;
/// `None` when there is none, or when the nearest is in a vendored
/// directory. The `Err` is the reason the nearest could not be read.
fn package_of(cx: &mut ResolveCx<'_>, file: &str) -> Result<Option<Arc<Manifest>>, String> {
    let mut at = dir_of(file);
    loop {
        if let Probe::File(path) = cx.probe_file(&join(at, "Cargo.toml")) {
            if cx.vendored(&path) {
                return Ok(None);
            }
            let manifest = read_manifest(cx, &path)?;
            return Ok(manifest.package.is_some().then_some(manifest));
        }
        if at.is_empty() {
            return Ok(None);
        }
        at = dir_of(at);
    }
}

/// The outcome of a `mod`, `use` or `extern crate` relationship whose file
/// is the module `start` of `tree`; `is_lib` tells whether the tree's root
/// is the lib root.
fn outcome_in(
    cx: &mut ResolveCx<'_>,
    rel: &Relationship,
    package: Option<&Manifest>,
    tree: &mut Tree,
    is_lib: Option<bool>,
    start: usize,
) -> Outcome {
    let m = innermost(tree, start, &rel.range);
    match rel.kind {
        RelationshipKind::Module => {
            let target = probe_mod(
                cx,
                &tree.modules[m],
                unraw(&rel.specifier),
                rel.path_attribute.as_deref(),
                tree.detached,
            );
            match target {
                ModTarget::File(found, _) => Outcome::Resolved(vec![found]),
                ModTarget::Outside => Outcome::External,
                ModTarget::Missing(reason) => Outcome::Unresolved(reason),
            }
        }
        RelationshipKind::ExternCrate => {
            match crate_start(cx, package, is_lib, unraw(&rel.specifier)) {
                Some(CrateStart::Libs(roots)) => Outcome::Resolved(roots),
                Some(CrateStart::External) => Outcome::External,
                Some(CrateStart::Unresolved(reason)) => Outcome::Unresolved(reason),
                None => {
                    Outcome::Unresolved(format!("{} is not a crate or dependency", rel.specifier))
                }
            }
        }
        _ => use_path(cx, &rel.specifier, package, tree, is_lib, m),
    }
}

/// Rule 2: the first segment picks the start modules, then `walk` follows
/// the rest from each.
fn use_path(
    cx: &mut ResolveCx<'_>,
    path: &str,
    package: Option<&Manifest>,
    tree: &mut Tree,
    is_lib: Option<bool>,
    m: usize,
) -> Outcome {
    let (global, path) = match path.strip_prefix("::") {
        Some(rest) => (true, rest),
        None => (false, path),
    };
    let segments: Vec<&str> = path.split("::").map(unraw).collect();
    let first = segments[0];
    if !global && matches!(first, "crate" | "self" | "super") {
        if tree.detached {
            return Outcome::Unresolved("file is not reached from a crate root".to_string());
        }
        let (mut k, mut i) = match first {
            "crate" => (0, 1),
            "self" => (m, 1),
            _ => (m, 0),
        };
        if first != "crate" {
            while segments.get(i) == Some(&"super") {
                let Some(parent) = tree.modules[k].parent else {
                    return Outcome::Unresolved("super goes above the crate root".to_string());
                };
                k = parent;
                i += 1;
            }
        }
        return walk(cx, tree, k, &segments, i);
    }
    if !global {
        let children = children(&tree.modules[m], first);
        if !children.is_empty() {
            return follow(cx, tree, &children, &segments, 1);
        }
    }
    match crate_start(cx, package, is_lib, first) {
        Some(CrateStart::Libs(roots)) => {
            // An analyzed lib root's package has all its trees read when
            // that root is resolved, so reading the tree now, in parallel,
            // reads nothing more than the build does.
            for root in &roots {
                if cx.analyzed(root) {
                    complete(cx, &[(root.clone(), true)]);
                }
            }
            let outcomes = roots
                .iter()
                .map(|root| with_tree(cx, root, false, |cx, lib| walk(cx, lib, 0, &segments, 1)))
                .collect();
            combine(outcomes)
        }
        Some(CrateStart::External) => Outcome::External,
        Some(CrateStart::Unresolved(reason)) => Outcome::Unresolved(reason),
        None => {
            let module = &tree.modules[m];
            if !global
                && let Ok(names) = &module.names
                && names.0.contains(first)
            {
                return Outcome::Resolved(vec![module.file.clone()]);
            }
            Outcome::Unresolved(format!(
                "first path segment is not a module, crate or dependency: {first}"
            ))
        }
    }
}

/// From the module `k`, follows `segments[i..]` while they name child
/// modules; a remaining segment must be declared by the deepest module's
/// file, or that file must have a glob `use`.
fn walk(cx: &mut ResolveCx<'_>, tree: &mut Tree, k: usize, segments: &[&str], i: usize) -> Outcome {
    expand(cx, tree, k);
    let module = &tree.modules[k];
    let Some(segment) = segments.get(i) else {
        return Outcome::Resolved(vec![module.file.clone()]);
    };
    let children = children(module, segment);
    if !children.is_empty() {
        return follow(cx, tree, &children, segments, i + 1);
    }
    match &module.names {
        Err(reason) => Outcome::Unresolved(reason.clone()),
        Ok(names) if names.1 || names.0.contains(*segment) => {
            Outcome::Resolved(vec![module.file.clone()])
        }
        Ok(_) => Outcome::Unresolved(format!(
            "{segment} is neither a module nor an item of {}",
            module.file
        )),
    }
}

/// Walks on from each `mod` of one name, `segments[i - 1]`.
fn follow(
    cx: &mut ResolveCx<'_>,
    tree: &mut Tree,
    children: &[Child],
    segments: &[&str],
    i: usize,
) -> Outcome {
    let outcomes = children
        .iter()
        .map(|child| match child {
            Child::Module(idx) => walk(cx, tree, *idx, segments, i),
            Child::Outside => Outcome::External,
            Child::Missing => {
                Outcome::Unresolved(format!("module {} was not found", segments[i - 1]))
            }
        })
        .collect();
    combine(outcomes)
}

/// Every file of the resolved outcomes, in byte order; else external when
/// one is external; else the first reason.
fn combine(outcomes: Vec<Outcome>) -> Outcome {
    let mut files = BTreeSet::new();
    let (mut resolved, mut external, mut first) = (false, false, None);
    for outcome in outcomes {
        match outcome {
            Outcome::Resolved(found) => {
                resolved = true;
                files.extend(found);
            }
            Outcome::External => external = true,
            Outcome::Unresolved(reason) => {
                first.get_or_insert(reason);
            }
        }
    }
    if resolved {
        Outcome::Resolved(files.into_iter().collect())
    } else if external {
        Outcome::External
    } else {
        Outcome::Unresolved(first.unwrap_or_default())
    }
}

/// The `mod` declarations of `module` named `name`.
fn children(module: &Module, name: &str) -> Vec<Child> {
    module
        .children
        .iter()
        .filter(|(child, _)| child == name)
        .map(|(_, child)| child.clone())
        .collect()
}

/// The crate bullets of rule 2 for the name `name`; `None` when the name is
/// no crate the package can name.
fn crate_start(
    cx: &mut ResolveCx<'_>,
    package: Option<&Manifest>,
    is_lib: Option<bool>,
    name: &str,
) -> Option<CrateStart> {
    if let Some(package) = package {
        if is_lib == Some(false)
            && package.lib_name().as_deref() == Some(name)
            && let Some(root) = lib_root(cx, package)
        {
            return Some(CrateStart::Libs(vec![root]));
        }
        let entries: Vec<&Dependency> = package
            .dependencies
            .iter()
            .filter(|d| d.key.replace('-', "_") == name)
            .collect();
        if !entries.is_empty() {
            return Some(dependency_start(cx, package, &entries));
        }
    }
    BUILTIN_CRATES
        .contains(&name)
        .then_some(CrateStart::External)
}

/// Where the entries of one dependency name start: the lib roots of the
/// workspace crates they name, unless two of them share a lib name; else
/// outside the workspace.
fn dependency_start(
    cx: &mut ResolveCx<'_>,
    package: &Manifest,
    entries: &[&Dependency],
) -> CrateStart {
    let mut crates: BTreeMap<String, Arc<Manifest>> = BTreeMap::new();
    let mut first = None;
    for entry in entries {
        match dependency_crates(cx, package, entry) {
            Ok(found) => {
                for manifest in found {
                    crates.insert(manifest.dir.clone(), manifest);
                }
            }
            Err(CrateStart::Unresolved(reason)) => {
                first.get_or_insert(reason);
            }
            Err(_) => {}
        }
    }
    if crates.is_empty() {
        return first.map_or(CrateStart::External, CrateStart::Unresolved);
    }
    let mut by_lib: BTreeMap<String, usize> = BTreeMap::new();
    for manifest in crates.values() {
        *by_lib
            .entry(manifest.lib_name().unwrap_or_default())
            .or_default() += 1;
    }
    if let Some((name, _)) = by_lib.iter().find(|(_, count)| **count > 1) {
        return CrateStart::Unresolved(format!(
            "lib name {name} is declared by more than one crate"
        ));
    }
    let mut roots = Vec::new();
    for manifest in crates.values() {
        match lib_root(cx, manifest) {
            Some(root) => roots.push(root),
            None => {
                first.get_or_insert(format!(
                    "the crate in {} has no lib target",
                    display_dir(&manifest.dir)
                ));
            }
        }
    }
    if roots.is_empty() {
        return CrateStart::Unresolved(first.unwrap_or_default());
    }
    roots.sort();
    CrateStart::Libs(roots)
}

/// The workspace crates that one dependency entry names: the crate at its
/// path, or the crates whose `[package] name` is its package name. The
/// `Err` is `External` or `Unresolved`.
fn dependency_crates(
    cx: &mut ResolveCx<'_>,
    package: &Manifest,
    entry: &Dependency,
) -> Result<Vec<Arc<Manifest>>, CrateStart> {
    let mut entry = entry.clone();
    let mut base = package.dir.clone();
    if entry.workspace
        && let Some(root) = workspace_root(cx, &package.dir)
        && let Some(shared) = root
            .workspace_dependencies
            .iter()
            .find(|d| d.key == entry.key)
    {
        entry.path = entry.path.or_else(|| shared.path.clone());
        entry.package = entry.package.or_else(|| shared.package.clone());
        base = root.dir.clone();
    }
    if let Some(path) = &entry.path {
        if is_absolute(path) {
            return Err(CrateStart::Unresolved(format!(
                "absolute path is not resolved: {path}"
            )));
        }
        let Some(dir) = cx.normalize(&base, path) else {
            return Err(CrateStart::External);
        };
        if cx.vendored(&join(&dir, "Cargo.toml")) {
            return Err(CrateStart::External);
        }
        let dir = match cx.probe_file(&dir) {
            Probe::Dir(dir) => dir,
            Probe::Outside => return Err(CrateStart::External),
            _ => {
                return Err(CrateStart::Unresolved(format!(
                    "path dependency {dir} does not exist"
                )));
            }
        };
        return match read_manifest(cx, &join(&dir, "Cargo.toml")) {
            Ok(manifest) => Ok(vec![manifest]),
            Err(reason) => Err(CrateStart::Unresolved(reason)),
        };
    }
    let name = entry.package.as_ref().unwrap_or(&entry.key);
    let found: Vec<Arc<Manifest>> = crates(cx)
        .into_iter()
        .filter(|c| c.package.as_ref() == Some(name))
        .collect();
    if !found.is_empty() {
        return Ok(found);
    }
    match cx.workspace_skipped().first() {
        Some(skip) => Err(CrateStart::Unresolved(format!(
            "{} could not be read",
            skip.file.file
        ))),
        None => Err(CrateStart::External),
    }
}

/// The nearest manifest with `[workspace]` in `dir` or an ancestor, outside
/// vendored directories.
fn workspace_root(cx: &mut ResolveCx<'_>, dir: &str) -> Option<Arc<Manifest>> {
    let mut at = dir;
    loop {
        if let Probe::File(path) = cx.probe_file(&join(at, "Cargo.toml"))
            && !cx.vendored(&path)
            && let Ok(manifest) = read_manifest(cx, &path)
            && manifest.workspace
        {
            return Some(manifest);
        }
        if at.is_empty() {
            return None;
        }
        at = dir_of(at);
    }
}

/// The crates of the workspace: every `Cargo.toml` with `[package]` outside
/// the test and vendored directories, read once.
fn crates(cx: &mut ResolveCx<'_>) -> Vec<Arc<Manifest>> {
    if let Some(known) = &cx.cache::<RustState>().crates {
        return known.clone();
    }
    let files: Vec<String> = cx
        .workspace_files()
        .iter()
        .filter(|f| f.as_str() == "Cargo.toml" || f.ends_with("/Cargo.toml"))
        .filter(|f| !dir_of(f).split('/').any(|part| TEST_DIRS.contains(&part)))
        .cloned()
        .collect();
    let mut found = Vec::new();
    for file in files {
        if cx.vendored(&file) {
            continue;
        }
        if let Ok(manifest) = read_manifest(cx, &file)
            && manifest.package.is_some()
        {
            found.push(manifest);
        }
    }
    cx.cache::<RustState>().crates = Some(found.clone());
    found
}

/// The lib root: `[lib] path`, else `src/lib.rs`, when it exists.
fn lib_root(cx: &mut ResolveCx<'_>, manifest: &Manifest) -> Option<String> {
    let path = match &manifest.lib_path {
        Some(path) => cx.normalize(&manifest.dir, path)?,
        None => join(&manifest.dir, "src/lib.rs"),
    };
    file_at(cx, &path)
}

/// The crate roots of a package in root order: the lib, the bins, then
/// `build.rs`, examples, benches and tests, each group by path.
fn roots(cx: &mut ResolveCx<'_>, package: &Manifest) -> Arc<Vec<Root>> {
    if let Some(known) = cx.cache::<RustState>().roots.get(&package.dir) {
        return Arc::clone(known);
    }
    let dir = &package.dir;
    let mut roots: Vec<Root> = Vec::new();
    if let Some(lib) = lib_root(cx, package) {
        roots.push((lib, true));
    }
    let mut bins = BTreeSet::new();
    if !package.no_autobins {
        bins.extend(file_at(cx, &join(dir, "src/main.rs")));
        bins.extend(rs_files(cx, &join(dir, "src/bin"), true));
    }
    for (name, path) in &package.bins {
        match (path, name) {
            (Some(path), _) => {
                if let Some(path) = cx.normalize(dir, path) {
                    bins.extend(file_at(cx, &path));
                }
            }
            (None, Some(name)) => {
                bins.extend(file_at(cx, &join(dir, &format!("src/bin/{name}.rs"))));
                bins.extend(file_at(cx, &join(dir, &format!("src/bin/{name}/main.rs"))));
                if package.package.as_ref() == Some(name) {
                    bins.extend(file_at(cx, &join(dir, "src/main.rs")));
                }
            }
            (None, None) => {}
        }
    }
    let mut rest: Vec<Root> = bins.into_iter().map(|bin| (bin, false)).collect();
    rest.extend(file_at(cx, &join(dir, "build.rs")).map(|b| (b, false)));
    for group in ROOT_GROUPS {
        for file in rs_files(cx, &join(dir, group), false) {
            rest.push((file, false));
        }
    }
    for root in rest {
        if !roots.iter().any(|(r, _)| *r == root.0) {
            roots.push(root);
        }
    }
    let roots = Arc::new(roots);
    cx.cache::<RustState>()
        .roots
        .insert(dir.clone(), Arc::clone(&roots));
    roots
}

/// The file at `path`, at its own path, when it exists.
fn file_at(cx: &mut ResolveCx<'_>, path: &str) -> Option<String> {
    match cx.probe_file(path) {
        Probe::File(found) => Some(found),
        _ => None,
    }
}

/// The `*.rs` files of `dir` in byte order, and with `mains` also each
/// `*/main.rs`.
fn rs_files(cx: &mut ResolveCx<'_>, dir: &str, mains: bool) -> Vec<String> {
    let names: Vec<String> = cx
        .list_dir(dir)
        .map(|entries| entries.iter().map(|e| e.name.clone()).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for name in names {
        match cx.probe_file(&join(dir, &name)) {
            Probe::File(found) if name.ends_with(".rs") => out.push(found),
            Probe::Dir(sub) if mains => out.extend(file_at(cx, &join(&sub, "main.rs"))),
            _ => {}
        }
    }
    out
}

/// Runs `f` on the tree from `root`, kept for the build. A detached tree is
/// the tree of a file that no crate root reaches.
fn with_tree<'a, R>(
    cx: &mut ResolveCx<'a>,
    root: &str,
    detached: bool,
    f: impl FnOnce(&mut ResolveCx<'a>, &mut Tree) -> R,
) -> R {
    let key = (root.to_string(), detached);
    let mut tree = cx
        .cache::<RustState>()
        .trees
        .remove(&key)
        .unwrap_or_else(|| {
            let mut tree = Tree {
                modules: Vec::new(),
                files: HashMap::new(),
                detached,
                complete: false,
            };
            add_file(&mut tree, root, None, !detached || is_mod_rs(root));
            tree
        });
    let out = f(cx, &mut tree);
    cx.cache::<RustState>().trees.insert(key, tree);
    out
}

/// The trees that reach `file`, each with its package: those of the file's
/// own package in root order, then those of each other workspace crate, by
/// directory in byte order, each in root order.
fn trees_of(
    cx: &mut ResolveCx<'_>,
    file: &str,
    own: Option<&Arc<Manifest>>,
) -> Vec<(Arc<Manifest>, Root)> {
    let mut packages: Vec<Arc<Manifest>> = own.cloned().into_iter().collect();
    if let Some(others) = reached(cx).get(file) {
        packages.extend(
            others
                .iter()
                .filter(|p| own.is_none_or(|o| o.dir != p.dir))
                .cloned(),
        );
    }
    let mut out = Vec::new();
    for package in packages {
        let roots = roots(cx, &package);
        let members = members(cx, &package, &roots);
        for &at in members.get(file).into_iter().flatten() {
            out.push((Arc::clone(&package), roots[at].clone()));
        }
    }
    out
}

/// For each file, the workspace crates with a tree that reaches it. Every
/// tree of every crate is read in full, the levels of all trees together,
/// so the index is built once per build.
fn reached(cx: &mut ResolveCx<'_>) -> Arc<HashMap<String, Vec<Arc<Manifest>>>> {
    if let Some(known) = &cx.cache::<RustState>().reached {
        return Arc::clone(known);
    }
    let mut packages = crates(cx);
    packages.sort_by(|a, b| a.dir.cmp(&b.dir));
    packages.dedup_by(|a, b| a.dir == b.dir);
    let mut all: Vec<Root> = Vec::new();
    for package in &packages {
        for root in roots(cx, package).iter() {
            if !all.iter().any(|(r, _)| *r == root.0) {
                all.push(root.clone());
            }
        }
    }
    complete(cx, &all);
    let mut index: HashMap<String, Vec<Arc<Manifest>>> = HashMap::new();
    for package in &packages {
        let roots = roots(cx, package);
        for file in members(cx, package, &roots).keys() {
            index
                .entry(file.clone())
                .or_default()
                .push(Arc::clone(package));
        }
    }
    // An analyzed file that no tree reaches gets a tree of its own, which
    // reads that file: read those files together now.
    let detached: Vec<String> = cx
        .analyzed_files(".rs")
        .into_iter()
        .filter(|f| !index.contains_key(f))
        .collect();
    summarize(cx, detached);
    let index = Arc::new(index);
    cx.cache::<RustState>().reached = Some(Arc::clone(&index));
    index
}

/// The trees of `package` that reach each file, by position in `roots`.
/// Each tree is read in full the first time, so the index is built once
/// per package and build.
fn members(
    cx: &mut ResolveCx<'_>,
    package: &Manifest,
    roots: &[Root],
) -> Arc<HashMap<String, Vec<usize>>> {
    if let Some(known) = cx.cache::<RustState>().members.get(&package.dir) {
        return Arc::clone(known);
    }
    complete(cx, roots);
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    for (at, (root, _)) in roots.iter().enumerate() {
        with_tree(cx, root, false, |_, tree| {
            for file in tree.files.keys() {
                index.entry(file.clone()).or_default().push(at);
            }
        });
    }
    let index = Arc::new(index);
    cx.cache::<RustState>()
        .members
        .insert(package.dir.clone(), Arc::clone(&index));
    index
}

/// Reads every module file of the trees of `roots`, one level of the trees
/// at a time: the files of a level are read together, several at once, and
/// each file once. Modules are expanded in the order of their index, as a
/// one-by-one expansion would.
fn complete(cx: &mut ResolveCx<'_>, roots: &[Root]) {
    let mut next: Vec<usize> = vec![0; roots.len()];
    loop {
        let mut files = Vec::new();
        for (root, _) in roots {
            with_tree(cx, root, false, |_, tree| {
                if !tree.complete {
                    files.extend(
                        tree.modules
                            .iter()
                            .filter(|m| !m.expanded)
                            .map(|m| m.file.clone()),
                    );
                }
            });
        }
        if files.is_empty() {
            break;
        }
        summarize(cx, files);
        for (at, (root, _)) in roots.iter().enumerate() {
            with_tree(cx, root, false, |cx, tree| {
                let end = tree.modules.len();
                for idx in next[at]..end {
                    expand(cx, tree, idx);
                }
                next[at] = end;
            });
        }
    }
    for (root, _) in roots {
        with_tree(cx, root, false, |_, tree| tree.complete = true);
    }
}

/// Reads each of `files` that has no summary yet.
fn summarize(cx: &mut ResolveCx<'_>, mut files: Vec<String>) {
    let state = cx.cache::<RustState>();
    files.retain(|f| !state.summaries.contains_key(f));
    files.sort_unstable();
    files.dedup();
    if files.is_empty() {
        return;
    }
    let read = cx.read_each(&files, module_file);
    let state = cx.cache::<RustState>();
    for (file, result) in files.into_iter().zip(read) {
        state.summaries.insert(file, Arc::new(result));
    }
}

/// Reads the file of the module `idx` once: its names, inline modules and
/// `mod` declarations, each probed.
fn expand(cx: &mut ResolveCx<'_>, tree: &mut Tree, idx: usize) {
    if tree.modules[idx].expanded {
        return;
    }
    tree.modules[idx].expanded = true;
    let file = tree.modules[idx].file.clone();
    summarize(cx, vec![file.clone()]);
    let summary = Arc::clone(&cx.cache::<RustState>().summaries[&file]);
    let (names, inline, mods) = match summary.as_ref() {
        Ok(read) => read,
        Err(reason) => {
            tree.modules[idx].names = Err(reason.clone());
            return;
        }
    };
    tree.modules[idx].names = Ok(Arc::clone(names));
    for module in inline {
        // A removed region's inline module can lie inside an inline module
        // of the file.
        let parent = innermost(tree, idx, &module.range);
        add_inline(tree, parent, vec![module.clone()]);
    }
    for (name, range, path_attribute) in mods {
        let name = name.clone();
        let parent = innermost(tree, idx, range);
        let target = probe_mod(
            cx,
            &tree.modules[parent],
            &name,
            path_attribute.as_deref(),
            tree.detached,
        );
        let child = match target {
            ModTarget::File(path, owns) => Child::Module(match ancestor(tree, parent, &path) {
                Some(existing) => existing,
                None => add_file(tree, &path, Some(parent), owns),
            }),
            ModTarget::Outside => Child::Outside,
            ModTarget::Missing(_) => Child::Missing,
        };
        tree.modules[parent].children.push((name, child));
    }
}

/// The module `idx` or its nearest ancestor whose file is `file`: a file
/// that loads itself through `#[path]` declarations is that module again,
/// not a new one, so the tree stays finite.
fn ancestor(tree: &Tree, idx: usize, file: &str) -> Option<usize> {
    let mut at = Some(idx);
    while let Some(i) = at {
        if tree.modules[i].file == file {
            return Some(i);
        }
        at = tree.modules[i].parent;
    }
    None
}

fn add_file(tree: &mut Tree, file: &str, parent: Option<usize>, owns: bool) -> usize {
    let attr_dir = dir_of(file).to_string();
    let dir = if owns {
        attr_dir.clone()
    } else {
        join(&attr_dir, stem(file))
    };
    tree.modules.push(Module {
        file: file.to_string(),
        parent,
        dir,
        attr_dir,
        children: Vec::new(),
        inline: Vec::new(),
        names: Ok(Arc::default()),
        expanded: false,
    });
    let idx = tree.modules.len() - 1;
    tree.files.entry(file.to_string()).or_default().push(idx);
    idx
}

/// Adds the inline modules of the module `idx`, and theirs.
fn add_inline(tree: &mut Tree, idx: usize, inline: Vec<InlineModule>) {
    for module in inline {
        let parent = &tree.modules[idx];
        let written = module.path_attribute.as_deref().unwrap_or(&module.name);
        let dir = lexical(&parent.dir, written).unwrap_or_else(|| join(&parent.dir, written));
        let file = parent.file.clone();
        tree.modules.push(Module {
            file,
            parent: Some(idx),
            attr_dir: dir.clone(),
            dir,
            children: Vec::new(),
            inline: Vec::new(),
            names: Ok(Arc::new((module.names.names, module.names.glob))),
            expanded: true,
        });
        let added = tree.modules.len() - 1;
        tree.modules[idx]
            .children
            .push((module.name.clone(), Child::Module(added)));
        tree.modules[idx].inline.push((module.range, added));
        add_inline(tree, added, module.names.inline);
    }
}

/// A module file's declared names and glob flag, inline modules, and its
/// `mod x;` declarations with their ranges and `#[path]` values, also the
/// inline modules and declarations of the removed test regions.
type ModuleFile = (
    Names,
    Vec<InlineModule>,
    Vec<(String, CompactRange, Option<String>)>,
);

fn module_file(parsed: &Parsed) -> ModuleFile {
    let names = module_names(&parsed.source);
    let mut inline = names.inline;
    let mut mods = module_decls(&parsed.source);
    for region in &parsed.removed {
        let Some(file) = region_source(region) else {
            continue;
        };
        inline.extend(module_names(&file).inline);
        mods.extend(module_decls(&file));
    }
    (Arc::new((names.names, names.glob)), inline, mods)
}

fn module_decls(file: &SourceFile) -> Vec<(String, CompactRange, Option<String>)> {
    extract(file)
        .into_iter()
        .filter(|r| r.kind == RelationshipKind::Module)
        .map(|r| (unraw(&r.specifier).to_string(), r.range, r.path_attribute))
        .collect()
}

/// The identifier that `name` spells: `r#type` is `type`.
fn unraw(name: &str) -> &str {
    name.strip_prefix("r#").unwrap_or(name)
}

/// The text of a removed test region parsed at its own location in the
/// file, so that its ranges are the file's. It is read for the module tree
/// only; none of its relationships is reported.
fn region_source(region: &RemovedRegion) -> Option<SourceFile> {
    let lines = region.range.start_line().saturating_sub(1) as usize;
    let columns = region.range.start_col().saturating_sub(1) as usize;
    let text = format!(
        "{}{}{}",
        "\n".repeat(lines),
        " ".repeat(columns),
        region.text
    );
    SourceFile::from_content("region.rs", text, Grammar::Rust).ok()
}

/// The innermost inline module of the module `idx` that contains `range`.
fn innermost(tree: &Tree, mut idx: usize, range: &CompactRange) -> usize {
    while let Some((_, inner)) = tree.modules[idx]
        .inline
        .iter()
        .find(|(outer, _)| contains(outer, range))
    {
        idx = *inner;
    }
    idx
}

fn contains(outer: &CompactRange, inner: &CompactRange) -> bool {
    (outer.start_line(), outer.start_col()) <= (inner.start_line(), inner.start_col())
        && (inner.end_line(), inner.end_col()) <= (outer.end_line(), outer.end_col())
}

/// Rule 1: `<dir>/x.rs`, then `<dir>/x/mod.rs`, or the `#[path]` value
/// relative to the module's attribute directory.
fn probe_mod(
    cx: &mut ResolveCx<'_>,
    module: &Module,
    name: &str,
    path_attribute: Option<&str>,
    detached: bool,
) -> ModTarget {
    if let Some(written) = path_attribute {
        if is_absolute(written) {
            return ModTarget::Missing(format!("absolute path is not resolved: {written}"));
        }
        let Some(path) = cx.normalize(&module.attr_dir, written) else {
            return ModTarget::Outside;
        };
        return match cx.probe_file(&path) {
            Probe::File(found) => {
                let owns = !detached || is_mod_rs(&found);
                ModTarget::File(found, owns)
            }
            Probe::Outside => ModTarget::Outside,
            _ => ModTarget::Missing(format!("{path} does not exist")),
        };
    }
    let flat = join(&module.dir, &format!("{name}.rs"));
    let nested = join(&module.dir, &format!("{name}/mod.rs"));
    for (path, owns) in [(&flat, false), (&nested, true)] {
        match cx.probe_file(path) {
            Probe::File(found) => return ModTarget::File(found, owns),
            Probe::Outside => return ModTarget::Outside,
            _ => {}
        }
    }
    ModTarget::Missing(format!("neither {flat} nor {nested} exists"))
}

/// Rule 4: the literal relative to the declaring file's directory.
fn include(cx: &mut ResolveCx<'_>, file: &str, rel: &Relationship) -> Outcome {
    if !rel.literal {
        return Outcome::Unresolved("specifier is not a string literal".to_string());
    }
    if is_absolute(&rel.specifier) {
        return Outcome::Unresolved(format!("absolute path is not resolved: {}", rel.specifier));
    }
    let Some(path) = cx.normalize(dir_of(file), &rel.specifier) else {
        return Outcome::External;
    };
    match cx.probe_file(&path) {
        Probe::File(found) => Outcome::Resolved(vec![found]),
        Probe::Outside => Outcome::External,
        _ => Outcome::Unresolved(format!("{path} does not exist")),
    }
}

/// The `Cargo.toml` at `path`, read once; a parse failure is recorded on the
/// input. The `Err` is the reason it could not be read.
fn read_manifest(cx: &mut ResolveCx<'_>, path: &str) -> Result<Arc<Manifest>, String> {
    if let Some(known) = cx.cache::<RustState>().manifests.get(path) {
        return known.clone();
    }
    let result = match cx.input(path, InputKind::CargoToml) {
        Err(reason) => Err(reason),
        Ok(text) => {
            let text = text.to_string();
            parse_manifest(&text).map_err(|error| {
                cx.input_error(path, error);
                format!("{path} could not be read")
            })
        }
    };
    let result = result.map(|mut manifest| {
        manifest.dir = dir_of(path).to_string();
        Arc::new(manifest)
    });
    cx.cache::<RustState>()
        .manifests
        .insert(path.to_string(), result.clone());
    result
}

/// A key path of the manifest and the value at it; `element` numbers the
/// `[[…]]` table the key is in.
struct Entry<'t> {
    path: Vec<String>,
    element: Option<usize>,
    value: Option<Node<'t>>,
}

/// `[package]`, `[lib]`, `[[bin]]`, the dependency tables and their
/// `[target.*]` forms, and `[workspace]`. The `Err` is the parse error.
fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let file = SourceFile::from_content("Cargo.toml", text.to_string(), Grammar::Toml)
        .map_err(|err| err.to_string())?;
    let root = file.tree().root_node();
    if let Some(error) = first_error(root) {
        let at = error.start_position();
        return Err(format!(
            "not valid TOML at {}:{}",
            at.row + 1,
            at.column + 1
        ));
    }
    let mut entries = Vec::new();
    let mut elements = 0;
    let mut cursor = root.walk();
    for item in root.named_children(&mut cursor) {
        match item.kind() {
            "pair" => toml_pair(item, &[], text, &mut |path, value| {
                entries.push(Entry {
                    path,
                    element: None,
                    value: Some(value),
                })
            }),
            "table" | "table_array_element" => {
                let element = (item.kind() == "table_array_element").then(|| {
                    elements += 1;
                    elements
                });
                let mut cursor = item.walk();
                let mut prefix: Option<Vec<String>> = None;
                for child in item.named_children(&mut cursor) {
                    match (&prefix, child.kind()) {
                        (None, _) if is_key(child) => {
                            let path = key_path(child, text);
                            entries.push(Entry {
                                path: path.clone(),
                                element,
                                value: None,
                            });
                            prefix = Some(path);
                        }
                        (Some(prefix), "pair") => {
                            toml_pair(child, prefix, text, &mut |path, value| {
                                entries.push(Entry {
                                    path,
                                    element,
                                    value: Some(value),
                                })
                            })
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let mut manifest = Manifest::default();
    let mut has_package = false;
    let mut bins: BTreeMap<usize, (Option<String>, Option<String>)> = BTreeMap::new();
    for entry in &entries {
        let string = entry.value.and_then(|v| string_value(v, text));
        let path: Vec<&str> = entry.path.iter().map(String::as_str).collect();
        match path.as_slice() {
            ["package", rest @ ..] => {
                has_package = true;
                match rest {
                    ["name"] => manifest.package = string,
                    ["autobins"] => {
                        manifest.no_autobins = entry
                            .value
                            .is_some_and(|v| &text[v.byte_range()] == "false")
                    }
                    _ => {}
                }
            }
            ["lib", "name"] => manifest.lib_name = string,
            ["lib", "path"] => manifest.lib_path = string,
            ["bin", rest @ ..] => {
                let bin = bins.entry(entry.element.unwrap_or(0)).or_default();
                match rest {
                    ["name"] => bin.0 = string,
                    ["path"] => bin.1 = string,
                    _ => {}
                }
            }
            [table, key, rest @ ..] if DEPENDENCY_TABLES.contains(table) => dependency(
                &mut manifest.dependencies,
                &path[..1],
                key,
                rest,
                entry,
                text,
            ),
            ["target", _, table, key, rest @ ..] if DEPENDENCY_TABLES.contains(table) => {
                dependency(
                    &mut manifest.dependencies,
                    &path[..3],
                    key,
                    rest,
                    entry,
                    text,
                )
            }
            ["workspace", rest @ ..] => {
                manifest.workspace = true;
                if let ["dependencies", key, rest @ ..] = rest {
                    dependency(
                        &mut manifest.workspace_dependencies,
                        &path[..2],
                        key,
                        rest,
                        entry,
                        text,
                    );
                }
            }
            _ => {}
        }
    }
    if has_package && manifest.package.is_none() {
        return Err("[package] has no name".to_string());
    }
    manifest.bins = bins.into_values().collect();
    Ok(manifest)
}

/// Records the part `rest` of the entry `key` of the dependency table
/// `table`.
fn dependency(
    list: &mut Vec<Dependency>,
    table: &[&str],
    key: &str,
    rest: &[&str],
    entry: &Entry,
    text: &str,
) {
    let idx = match list.iter().position(|d| d.table == table && d.key == key) {
        Some(idx) => idx,
        None => {
            list.push(Dependency {
                table: table.iter().map(|t| t.to_string()).collect(),
                key: key.to_string(),
                ..Dependency::default()
            });
            list.len() - 1
        }
    };
    let dependency = &mut list[idx];
    let value = entry.value;
    match rest {
        ["path"] => dependency.path = value.and_then(|v| string_value(v, text)),
        ["package"] => dependency.package = value.and_then(|v| string_value(v, text)),
        ["workspace"] => {
            dependency.workspace = value.is_some_and(|v| &text[v.byte_range()] == "true")
        }
        _ => {}
    }
}

fn string_value(node: Node, text: &str) -> Option<String> {
    (node.kind() == "string").then(|| unquote(&text[node.byte_range()]))
}

/// Whether a path as written is absolute.
fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || Path::new(path).is_absolute()
}

/// The directory of a workspace-relative path; `""` for the root.
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// A directory as written in a reason: `.` for the root.
fn display_dir(dir: &str) -> &str {
    if dir.is_empty() { "." } else { dir }
}

fn stem(path: &str) -> &str {
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    name.strip_suffix(".rs").unwrap_or(name)
}

fn is_mod_rs(path: &str) -> bool {
    path == "mod.rs" || path.ends_with("/mod.rs")
}
