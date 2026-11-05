//! Resolution of JavaScript, JSX, TypeScript and TSX relationships.
//!
//! A relative specifier is probed as a file, with TypeScript's extension
//! replacement, the extension list and Node's directory rule (`main`, then
//! `index`). A bare specifier goes through the nearest `tsconfig.json` or
//! `jsconfig.json` (`paths`, then `baseUrl`), then the workspace packages by
//! `name`, then the dependency tables of the `package.json` files from the
//! importing file's directory up to the workspace root. It is `external`
//! only as a Node built-in, a declared dependency or a `node_modules`
//! directory; otherwise it is `unresolved`.

use super::{Family, InputKind, Outcome, Probe, ResolveCx, Resolver};
use act_lite_parser::deps::Relationship;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// The Node.js built-in module names (`node:module` `builtinModules`).
pub const NODE_BUILTINS: &[&str] = &[
    "_http_agent",
    "_http_client",
    "_http_common",
    "_http_incoming",
    "_http_outgoing",
    "_http_server",
    "_stream_duplex",
    "_stream_passthrough",
    "_stream_readable",
    "_stream_transform",
    "_stream_wrap",
    "_stream_writable",
    "_tls_common",
    "_tls_wrap",
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "inspector/promises",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "readline/promises",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "util/types",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// The extensions tried after the path itself, in order.
const EXTENSIONS: [&str; 10] = [
    ".ts", ".tsx", ".d.ts", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".cts", ".json",
];

/// The emitted extensions and the source extensions tried in their place.
const REPLACEMENTS: [(&str, &[&str]); 4] = [
    (".js", &[".ts", ".tsx", ".d.ts"]),
    (".jsx", &[".tsx"]),
    (".mjs", &[".mts", ".d.mts"]),
    (".cjs", &[".cts", ".d.cts"]),
];

const DEPENDENCY_TABLES: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

/// Directory names under which a `package.json` adds no workspace package.
const TEST_DIRS: [&str; 5] = ["test", "tests", "__tests__", "spec", "testdata"];

/// The most `extends` levels followed; a deeper chain is reported as a
/// cycle.
const MAX_EXTENDS_DEPTH: usize = 32;

/// The resolver of the JavaScript, JSX, TypeScript and TSX family.
pub struct JsResolver;

impl Resolver for JsResolver {
    fn family(&self) -> Family {
        Family::JsTs
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        if !rel.literal {
            return Outcome::Unresolved("specifier is not a string literal".to_string());
        }
        let spec = rel.specifier.as_str();
        let dir = parent(file);
        if is_relative(spec) {
            let Some(path) = cx.normalize(dir, spec) else {
                return Outcome::External;
            };
            let found = if spec.ends_with('/') {
                probe_dir(cx, &path)
            } else {
                probe_path(cx, &path)
            };
            return found.outcome(|| format!("no file matches {spec}"));
        }
        if spec.starts_with('/') {
            return Outcome::Unresolved("root-relative specifier is not applied".to_string());
        }
        if spec.starts_with("node:") {
            return Outcome::External;
        }
        if spec.starts_with('#') {
            return Outcome::Unresolved("package.json \"imports\" is not applied".to_string());
        }
        bare(cx, dir, spec)
    }
}

/// The result of a probe sequence.
enum Found {
    File(String),
    /// A symlink on the matched path leads outside the workspace.
    Outside,
    None,
    /// An input the probe needed could not be read; the reason.
    Failed(String),
}

impl Found {
    fn outcome(self, no_match: impl FnOnce() -> String) -> Outcome {
        match self {
            Found::File(path) => Outcome::Resolved(vec![path]),
            Found::Outside => Outcome::External,
            Found::None => Outcome::Unresolved(no_match()),
            Found::Failed(reason) => Outcome::Unresolved(reason),
        }
    }
}

/// Step 1: the extension replacement, the path as a file, the path with
/// each extension, then the path as a directory.
fn probe_path(cx: &mut ResolveCx<'_>, path: &str) -> Found {
    if let Some((emitted, sources)) = REPLACEMENTS.iter().find(|(e, _)| path.ends_with(e)) {
        let stem = &path[..path.len() - emitted.len()];
        for ext in *sources {
            if let Some(found) = probe_one(cx, &format!("{stem}{ext}")) {
                return found;
            }
        }
    }
    match probe_as_file(cx, path) {
        Found::None => probe_dir(cx, path),
        found => found,
    }
}

/// Steps 1.2 and 1.3.
fn probe_as_file(cx: &mut ResolveCx<'_>, path: &str) -> Found {
    if let Some(found) = probe_one(cx, path) {
        return found;
    }
    for ext in EXTENSIONS {
        if let Some(found) = probe_one(cx, &format!("{path}{ext}")) {
            return found;
        }
    }
    Found::None
}

/// Step 1.4: the directory's `package.json` `main`, then `index`. A
/// `main` that names the directory itself is passed over; one with a
/// trailing `/` is a directory, probed for its `index`; one that leaves the
/// workspace is outside. A `package.json` in a vendored directory is not
/// read.
fn probe_dir(cx: &mut ResolveCx<'_>, path: &str) -> Found {
    let dir = match cx.probe_file(path) {
        Probe::Dir(dir) => dir,
        Probe::Outside => return Found::Outside,
        _ => return Found::None,
    };
    let manifest = join(&dir, "package.json");
    if !cx.vendored(&manifest) && is_file(cx, &manifest) {
        match package(cx, &manifest) {
            Ok(PackageInfo {
                main: Some(main), ..
            }) => {
                let found = match cx.normalize(&dir, &main) {
                    None => Found::Outside,
                    Some(target) if target == dir => Found::None,
                    Some(target) if main.ends_with('/') => probe_index(cx, &target),
                    Some(target) => probe_as_file(cx, &target),
                };
                if !matches!(found, Found::None) {
                    return found;
                }
            }
            Ok(_) => {}
            Err(reason) => return Found::Failed(reason),
        }
    }
    probe_index(cx, &dir)
}

/// `<dir>/index` with each extension.
fn probe_index(cx: &mut ResolveCx<'_>, dir: &str) -> Found {
    let index = join(dir, "index");
    for ext in EXTENSIONS {
        if let Some(found) = probe_one(cx, &format!("{index}{ext}")) {
            return found;
        }
    }
    Found::None
}

/// One candidate file: a match, a symlink out of the workspace, or nothing.
fn probe_one(cx: &mut ResolveCx<'_>, path: &str) -> Option<Found> {
    match cx.probe_file(path) {
        Probe::File(target) => Some(Found::File(target)),
        Probe::Outside => Some(Found::Outside),
        _ => None,
    }
}

/// Step 5: a bare specifier.
fn bare(cx: &mut ResolveCx<'_>, dir: &str, spec: &str) -> Outcome {
    let name = package_name(spec);
    if let Some(config_path) = nearest_config(cx, dir) {
        let config = match config(cx, &config_path, &mut Vec::new()) {
            Ok(config) => config,
            Err(reason) => return Outcome::Unresolved(reason),
        };
        if let Some((declared_in, entries)) = &config.paths
            && let Some((replacements, star)) = match_paths(entries, spec)
        {
            let base = match &config.base_url {
                Some(BaseUrl::Inside(base)) => Some(base.as_str()),
                Some(BaseUrl::Outside) => None,
                None => Some(declared_in.as_str()),
            };
            let mut left = false;
            for replacement in replacements {
                let target = replacement.replacen('*', star, 1);
                let Some(path) = base.and_then(|base| cx.normalize(base, &target)) else {
                    left = true;
                    continue;
                };
                match probe_path(cx, &path) {
                    Found::None => {}
                    found => return found.outcome(String::new),
                }
            }
            if left {
                return Outcome::External;
            }
        }
        match &config.base_url {
            Some(BaseUrl::Outside) => return Outcome::External,
            Some(BaseUrl::Inside(base)) => {
                let Some(path) = cx.normalize(base, spec) else {
                    return Outcome::External;
                };
                match probe_path(cx, &path) {
                    Found::None => {}
                    found => return found.outcome(String::new),
                }
            }
            None => {}
        }
        if let Some(reason) = config.broken {
            return Outcome::Unresolved(reason);
        }
    }
    if let Some(outcome) = workspace_package(cx, name, spec) {
        return outcome;
    }
    let chain = match manifests(cx, dir) {
        Ok(chain) => chain,
        Err(reason) => return Outcome::Unresolved(reason),
    };
    for (manifest_dir, info) in &chain {
        for (key, value) in &info.dependencies {
            if key != name {
                continue;
            }
            if let Some(path) = ["file:", "link:", "portal:"]
                .iter()
                .find_map(|p| value.strip_prefix(p))
            {
                let Some(location) = cx.normalize(manifest_dir, path) else {
                    return Outcome::External;
                };
                return in_package(cx, &location, name, spec);
            }
            if let Some(range) = value.strip_prefix("workspace:") {
                let alias = alias_name(range).unwrap_or(name);
                let aliased = format!("{alias}{}", &spec[name.len()..]);
                return workspace_package(cx, alias, &aliased).unwrap_or_else(|| {
                    Outcome::Unresolved(format!("no workspace package is named {alias}"))
                });
            }
        }
    }
    let declared = chain
        .iter()
        .any(|(_, info)| info.dependencies.iter().any(|(key, _)| key == name));
    let builtin = NODE_BUILTINS.contains(&spec) || NODE_BUILTINS.contains(&name);
    if builtin || declared || in_node_modules(cx, dir, name) {
        if let Some(skip) = cx.workspace_skipped().first() {
            return Outcome::Unresolved(format!("{} could not be read", skip.file.file));
        }
        return Outcome::External;
    }
    Outcome::Unresolved(format!(
        "bare specifier is not a declared dependency: {name}"
    ))
}

/// Step 5.3: `spec` in the workspace package named `name`; `None` when no
/// workspace `package.json` has that name.
fn workspace_package(cx: &mut ResolveCx<'_>, name: &str, spec: &str) -> Option<Outcome> {
    let dirs = workspace_names(cx).get(name).cloned()?;
    if dirs.len() > 1 {
        return Some(Outcome::Unresolved(format!(
            "workspace package name {name} is declared by more than one package.json"
        )));
    }
    Some(in_package(cx, &dirs[0], name, spec))
}

/// `spec` inside the package directory `dir`: its `exports` is not applied;
/// the package itself is probed as a directory, a subpath as a path.
fn in_package(cx: &mut ResolveCx<'_>, dir: &str, name: &str, spec: &str) -> Outcome {
    let manifest = join(dir, "package.json");
    if !cx.vendored(&manifest) && is_file(cx, &manifest) {
        match package(cx, &manifest) {
            Ok(info) if info.exports => {
                return Outcome::Unresolved(format!(
                    "package.json \"exports\" of {name} is not applied"
                ));
            }
            Ok(_) => {}
            Err(reason) => return Outcome::Unresolved(reason),
        }
    }
    let found = match spec
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        Some(sub) => match cx.normalize(dir, sub) {
            Some(path) => probe_path(cx, &path),
            None => Found::Outside,
        },
        None => probe_dir(cx, dir),
    };
    found.outcome(|| format!("no file matches {spec} in workspace package {name}"))
}

