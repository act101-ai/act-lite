//! Resolution of Rust relationships: the crates of the `Cargo.toml` files,
//! the module tree of each crate root, and the rules for `mod x;`, `use`
//! paths, `extern crate` and the `include!` family.
//!
//! A crate root, a `mod.rs` file and a file loaded through `#[path]` own
//! their directory; any other module file's children live in a directory
//! named after its stem. A file that no tree of its package reaches gets a
//! tree of its own in which only `mod.rs` files own their directory.

use super::{Family, InputKind, Outcome, Probe, ResolveCx, Resolver, join, lexical};
use act_lite_core::CompactRange;
use act_lite_parser::deps::{
    InlineModule, Relationship, RelationshipKind, extract, mods_in_text, module_names,
};
use act_lite_parser::{Grammar, SourceFile};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use tree_sitter::Node;

/// The resolver of the Rust family.
pub struct RustResolver;

/// The crates every Rust program can name without a dependency.
const BUILTIN_CRATES: [&str; 5] = ["alloc", "core", "proc_macro", "std", "test"];

/// Directories whose `Cargo.toml` adds no crate to the workspace.
const TEST_DIRS: [&str; 5] = ["test", "tests", "__tests__", "spec", "testdata"];

const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

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
    key: String,
    package: Option<String>,
    path: Option<String>,
    workspace: bool,
}

/// One module of a tree: a module file or an inline module.
struct Module {
    /// The file that holds the module.
    file: String,
    parent: Option<usize>,
    /// Where the module's `mod x;` declarations are probed.
    dir: String,
    /// The directory a `#[path]` attribute in the module is relative to.
    attr_dir: String,
    /// Each `mod` of the module by name: the child, or `None` when its file
    /// was not found.
    children: Vec<(String, Option<usize>)>,
    /// The inline modules of the module and their ranges.
    inline: Vec<(CompactRange, usize)>,
    /// The declared names and whether a glob `use` is present; the `Err` is
    /// the reason the file could not be read.
    names: Result<(BTreeSet<String>, bool), String>,
}

/// The module tree of one crate root, or of a file that no root reaches.
struct Tree {
    modules: Vec<Module>,
    /// The first module of each module file.
    files: HashMap<String, usize>,
    detached: bool,
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
    /// The lib root of a workspace crate.
    Lib(String),
    External,
    Unresolved(String),
}

#[derive(Default)]
struct RustState {
    /// Each `Cargo.toml` read; the `Err` is the reason it could not be read.
    manifests: HashMap<String, Result<Arc<Manifest>, String>>,
    /// The crates of the workspace, outside test directories.
    crates: Option<Vec<Arc<Manifest>>>,
    /// The crate roots of each package directory, in root order, each with
    /// whether it is the lib root.
    roots: HashMap<String, Arc<Vec<(String, bool)>>>,
    /// The trees by root file and whether the tree is detached.
    trees: HashMap<(String, bool), Arc<Tree>>,
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
        let mut first = None;
        if let Some(package) = &package {
            let roots = roots(cx, package);
            for (root, is_lib) in roots.iter() {
                let tree = tree(cx, root, false);
                if !tree.files.contains_key(file) {
                    continue;
                }
                let outcome = outcome_in(cx, file, rel, Some(package), &tree, Some(*is_lib));
                if !matches!(outcome, Outcome::Unresolved(_)) {
                    return outcome;
                }
                first.get_or_insert(outcome);
            }
        }
        first.unwrap_or_else(|| {
            let tree = tree(cx, file, true);
            outcome_in(cx, file, rel, package.as_deref(), &tree, None)
        })
    }
}

/// The package of `file`: the nearest `Cargo.toml` with `[package]`;
/// `None` when there is none. The `Err` is the reason the nearest
/// `Cargo.toml` could not be read.
fn package_of(cx: &mut ResolveCx<'_>, file: &str) -> Result<Option<Arc<Manifest>>, String> {
    let mut at = dir_of(file);
    loop {
        if let Probe::File(path) = cx.probe_file(&join(at, "Cargo.toml")) {
            let manifest = read_manifest(cx, &path)?;
            return Ok(manifest.package.is_some().then_some(manifest));
        }
        if at.is_empty() {
            return Ok(None);
        }
        at = dir_of(at);
    }
}

