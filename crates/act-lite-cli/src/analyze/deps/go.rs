//! Resolution of Go relationships: the module map from the `go.mod` files of
//! the workspace, the candidate modules of an import path and the package
//! files of the one valid candidate.

use super::{Family, InputKind, Outcome, Probe, ResolveCx, Resolver, join};
use crate::analyze::read::EntryKind;
use act_lite_parser::deps::Relationship;
use std::collections::HashMap;
use std::sync::Arc;

/// The resolver of the Go family.
pub struct GoResolver;

/// Directories whose `go.mod` adds no module path to the map.
const TEST_DIRS: [&str; 5] = ["test", "tests", "__tests__", "spec", "testdata"];

/// Directory names that mark vendored dependencies, as discovery names them.
const VENDORED_DIRS: [&str; 7] = [
    "vendor",
    "third_party",
    "third-party",
    "node_modules",
    "bower_components",
    "Pods",
    "Carthage",
];

/// What is read of one `go.mod`.
struct GoMod {
    module: String,
    /// The module path and the workspace directory of each `replace` with a
    /// relative path inside the workspace.
    replaces: Vec<(String, String)>,
}

#[derive(Default)]
struct GoState {
    built: bool,
    /// The module path and directory of each `go.mod` of the map.
    modules: Arc<Vec<(String, String)>>,
    /// Each `go.mod` read: `None` when it could not be read or parsed.
    mods: HashMap<String, Option<Arc<GoMod>>>,
}

impl Resolver for GoResolver {
    fn family(&self) -> Family {
        Family::Go
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        let import = rel.specifier.as_str();
        if import == "C" {
            return Outcome::External;
        }
        if !import
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        {
            return Outcome::Unresolved(format!("invalid import path: {import}"));
        }
        build_map(cx);
        let own_dir = nearest_module(cx, dir_of(file));
        let own_path = own_dir.as_deref().map(|dir| join(dir, "go.mod"));
        let own = own_path.as_deref().and_then(|path| read_mod(cx, path));
        let modules = Arc::clone(&cx.cache::<GoState>().modules);
        let replaces = own.iter().flat_map(|m| m.replaces.iter());
        let mut candidates: Vec<(&str, &str)> = Vec::new();
        for (path, dir) in modules.iter().chain(replaces) {
            let under = import == path
                || import
                    .strip_prefix(path.as_str())
                    .is_some_and(|rest| rest.starts_with('/'));
            if under && !candidates.contains(&(path, dir)) {
                candidates.push((path, dir));
            }
        }
        let mut valid: Vec<(&str, Vec<String>)> = Vec::new();
        let mut outside = false;
        let mut unvendored = false;
        for &(path, dir) in &candidates {
            let rest = import[path.len()..].trim_start_matches('/');
            let package = cx.normalize(dir, rest);
            if package.as_deref().is_some_and(vendored) {
                continue;
            }
            unvendored = true;
            let Some(package) = package else {
                continue;
            };
            let package = match cx.probe_file(&package) {
                Probe::Dir(package) => package,
                Probe::Outside => {
                    outside = true;
                    continue;
                }
                _ => continue,
            };
            if ignored(&package)
                || vendored(&package)
                || nearest_module(cx, &package).as_deref() != Some(dir)
            {
                continue;
            }
            let files = package_files(cx, &package);
            if !files.is_empty() {
                valid.push((path, files));
            }
        }
        match valid.len() {
            1 => Outcome::Resolved(valid.remove(0).1),
            0 if !unvendored => {
                let standard = import
                    .split('/')
                    .next()
                    .is_some_and(|first| !first.contains('.'));
                if standard {
                    Outcome::External
                } else if own_dir.is_none() {
                    Outcome::Unresolved("file is under no go.mod in the workspace".to_string())
                } else if let Some(skip) = cx.workspace_skipped().first() {
                    Outcome::Unresolved(format!("{} could not be read", skip.file.file))
                } else {
                    Outcome::External
                }
            }
            0 if outside => Outcome::External,
            0 => match own_path {
                Some(path) if own.is_none() => {
                    Outcome::Unresolved(format!("{path} could not be read"))
                }
                _ => Outcome::Unresolved(format!(
                    "no Go package files for {import} in the workspace"
                )),
            },
            _ => {
                let mut paths: Vec<&str> = valid.iter().map(|(path, _)| *path).collect();
                paths.sort_unstable();
                paths.dedup();
                Outcome::Unresolved(format!("ambiguous import: {}", paths.join(", ")))
            }
        }
    }
}

fn dir_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Whether the go command ignores the directory `dir`.
fn ignored(dir: &str) -> bool {
    dir.split('/')
        .any(|part| part == "testdata" || part.starts_with(['_', '.']))
}