/// The workspace package names and their directories, read once per build.
/// A `package.json` below a test directory adds no name; one in a vendored
/// directory is not read.
fn workspace_names<'c>(cx: &'c mut ResolveCx<'_>) -> &'c BTreeMap<String, Vec<String>> {
    if cx.cache::<JsCache>().names.is_none() {
        let mut manifests: Vec<String> = cx
            .workspace_files()
            .iter()
            .filter(|f| f.rsplit('/').next() == Some("package.json"))
            .filter(|f| !f.split('/').any(|part| TEST_DIRS.contains(&part)))
            .cloned()
            .collect();
        manifests.retain(|f| !cx.vendored(f));
        let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for manifest in manifests {
            if let Ok(PackageInfo {
                name: Some(name), ..
            }) = package(cx, &manifest)
            {
                names
                    .entry(name)
                    .or_default()
                    .push(parent(&manifest).to_string());
            }
        }
        cx.cache::<JsCache>().names = Some(names);
    }
    cx.cache::<JsCache>().names.as_ref().expect("set above")
}

/// The `package.json` files in `dir` and its ancestors, nearest first.
fn manifests(cx: &mut ResolveCx<'_>, dir: &str) -> Result<Vec<(String, PackageInfo)>, String> {
    let mut out = Vec::new();
    for ancestor in ancestors(dir) {
        let manifest = join(ancestor, "package.json");
        if !cx.vendored(&manifest) && is_file(cx, &manifest) {
            out.push((ancestor.to_string(), package(cx, &manifest)?));
        }
    }
    Ok(out)
}