/// The outcome of a `mod`, `use` or `extern crate` relationship of `file`
/// in `tree`; `is_lib` tells whether the tree's root is the lib root.
fn outcome_in(
    cx: &mut ResolveCx<'_>,
    file: &str,
    rel: &Relationship,
    package: Option<&Manifest>,
    tree: &Arc<Tree>,
    is_lib: Option<bool>,
) -> Outcome {
    let m = innermost(tree, tree.files[file], &rel.range);
    match rel.kind {
        RelationshipKind::Module => {
            let module = &tree.modules[m];
            match probe_mod(
                cx,
                module,
                &rel.specifier,
                rel.path_attribute.as_deref(),
                tree.detached,
            ) {
                ModTarget::File(found, _) => Outcome::Resolved(vec![found]),
                ModTarget::Outside => Outcome::External,
                ModTarget::Missing(reason) => Outcome::Unresolved(reason),
            }
        }
        RelationshipKind::ExternCrate => match crate_start(cx, package, is_lib, &rel.specifier) {
            Some(CrateStart::Lib(root)) => Outcome::Resolved(vec![root]),
            Some(CrateStart::External) => Outcome::External,
            Some(CrateStart::Unresolved(reason)) => Outcome::Unresolved(reason),
            None => Outcome::Unresolved(format!("{} is not a crate or dependency", rel.specifier)),
        },
        _ => use_path(cx, &rel.specifier, package, tree, is_lib, m),
    }
}

/// Rule 2: the first segment picks the start module, each next segment
/// that names a child module is followed, and a remaining segment must be
/// declared by the deepest module's file.
fn use_path(
    cx: &mut ResolveCx<'_>,
    path: &str,
    package: Option<&Manifest>,
    tree: &Arc<Tree>,
    is_lib: Option<bool>,
    m: usize,
) -> Outcome {
    let (global, path) = match path.strip_prefix("::") {
        Some(rest) => (true, rest),
        None => (false, path),
    };
    let segments: Vec<&str> = path.split("::").collect();
    let first = segments[0];
    let mut tree = Arc::clone(tree);
    let mut k = m;
    let mut i = 1;
    if !global && matches!(first, "crate" | "self" | "super") {
        if tree.detached {
            return Outcome::Unresolved("file is not reached from a crate root".to_string());
        }
        match first {
            "crate" => k = 0,
            "super" => i = 0,
            _ => {}
        }
        if first != "crate" {
            while segments.get(i) == Some(&"super") {
                let Some(parent) = tree.modules[k].parent else {
                    return Outcome::Unresolved("super goes above the crate root".to_string());
                };
                k = parent;
                i += 1;
            }
        }
    } else {
        match child(&tree.modules[m], first) {
            Some(Some(found)) if !global => k = found,
            Some(None) if !global => {
                return Outcome::Unresolved(format!("module {first} was not found"));
            }
            _ => match crate_start(cx, package, is_lib, first) {
                Some(CrateStart::Lib(root)) => {
                    tree = self::tree(cx, &root, false);
                    k = 0;
                }
                Some(CrateStart::External) => return Outcome::External,
                Some(CrateStart::Unresolved(reason)) => return Outcome::Unresolved(reason),
                None => {
                    let module = &tree.modules[m];
                    if !global
                        && let Ok((names, _)) = &module.names
                        && names.contains(first)
                    {
                        return Outcome::Resolved(vec![module.file.clone()]);
                    }
                    return Outcome::Unresolved(format!(
                        "first path segment is not a module, crate or dependency: {first}"
                    ));
                }
            },
        }
    }
    while let Some(segment) = segments.get(i) {
        match child(&tree.modules[k], segment) {
            Some(Some(found)) => k = found,
            Some(None) => return Outcome::Unresolved(format!("module {segment} was not found")),
            None => break,
        }
        i += 1;
    }
    let module = &tree.modules[k];
    let Some(segment) = segments.get(i) else {
        return Outcome::Resolved(vec![module.file.clone()]);
    };
    match &module.names {
        Err(reason) => Outcome::Unresolved(reason.clone()),
        Ok((names, glob)) if *glob || names.contains(*segment) => {
            Outcome::Resolved(vec![module.file.clone()])
        }
        Ok(_) => Outcome::Unresolved(format!(
            "{segment} is neither a module nor an item of {}",
            module.file
        )),
    }
}

