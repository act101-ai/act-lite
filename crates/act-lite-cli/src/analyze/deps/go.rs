//! Resolution of Go relationships: the module map from the `go.mod` files of
//! the workspace, the candidate modules of an import path and the package
//! files of the one valid candidate.

use super::{Family, INVALID_LITERAL, InputKind, Outcome, Probe, ResolveCx, Resolver, join};
use crate::analyze::read::EntryKind;
use act_lite_parser::deps::{Relationship, go_value};
use std::collections::HashMap;
use std::sync::Arc;

/// The resolver of the Go family.
pub struct GoResolver;

/// Directories whose `go.mod` adds no module path to the map.
const TEST_DIRS: [&str; 5] = ["test", "tests", "__tests__", "spec", "testdata"];

/// What is read of one `go.mod`.
struct GoMod {
    module: String,
    /// The module path of each `replace` and its workspace directory: `None`
    /// when the replacement is not a relative path inside the workspace.
    replaces: Vec<(String, Option<String>)>,
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
        let Ok(import) = rel.value.as_deref() else {
            return Outcome::Unresolved(INVALID_LITERAL.to_string());
        };
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
        let replaces: Vec<&(String, Option<String>)> =
            own.iter().flat_map(|m| m.replaces.iter()).collect();
        let under = |path: &str| {
            import == path
                || import
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        // A replaced module path has only its replacement directories as
        // candidates, as the go command uses no other module of that path; a
        // replacement outside the workspace leaves it none.
        let replaced_away = replaces
            .iter()
            .any(|(old, dir)| dir.is_none() && under(old));
        let kept = modules
            .iter()
            .filter(|(path, _)| !replaces.iter().any(|(old, _)| old == path))
            .map(|(path, dir)| (path.as_str(), dir.as_str()));
        let local = replaces
            .iter()
            .filter_map(|(path, dir)| Some((path.as_str(), dir.as_deref()?)));
        let mut candidates: Vec<(&str, &str)> = Vec::new();
        for (path, dir) in kept.chain(local) {
            if under(path) && !candidates.contains(&(path, dir)) {
                candidates.push((path, dir));
            }
        }
        let mut valid: Vec<(&str, Vec<String>)> = Vec::new();
        let mut outside = false;
        let mut unvendored = false;
        for &(path, dir) in &candidates {
            let rest = import[path.len()..].trim_start_matches('/');
            let package = cx.normalize(dir, rest);
            if package
                .as_deref()
                .is_some_and(|package| vendored(cx, package))
            {
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
                || vendored(cx, &package)
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
            0 if replaced_away => Outcome::External,
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

/// Whether the directory `dir` is a vendored directory or inside one.
fn vendored(cx: &mut ResolveCx<'_>, dir: &str) -> bool {
    cx.vendored(&join(dir, "go.mod"))
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
        if ignored(dir)
            || cx.vendored(&file)
            || dir.split('/').any(|part| TEST_DIRS.contains(&part))
        {
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
            && !vendored(cx, at)
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
            .map(|(path, target)| {
                // Go's modfile treats `.`, `..` and paths starting `./` or
                // `../` as directories.
                let relative = target == "."
                    || target == ".."
                    || target.starts_with("./")
                    || target.starts_with("../");
                let local = relative.then(|| cx.normalize(dir, &target)).flatten();
                (path, local)
            })
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

/// The `module` path and the module path and replacement of each `replace`
/// directive of a `go.mod`; `None` without a `module` directive.
fn parse(text: &str) -> Option<(String, Vec<(String, String)>)> {
    let mut module = None;
    let mut replaces = Vec::new();
    let mut block: Option<String> = None;
    for line in text.lines() {
        let Some(words) = tokens(line) else {
            continue;
        };
        let (verb, args) = match (block.as_deref(), words.split_first()) {
            (_, None) => continue,
            (Some(_), Some((first, _))) if first == ")" => {
                block = None;
                continue;
            }
            (Some(verb), Some(_)) => (verb, &words[..]),
            (None, Some((verb, args))) if args == ["("] => {
                block = Some(verb.clone());
                continue;
            }
            (None, Some((verb, args))) => (verb.as_str(), args),
        };
        match verb {
            "module" => {
                module = module.or(args.first().cloned());
            }
            "replace" => {
                let Some(arrow) = args.iter().position(|a| a == "=>") else {
                    continue;
                };
                if let (Some(old), Some(new)) = (args.first(), args.get(arrow + 1)) {
                    replaces.push((old.clone(), new.clone()));
                }
            }
            _ => {}
        }
    }
    module.map(|module| (module, replaces))
}

/// The tokens of one `go.mod` line, as Go's modfile reads them: `//` outside
/// a string starts a comment; `(`, `)`, `[`, `]`, `{`, `}` and `,` are tokens
/// of their own; a string is one token, its value (an interpreted `"…"`
/// string decoded by Go's escapes, a raw `` `…` `` string as written); any
/// other run of characters up to a space, one of those characters or `//`
/// is a token. `None` when a string is not closed or does not decode.
fn tokens(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut rest = line;
    loop {
        rest = rest.trim_start();
        let Some(first) = rest.chars().next() else {
            return Some(words);
        };
        if rest.starts_with("//") {
            return Some(words);
        }
        let end = match first {
            '(' | ')' | '[' | ']' | '{' | '}' | ',' => {
                words.push(first.to_string());
                1
            }
            '"' => {
                let mut chars = rest.char_indices().skip(1);
                let close = loop {
                    match chars.next()? {
                        (_, '\\') => {
                            chars.next()?;
                        }
                        (at, '"') => break at,
                        _ => {}
                    }
                };
                words.push(go_value(&rest[1..close]).ok()?);
                close + 1
            }
            '`' => {
                let close = rest[1..].find('`')? + 1;
                words.push(rest[1..close].to_string());
                close + 1
            }
            _ => {
                let end = rest
                    .char_indices()
                    .find(|&(at, c)| {
                        c.is_whitespace()
                            || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | ',')
                            || rest[at..].starts_with("//")
                    })
                    .map_or(rest.len(), |(at, _)| at);
                words.push(rest[..end].to_string());
                end
            }
        };
        rest = &rest[end..];
    }
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