/// Whether `node_modules/<name>/` exists in `dir` or an ancestor, as a
/// directory or a symlink out of the workspace. A name with an empty, `.`
/// or `..` segment is never there.
fn in_node_modules(cx: &mut ResolveCx<'_>, dir: &str, name: &str) -> bool {
    if name.split('/').any(|part| matches!(part, "" | "." | "..")) {
        return false;
    }
    ancestors(dir).into_iter().any(|ancestor| {
        let path = join(&join(ancestor, "node_modules"), name);
        matches!(cx.probe_file(&path), Probe::Dir(_) | Probe::Outside)
    })
}

/// The nearest `tsconfig.json`, else `jsconfig.json`, in `dir` or an
/// ancestor.
fn nearest_config(cx: &mut ResolveCx<'_>, dir: &str) -> Option<String> {
    for ancestor in ancestors(dir) {
        for name in ["tsconfig.json", "jsconfig.json"] {
            let path = join(ancestor, name);
            if is_file(cx, &path) {
                return Some(path);
            }
        }
    }
    None
}

/// What a `package.json` gives resolution.
#[derive(Debug, Clone, Default)]
struct PackageInfo {
    name: Option<String>,
    main: Option<String>,
    exports: bool,
    /// The keys and string values of the four dependency tables, in table
    /// order.
    dependencies: Vec<(String, String)>,
}