/// The child module `name` of `module`: `Some(None)` when every `mod` of
/// that name is unfound, `None` when there is no such `mod`.
fn child(module: &Module, name: &str) -> Option<Option<usize>> {
    let mut found = None;
    for (child, idx) in &module.children {
        if child == name {
            if idx.is_some() {
                return Some(*idx);
            }
            found = Some(None);
        }
    }
    found
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
            return Some(CrateStart::Lib(root));
        }
        if let Some(dependency) = package
            .dependencies
            .iter()
            .find(|d| d.key.replace('-', "_") == name)
        {
            return Some(dependency_start(cx, package, dependency));
        }
    }
    BUILTIN_CRATES
        .contains(&name)
        .then_some(CrateStart::External)
}

/// Where a dependency of `package` starts: the lib root of the crate at its
/// path, or of the one workspace crate with its package name; else outside.
fn dependency_start(
    cx: &mut ResolveCx<'_>,
    package: &Manifest,
    dependency: &Dependency,
) -> CrateStart {
    let mut dependency = dependency.clone();
    let mut base = package.dir.clone();
    if dependency.workspace
        && let Some(root) = workspace_root(cx, &package.dir)
        && let Some(entry) = root
            .workspace_dependencies
            .iter()
            .find(|d| d.key == dependency.key)
    {
        dependency.path = dependency.path.or_else(|| entry.path.clone());
        dependency.package = dependency.package.or_else(|| entry.package.clone());
        base = root.dir.clone();
    }
    if let Some(path) = &dependency.path {
        let Some(dir) = cx.normalize(&base, path) else {
            return CrateStart::External;
        };
        let dir = match cx.probe_file(&dir) {
            Probe::Dir(dir) => dir,
            Probe::Outside => return CrateStart::External,
            _ => return CrateStart::Unresolved(format!("path dependency {dir} does not exist")),
        };
        return match read_manifest(cx, &join(&dir, "Cargo.toml")) {
            Ok(manifest) => lib_start(cx, &manifest),
            Err(reason) => CrateStart::Unresolved(reason),
        };
    }
    let name = dependency.package.as_ref().unwrap_or(&dependency.key);
    let candidates: Vec<Arc<Manifest>> = crates(cx)
        .into_iter()
        .filter(|c| c.package.as_ref() == Some(name))
        .collect();
    match candidates.as_slice() {
        [] => match cx.workspace_skipped().first() {
            Some(skip) => CrateStart::Unresolved(format!("{} could not be read", skip.file.file)),
            None => CrateStart::External,
        },
        [only] => lib_start(cx, only),
        [first, ..] => CrateStart::Unresolved(format!(
            "lib name {} is declared by more than one crate",
            first.lib_name().unwrap_or_default()
        )),
    }
}

fn lib_start(cx: &mut ResolveCx<'_>, manifest: &Manifest) -> CrateStart {
    match lib_root(cx, manifest) {
        Some(root) => CrateStart::Lib(root),
        None => CrateStart::Unresolved(format!(
            "the crate in {} has no lib target",
            display_dir(&manifest.dir)
        )),
    }
}