/// Whether `dir` is a vendored directory or inside one.
fn vendored(dir: &str) -> bool {
    dir.split('/').any(|part| VENDORED_DIRS.contains(&part))
}

/// Reads every `go.mod` of the workspace that can add a module path, once.
fn build_map(cx: &mut ResolveCx<'_>) {
    if cx.cache::<GoState>().built {
        return;
    }
    let files: Vec<String> = cx
        .workspace_files()
        .iter()
        .filter(|f| f.as_str() == "go.mod" || f.ends_with("/go.mod"))
        .cloned()
        .collect();
    let mut modules = Vec::new();
    for file in files {
        let dir = dir_of(&file);
        if ignored(dir) || vendored(dir) || dir.split('/').any(|part| TEST_DIRS.contains(&part)) {
            continue;
        }
        if let Some(m) = read_mod(cx, &file) {
            modules.push((m.module.clone(), dir.to_string()));
        }
    }
    let state = cx.cache::<GoState>();
    state.modules = Arc::new(modules);
    state.built = true;
}

/// The directory of the nearest `go.mod` in `dir` or an ancestor that the go
/// command reads; a vendored `go.mod` is never read.
fn nearest_module(cx: &mut ResolveCx<'_>, dir: &str) -> Option<String> {
    let mut at = dir;
    loop {
        if !ignored(at)
            && !vendored(at)
            && matches!(cx.probe_file(&join(at, "go.mod")), Probe::File(_))
        {
            return Some(at.to_string());
        }
        if at.is_empty() {
            return None;
        }
        at = dir_of(at);
    }
}

/// The `go.mod` at `rel`, read once; a failure is recorded on the input.
fn read_mod(cx: &mut ResolveCx<'_>, rel: &str) -> Option<Arc<GoMod>> {
    if let Some(known) = cx.cache::<GoState>().mods.get(rel) {
        return known.clone();
    }
    let parsed = match cx.input(rel, InputKind::GoMod) {
        Ok(text) => parse(text),
        Err(_) => None,
    };
    let found = parsed.map(|(module, replaces)| {
        let dir = dir_of(rel);
        let replaces = replaces
            .into_iter()
            .filter_map(|(path, target)| Some((path, cx.normalize(dir, &target)?)))
            .collect();
        Arc::new(GoMod { module, replaces })
    });
    if found.is_none() {
        cx.input_error(rel, "no module directive".to_string());
    }
    cx.cache::<GoState>()
        .mods
        .insert(rel.to_string(), found.clone());
    found
}

/// The `module` path and the `replace` directives with a relative path of a
/// `go.mod`; `None` without a `module` directive.
fn parse(text: &str) -> Option<(String, Vec<(String, String)>)> {
    let mut module = None;
    let mut replaces = Vec::new();
    let mut block: Option<&str> = None;
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or("");
        let words: Vec<&str> = line.split_whitespace().collect();
        let (verb, args) = match (block, words.split_first()) {
            (_, None) => continue,
            (Some(_), Some((&")", _))) => {
                block = None;
                continue;
            }
            (Some(verb), Some(_)) => (verb, &words[..]),
            (None, Some((&verb, args))) if args == ["("] => {
                block = Some(verb);
                continue;
            }
            (None, Some((&verb, args))) => (verb, args),
        };
        match verb {
            "module" => {
                module = module.or(args.first().map(|path| unquote(path).to_string()));
            }
            "replace" => {
                let Some(arrow) = args.iter().position(|a| *a == "=>") else {
                    continue;
                };
                if let (Some(old), Some(new)) = (args.first(), args.get(arrow + 1)) {
                    let new = unquote(new);
                    if new.starts_with("./") || new.starts_with("../") {
                        replaces.push((unquote(old).to_string(), new.to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    module.map(|module| (module, replaces))
}

fn unquote(word: &str) -> &str {
    word.trim_matches(['"', '`'])
}

/// The files of the package in `dir`: the `.go` files that are not test
/// files and whose name does not start with `_` or `.`, a symlink at its own
/// path.
fn package_files(cx: &mut ResolveCx<'_>, dir: &str) -> Vec<String> {
    let names: Vec<(String, bool)> = cx
        .list_dir(dir)
        .unwrap_or_default()
        .iter()
        .filter(|e| e.name.ends_with(".go") && !e.name.ends_with("_test.go"))
        .filter(|e| !e.name.starts_with(['_', '.']))
        .filter(|e| e.kind != EntryKind::Dir)
        .map(|e| (join(dir, &e.name), e.kind == EntryKind::Symlink))
        .collect();
    let mut files = Vec::new();
    for (path, symlink) in names {
        if !symlink {
            files.push(path);
        } else if let Probe::File(target) = cx.probe_file(&path) {
            files.push(target);
        }
    }
    files
}