/// A `paths` key and its replacements.
type PathsEntry = (String, Vec<String>);

/// A config's `baseUrl`.
#[derive(Debug, Clone)]
enum BaseUrl {
    /// The workspace-relative directory.
    Inside(String),
    /// It leaves the workspace root.
    Outside,
}

/// The `baseUrl` and `paths` that apply to a config, after `extends`.
#[derive(Debug, Clone, Default)]
struct Config {
    base_url: Option<BaseUrl>,
    /// The declaring config's directory and the `paths` entries.
    paths: Option<(String, Vec<PathsEntry>)>,
    /// When the `extends` chain is broken, the reason a bare specifier
    /// that these settings do not resolve is unresolved.
    broken: Option<String>,
}

/// The parsed inputs of one build.
#[derive(Default)]
struct JsCache {
    packages: HashMap<String, Result<PackageInfo, String>>,
    configs: HashMap<String, Result<Config, String>>,
    names: Option<BTreeMap<String, Vec<String>>>,
}

/// The `package.json` at `path`, read and parsed once.
fn package(cx: &mut ResolveCx<'_>, path: &str) -> Result<PackageInfo, String> {
    if let Some(found) = cx.cache::<JsCache>().packages.get(path) {
        return found.clone();
    }
    let parsed = read_json(cx, path, InputKind::PackageJson, false).map(|value| {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        let mut dependencies = Vec::new();
        for table in DEPENDENCY_TABLES {
            if let Some(entries) = value.get(table).and_then(Value::as_object) {
                for (key, version) in entries {
                    let version = version.as_str().unwrap_or_default().to_string();
                    dependencies.push((key.clone(), version));
                }
            }
        }
        PackageInfo {
            name: text("name"),
            main: text("main"),
            exports: value.get("exports").is_some(),
            dependencies,
        }
    });
    cx.cache::<JsCache>()
        .packages
        .insert(path.to_string(), parsed.clone());
    parsed
}