/// The nearest manifest with `[workspace]` in `dir` or an ancestor.
fn workspace_root(cx: &mut ResolveCx<'_>, dir: &str) -> Option<Arc<Manifest>> {
    let mut at = dir;
    loop {
        if let Probe::File(path) = cx.probe_file(&join(at, "Cargo.toml"))
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
/// the test directories, read once.
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
    match cx.probe_file(&path) {
        Probe::File(found) => Some(found),
        _ => None,
    }
}

/// The crate roots of a package in root order: the lib, the bins, then
/// `build.rs`, examples, benches and tests, each group by path.
fn roots(cx: &mut ResolveCx<'_>, package: &Manifest) -> Arc<Vec<(String, bool)>> {
    if let Some(known) = cx.cache::<RustState>().roots.get(&package.dir) {
        return Arc::clone(known);
    }
    let dir = &package.dir;
    let mut roots = Vec::new();
    if let Some(lib) = lib_root(cx, package) {
        roots.push((lib, true));
    }
    let mut bins = BTreeSet::new();
    if !package.no_autobins {
        bins.extend(files(cx, &join(dir, "src/main.rs")));
        bins.extend(rs_files(cx, &join(dir, "src/bin"), true));
    }
    for (name, path) in &package.bins {
        match (path, name) {
            (Some(path), _) => {
                if let Some(path) = cx.normalize(dir, path) {
                    bins.extend(files(cx, &path));
                }
            }
            (None, Some(name)) => {
                bins.extend(files(cx, &join(dir, &format!("src/bin/{name}.rs"))));
                bins.extend(files(cx, &join(dir, &format!("src/bin/{name}/main.rs"))));
                if package.package.as_ref() == Some(name) {
                    bins.extend(files(cx, &join(dir, "src/main.rs")));
                }
            }
            (None, None) => {}
        }
    }
    let mut rest: Vec<String> = bins.into_iter().collect();
    rest.extend(files(cx, &join(dir, "build.rs")));
    for group in ["examples", "benches", "tests"] {
        rest.extend(rs_files(cx, &join(dir, group), false));
    }
    for root in rest {
        if !roots.iter().any(|(r, _)| *r == root) {
            roots.push((root, false));
        }
    }
    let roots = Arc::new(roots);
    cx.cache::<RustState>()
        .roots
        .insert(dir.clone(), Arc::clone(&roots));
    roots
}

/// The file at `path`, at its own path, when it exists.
fn files(cx: &mut ResolveCx<'_>, path: &str) -> Option<String> {
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
            Probe::Dir(sub) if mains => out.extend(files(cx, &join(&sub, "main.rs"))),
            _ => {}
        }
    }
    out
}

/// The module tree from `root`, built once. A detached tree is the tree of
/// a file that no crate root reaches.
fn tree(cx: &mut ResolveCx<'_>, root: &str, detached: bool) -> Arc<Tree> {
    let key = (root.to_string(), detached);
    if let Some(known) = cx.cache::<RustState>().trees.get(&key) {
        return Arc::clone(known);
    }
    let mut tree = Tree {
        modules: Vec::new(),
        files: HashMap::new(),
        detached,
    };
    let owns = !detached || is_mod_rs(root);
    add_file(&mut tree, root, None, owns);
    let mut queue = VecDeque::from([0]);
    while let Some(idx) = queue.pop_front() {
        let file = tree.modules[idx].file.clone();
        let (mods, inline) = match read_module(cx, &file) {
            Ok((names, glob, inline, mods)) => {
                tree.modules[idx].names = Ok((names, glob));
                (mods, inline)
            }
            Err(reason) => {
                tree.modules[idx].names = Err(reason);
                continue;
            }
        };
        add_inline(&mut tree, idx, inline);
        for (name, range, path_attribute) in mods {
            let parent = innermost(&tree, idx, &range);
            let target = probe_mod(
                cx,
                &tree.modules[parent],
                &name,
                path_attribute.as_deref(),
                detached,
            );
            let found = match target {
                ModTarget::File(path, owns) => Some(match tree.files.get(&path) {
                    Some(&existing) => existing,
                    None => {
                        let added = add_file(&mut tree, &path, Some(parent), owns);
                        queue.push_back(added);
                        added
                    }
                }),
                _ => None,
            };
            tree.modules[parent].children.push((name, found));
        }
    }
    let tree = Arc::new(tree);
    cx.cache::<RustState>().trees.insert(key, Arc::clone(&tree));
    tree
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
        names: Ok((BTreeSet::new(), false)),
    });
    let idx = tree.modules.len() - 1;
    tree.files.entry(file.to_string()).or_insert(idx);
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
            names: Ok((module.names.names, module.names.glob)),
        });
        let added = tree.modules.len() - 1;
        tree.modules[idx]
            .children
            .push((module.name.clone(), Some(added)));
        tree.modules[idx].inline.push((module.range, added));
        add_inline(tree, added, module.names.inline);
    }
}

/// A module file's declared names, glob flag, inline modules, and its
/// `mod x;` declarations with their ranges and `#[path]` values, also
/// those of the removed test regions.
type ModuleFile = (
    BTreeSet<String>,
    bool,
    Vec<InlineModule>,
    Vec<(String, CompactRange, Option<String>)>,
);

fn read_module(cx: &mut ResolveCx<'_>, file: &str) -> Result<ModuleFile, String> {
    let parsed = cx.source(file)?;
    let mut mods: Vec<(String, CompactRange, Option<String>)> = extract(&parsed.source)
        .into_iter()
        .filter(|r| r.kind == RelationshipKind::Module)
        .map(|r| (r.specifier, r.range, r.path_attribute))
        .collect();
    for region in &parsed.removed {
        for name in mods_in_text(&region.text) {
            mods.push((name, region.range.clone(), None));
        }
    }
    let names = module_names(&parsed.source);
    Ok((names.names, names.glob, names.inline, mods))
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
            "pair" => toml_pair(item, &[], None, text, &mut entries),
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
                            toml_pair(child, prefix, element, text, &mut entries)
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
            [table, key, rest @ ..] if DEPENDENCY_TABLES.contains(table) => {
                dependency(&mut manifest.dependencies, key, rest, entry, text)
            }
            ["target", _, table, key, rest @ ..] if DEPENDENCY_TABLES.contains(table) => {
                dependency(&mut manifest.dependencies, key, rest, entry, text)
            }
            ["workspace", rest @ ..] => {
                manifest.workspace = true;
                if let ["dependencies", key, rest @ ..] = rest {
                    dependency(&mut manifest.workspace_dependencies, key, rest, entry, text);
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

/// Records the part `rest` of the dependency `key`.
fn dependency(list: &mut Vec<Dependency>, key: &str, rest: &[&str], entry: &Entry, text: &str) {
    let idx = match list.iter().position(|d| d.key == key) {
        Some(idx) => idx,
        None => {
            list.push(Dependency {
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

/// Each pair below `prefix`, also those of inline tables, with its full key.
fn toml_pair<'t>(
    pair: Node<'t>,
    prefix: &[String],
    element: Option<usize>,
    text: &str,
    out: &mut Vec<Entry<'t>>,
) {
    let mut cursor = pair.walk();
    let children: Vec<Node> = pair.named_children(&mut cursor).collect();
    let (Some(key), Some(value)) = (children.first(), children.last()) else {
        return;
    };
    let mut path = prefix.to_vec();
    path.extend(key_path(*key, text));
    if value.kind() == "inline_table" {
        let mut cursor = value.walk();
        for inner in value.named_children(&mut cursor) {
            if inner.kind() == "pair" {
                toml_pair(inner, &path, element, text, out);
            }
        }
    }
    out.push(Entry {
        path,
        element,
        value: Some(*value),
    });
}

fn is_key(node: Node) -> bool {
    matches!(node.kind(), "bare_key" | "quoted_key" | "dotted_key")
}

/// The segments of a key.
fn key_path(key: Node, text: &str) -> Vec<String> {
    if key.kind() == "dotted_key" {
        let mut cursor = key.walk();
        return key
            .named_children(&mut cursor)
            .filter(|k| is_key(*k))
            .flat_map(|k| key_path(k, text))
            .collect();
    }
    vec![unquote(&text[key.byte_range()]).to_string()]
}

fn string_value(node: Node, text: &str) -> Option<String> {
    (node.kind() == "string").then(|| unquote(&text[node.byte_range()]).to_string())
}

/// A TOML string or quoted key without its quotes.
fn unquote(literal: &str) -> &str {
    for quote in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(inner) = literal
            .strip_prefix(quote)
            .and_then(|l| l.strip_suffix(quote))
        {
            return inner;
        }
    }
    literal
}

/// The first `ERROR` or `MISSING` node of the tree.
fn first_error(node: Node) -> Option<Node> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    if !node.has_error() {
        return None;
    }
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    children.into_iter().find_map(first_error)
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