/// The config at `path` with its relative `extends` applied, read once: a
/// setting of the config itself wins, then the later `extends` entries.
/// `chain` holds the configs that extend it, for the cycle check.
fn config(cx: &mut ResolveCx<'_>, path: &str, chain: &mut Vec<String>) -> Result<Config, String> {
    if let Some(found) = cx.cache::<JsCache>().configs.get(path) {
        return found.clone();
    }
    let result = read_config(cx, path, chain);
    cx.cache::<JsCache>()
        .configs
        .insert(path.to_string(), result.clone());
    result
}

/// A setting that an unreadable `extends` member could have supplied is
/// unknown: a later member that is broken blocks it, and so does a member
/// whose own chain is broken without the setting.
fn read_config(
    cx: &mut ResolveCx<'_>,
    path: &str,
    chain: &mut Vec<String>,
) -> Result<Config, String> {
    let value = read_json(cx, path, InputKind::Tsconfig, true)?;
    let dir = parent(path).to_string();
    let options = value.get("compilerOptions");
    let mut own = Config {
        base_url: options
            .and_then(|o| o.get("baseUrl"))
            .and_then(Value::as_str)
            .map(|base| match cx.normalize(&dir, base) {
                Some(base) => BaseUrl::Inside(base),
                None => BaseUrl::Outside,
            }),
        paths: options
            .and_then(|o| o.get("paths"))
            .and_then(Value::as_object)
            .map(|entries| {
                let entries = entries
                    .iter()
                    .map(|(key, targets)| {
                        let targets = targets
                            .as_array()
                            .map(|list| {
                                list.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default();
                        (key.clone(), targets)
                    })
                    .collect();
                (dir.clone(), entries)
            }),
        broken: None,
    };
    let extends: Vec<String> = match value.get("extends") {
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    };
    chain.push(path.to_string());
    let (mut base_blocked, mut paths_blocked) = (false, false);
    for entry in extends.iter().rev().filter(|e| is_relative(e)) {
        match extends_member(cx, path, &dir, entry, chain) {
            Err(reason) => {
                base_blocked |= own.base_url.is_none();
                paths_blocked |= own.paths.is_none();
                own.broken = Some(reason);
            }
            Ok(member) => {
                if own.base_url.is_none() && !base_blocked {
                    match member.base_url {
                        Some(base) => own.base_url = Some(base),
                        None => base_blocked = member.broken.is_some(),
                    }
                }
                if own.paths.is_none() && !paths_blocked {
                    match member.paths {
                        Some(paths) => own.paths = Some(paths),
                        None => paths_blocked = member.broken.is_some(),
                    }
                }
                if member.broken.is_some() {
                    own.broken = member.broken;
                }
            }
        }
    }
    chain.pop();
    Ok(own)
}

/// The config that the relative `extends` entry of `config_path` names:
/// the path, else the path with `.json`. A target that does not exist,
/// leaves the workspace or is already on the chain is an error of
/// `config_path`.
fn extends_member(
    cx: &mut ResolveCx<'_>,
    config_path: &str,
    dir: &str,
    entry: &str,
    chain: &mut Vec<String>,
) -> Result<Config, String> {
    let Some(target) = cx.normalize(dir, entry) else {
        let error = format!("extends target {entry} leaves the workspace");
        return Err(broken_input(cx, config_path, error));
    };
    let target = if is_file(cx, &target) {
        target
    } else if is_file(cx, &format!("{target}.json")) {
        format!("{target}.json")
    } else {
        let error = format!("extends target {target} does not exist");
        return Err(broken_input(cx, config_path, error));
    };
    if chain.contains(&target) || chain.len() >= MAX_EXTENDS_DEPTH {
        let error = format!("extends cycle at {target}");
        return Err(broken_input(cx, config_path, error));
    }
    config(cx, &target, chain)
}

/// Records `error` on the input `path` and gives the reason of the
/// relationships that needed it.
fn broken_input(cx: &mut ResolveCx<'_>, path: &str, error: String) -> String {
    cx.input_error(path, error);
    format!("{path} could not be read")
}

/// An input read with [`ResolveCx::input`] and parsed as a JSON object;
/// with `relaxed`, comments and trailing commas are accepted. A parse
/// failure is recorded on the input.
fn read_json(
    cx: &mut ResolveCx<'_>,
    path: &str,
    kind: InputKind,
    relaxed: bool,
) -> Result<Value, String> {
    let text = cx.input(path, kind)?;
    let text = if relaxed {
        strip_jsonc(text)
    } else {
        text.to_string()
    };
    let parsed = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
        .map_err(|err| err.to_string())
        .and_then(|value| {
            if value.is_object() {
                Ok(value)
            } else {
                Err("not a JSON object".to_string())
            }
        });
    parsed.map_err(|err| {
        cx.input_error(path, err);
        format!("{path} could not be read")
    })
}

/// `text` without `//` and `/* */` comments and without a comma before a
/// closing `}` or `]`, strings left as they are.
fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut plain = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            plain.push(c);
            i += 1;
            while i < chars.len() {
                plain.push(chars[i]);
                if chars[i] == '\\' && i + 1 < chars.len() {
                    plain.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                i += 1;
                if chars[i - 1] == '"' {
                    break;
                }
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
        } else {
            plain.push(c);
            i += 1;
        }
    }
    let chars: Vec<char> = plain.chars().collect();
    let mut out = String::with_capacity(plain.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if !matches!(next, Some('}') | Some(']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// The `paths` entry for `spec`: an exact key, else the matching key with one
/// `*` and the longest prefix before it; its replacements and the text `*`
/// matched.
fn match_paths<'a>(entries: &'a [PathsEntry], spec: &'a str) -> Option<(&'a [String], &'a str)> {
    if let Some((_, targets)) = entries.iter().find(|(key, _)| key == spec) {
        return Some((targets, ""));
    }
    let mut best: Option<(usize, &[String], &str)> = None;
    for (key, targets) in entries {
        let Some((prefix, suffix)) = key.split_once('*') else {
            continue;
        };
        if suffix.contains('*')
            || spec.len() < prefix.len() + suffix.len()
            || !spec.starts_with(prefix)
            || !spec.ends_with(suffix)
        {
            continue;
        }
        if best.is_none_or(|(len, _, _)| prefix.len() > len) {
            let star = &spec[prefix.len()..spec.len() - suffix.len()];
            best = Some((prefix.len(), targets, star));
        }
    }
    best.map(|(_, targets, star)| (targets, star))
}

/// The package name of a bare specifier: its first segment, or its first
/// two for a scoped name.
fn package_name(spec: &str) -> &str {
    let mut ends = spec.match_indices('/').map(|(i, _)| i);
    let end = if spec.starts_with('@') {
        ends.nth(1)
    } else {
        ends.next()
    };
    &spec[..end.unwrap_or(spec.len())]
}

/// The package name of a `workspace:<name>@<range>` alias; `None` for a
/// range alone (`*`, `^1.0.0`).
fn alias_name(value: &str) -> Option<&str> {
    let at = value.rfind('@').filter(|at| *at > 0)?;
    Some(&value[..at])
}

fn is_relative(spec: &str) -> bool {
    spec == "." || spec == ".." || spec.starts_with("./") || spec.starts_with("../")
}

fn is_file(cx: &mut ResolveCx<'_>, path: &str) -> bool {
    matches!(cx.probe_file(path), Probe::File(_))
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// `dir`, then each ancestor up to the workspace root (`""`).
fn ancestors(dir: &str) -> Vec<&str> {
    let mut out = vec![dir];
    let mut at = dir;
    while !at.is_empty() {
        at = parent(at);
        out.push(at);
    }
    out
}
