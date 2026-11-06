//! Resolution of Python relationships.
//!
//! A file's *own root* is the parent of the topmost package directory that
//! holds it (the file's directory when that has no `__init__.py`). The roots
//! are, in order, the own root, the workspace root and `src/` when it is a
//! directory. A dotted name exists under a root as a package
//! (`a/b/__init__.py`, checked first), a module (`a/b.py`) or a namespace
//! package (a directory without `__init__.py`). A name that exists under no
//! root is external only when it is a standard-library module or a
//! distribution declared in a dependency file of the importing file's
//! directory or an ancestor, and no module of that name sits elsewhere in
//! the workspace.

use super::toml::{first_error, is_key, key_path, toml_pair, unquote};
use super::{Family, InputKind, Outcome, Probe, ResolveCx, Resolver};
use act_lite_parser::deps::{ModuleNames, Relationship, RelationshipKind, module_names};
use act_lite_parser::{Grammar, SourceFile};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Python 3.13's `sys.stdlib_module_names`, in byte order.
pub const STDLIB_MODULES: &[&str] = &[
    "__future__",
    "_abc",
    "_aix_support",
    "_android_support",
    "_apple_support",
    "_ast",
    "_asyncio",
    "_bisect",
    "_blake2",
    "_bz2",
    "_codecs",
    "_codecs_cn",
    "_codecs_hk",
    "_codecs_iso2022",
    "_codecs_jp",
    "_codecs_kr",
    "_codecs_tw",
    "_collections",
    "_collections_abc",
    "_colorize",
    "_compat_pickle",
    "_compression",
    "_contextvars",
    "_csv",
    "_ctypes",
    "_curses",
    "_curses_panel",
    "_datetime",
    "_dbm",
    "_decimal",
    "_elementtree",
    "_frozen_importlib",
    "_frozen_importlib_external",
    "_functools",
    "_gdbm",
    "_hashlib",
    "_heapq",
    "_imp",
    "_interpchannels",
    "_interpqueues",
    "_interpreters",
    "_io",
    "_ios_support",
    "_json",
    "_locale",
    "_lsprof",
    "_lzma",
    "_markupbase",
    "_md5",
    "_multibytecodec",
    "_multiprocessing",
    "_opcode",
    "_opcode_metadata",
    "_operator",
    "_osx_support",
    "_overlapped",
    "_pickle",
    "_posixshmem",
    "_posixsubprocess",
    "_py_abc",
    "_pydatetime",
    "_pydecimal",
    "_pyio",
    "_pylong",
    "_pyrepl",
    "_queue",
    "_random",
    "_scproxy",
    "_sha1",
    "_sha2",
    "_sha3",
    "_signal",
    "_sitebuiltins",
    "_socket",
    "_sqlite3",
    "_sre",
    "_ssl",
    "_stat",
    "_statistics",
    "_string",
    "_strptime",
    "_struct",
    "_suggestions",
    "_symtable",
    "_sysconfig",
    "_thread",
    "_threading_local",
    "_tkinter",
    "_tokenize",
    "_tracemalloc",
    "_typing",
    "_uuid",
    "_warnings",
    "_weakref",
    "_weakrefset",
    "_winapi",
    "_wmi",
    "_zoneinfo",
    "abc",
    "antigravity",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "atexit",
    "base64",
    "bdb",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "cProfile",
    "calendar",
    "cmath",
    "cmd",
    "code",
    "codecs",
    "codeop",
    "collections",
    "colorsys",
    "compileall",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "copyreg",
    "csv",
    "ctypes",
    "curses",
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "doctest",
    "email",
    "encodings",
    "ensurepip",
    "enum",
    "errno",
    "faulthandler",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    "gc",
    "genericpath",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "grp",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "idlelib",
    "imaplib",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "mailbox",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "modulefinder",
    "msvcrt",
    "multiprocessing",
    "netrc",
    "nt",
    "ntpath",
    "nturl2path",
    "numbers",
    "opcode",
    "operator",
    "optparse",
    "os",
    "pathlib",
    "pdb",
    "pickle",
    "pickletools",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posix",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "pty",
    "pwd",
    "py_compile",
    "pyclbr",
    "pydoc",
    "pydoc_data",
    "pyexpat",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "rlcompleter",
    "runpy",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtplib",
    "socket",
    "socketserver",
    "sqlite3",
    "sre_compile",
    "sre_constants",
    "sre_parse",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "symtable",
    "sys",
    "sysconfig",
    "syslog",
    "tabnanny",
    "tarfile",
    "tempfile",
    "termios",
    "textwrap",
    "this",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "turtle",
    "turtledemo",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "winreg",
    "winsound",
    "wsgiref",
    "xml",
    "xmlrpc",
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "zoneinfo",
];

/// The resolver of the Python family.
pub struct PythonResolver;

impl Resolver for PythonResolver {
    fn family(&self) -> Family {
        Family::Python
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        // The value is the module name without whitespace, comments and
        // line continuations; the specifier keeps the written text.
        let value = rel.value.as_deref().unwrap_or(&rel.specifier);
        match rel.kind {
            RelationshipKind::Import => import(cx, file, value),
            RelationshipKind::FromImport => {
                let name = rel.imported.as_deref().unwrap_or_default();
                from_import(cx, file, module_of(value, name), name)
            }
            _ => Outcome::Unresolved("not a Python import".to_string()),
        }
    }
}

/// What resolution keeps across relationships.
#[derive(Default)]
struct PyCache {
    /// The names bound in each module file read, or the read error.
    names: HashMap<String, Result<ModuleNames, String>>,
    /// The normalized distribution names of each dependency file, or the
    /// reason it could not be read.
    declared: HashMap<String, Result<BTreeSet<String>, String>>,
    /// The top-level candidates by module name, each list in byte order.
    candidates: Option<HashMap<String, Vec<String>>>,
}

/// Where a dotted name was found.
enum Found {
    /// A package's `__init__.py` or a module file.
    File(String),
    /// A namespace package directory.
    Namespace(String),
    /// A symlink on the way leads out of the workspace.
    Outside,
    /// Below a directory that the workspace walk could not read, `<dir>/`.
    Blocked(String),
    /// A segment before the last is a module file, not a package; the
    /// dotted name up to that segment.
    NotPackage(String),
    Missing,
}

/// The outcome of a name found outside the workspace or below an unreadable
/// directory.
fn settled(found: &Found) -> Option<Outcome> {
    match found {
        Found::Outside => Some(Outcome::External),
        Found::Blocked(dir) => Some(Outcome::Unresolved(format!("{dir} could not be read"))),
        Found::NotPackage(prefix) => {
            Some(Outcome::Unresolved(format!("{prefix} is not a package")))
        }
        _ => None,
    }
}

/// `import a.b.c`.
fn import(cx: &mut ResolveCx<'_>, file: &str, dotted: &str) -> Outcome {
    let segments: Vec<&str> = dotted.split('.').collect();
    match locate(cx, file, &segments) {
        Err(outcome) => outcome,
        Ok((_, Found::File(target))) => Outcome::Resolved(vec![target]),
        Ok((_, Found::Namespace(_))) => Outcome::Resolved(Vec::new()),
        Ok((root, Found::Missing)) => {
            Outcome::Unresolved(format!("{dotted} not found under {}", shown(&root)))
        }
        Ok((_, found)) => settled(&found).expect("an outside, blocked or not-package name"),
    }
}

/// `from X import n`; `n` is `*` for a wildcard import.
fn from_import(cx: &mut ResolveCx<'_>, file: &str, module: &str, name: &str) -> Outcome {
    let found = if module.starts_with('.') {
        match relative(cx, file, module) {
            Ok(found) => found,
            Err(outcome) => return outcome,
        }
    } else {
        let segments: Vec<&str> = module.split('.').collect();
        match locate(cx, file, &segments) {
            Err(outcome) => return outcome,
            Ok((root, Found::Missing)) => {
                return Outcome::Unresolved(format!("{module} not found under {}", shown(&root)));
            }
            Ok((_, found)) => found,
        }
    };
    if let Some(outcome) = settled(&found) {
        return outcome;
    }
    imported_from(cx, module, found, name)
}

/// The target of `n` in the module or package `module`, found at `found`:
/// a name bound in the module's file first, then a submodule, together with
/// the file when it has a wildcard import.
fn imported_from(cx: &mut ResolveCx<'_>, module: &str, found: Found, name: &str) -> Outcome {
    let (own_file, package_dir) = match found {
        Found::File(path) => {
            let dir = is_init(&path).then(|| parent(&path).to_string());
            (Some(path), dir)
        }
        Found::Namespace(dir) => (None, Some(dir)),
        _ => (None, None),
    };
    if name == "*" {
        return Outcome::Resolved(own_file.into_iter().collect());
    }
    let names = match &own_file {
        Some(path) => match names_of(cx, path) {
            Ok(names) => Some(names),
            Err(reason) => return Outcome::Unresolved(reason),
        },
        None => None,
    };
    if let (Some(path), Some(names)) = (&own_file, &names)
        && names.names.contains(name)
    {
        return Outcome::Resolved(vec![path.clone()]);
    }
    let glob = names.as_ref().is_some_and(|n| n.glob);
    let submodule = match &package_dir {
        Some(dir) => find(cx, dir, &[name]),
        None => Found::Missing,
    };
    if let Some(outcome) = settled(&submodule) {
        return outcome;
    }
    let mut targets = Vec::new();
    match submodule {
        Found::File(path) => targets.push(path),
        Found::Namespace(_) => {}
        Found::Missing if !glob => {
            let file = match (&own_file, &package_dir) {
                (Some(path), _) => path.clone(),
                (None, Some(dir)) => format!("{dir}/"),
                (None, None) => module.to_string(),
            };
            return Outcome::Unresolved(format!(
                "{name} is neither bound in {file} nor a submodule of {module}"
            ));
        }
        _ => {}
    }
    if glob {
        targets.extend(own_file);
    }
    Outcome::Resolved(targets)
}

/// A relative module (`.`, `..models`) of `file`, in the package directory
/// its dots name.
fn relative(cx: &mut ResolveCx<'_>, file: &str, module: &str) -> Result<Found, Outcome> {
    let dots = module.bytes().take_while(|b| *b == b'.').count();
    let rest = &module[dots..];
    let dir = parent(file);
    let depth = depth(dir).saturating_sub(depth(&own_root(cx, file)));
    if depth == 0 {
        return Err(Outcome::Unresolved(
            "relative import outside a package".to_string(),
        ));
    }
    if dots > depth {
        return Err(Outcome::Unresolved(
            "relative import beyond the top-level package".to_string(),
        ));
    }
    let mut base = dir;
    for _ in 1..dots {
        base = parent(base);
    }
    if rest.is_empty() {
        return Ok(match cx.probe_file(&join(base, "__init__.py")) {
            Probe::File(path) => Found::File(path),
            _ => Found::Namespace(base.to_string()),
        });
    }
    let segments: Vec<&str> = rest.split('.').collect();
    match find(cx, base, &segments) {
        Found::Missing => Err(Outcome::Unresolved(format!(
            "{module} not found under {}",
            shown(base)
        ))),
        Found::NotPackage(prefix) => Err(Outcome::Unresolved(format!(
            "{} is not a package",
            &module[..dots + prefix.len()]
        ))),
        found => Ok(found),
    }
}

/// The root that `segments[0]` exists under (a module or a regular package
/// first, else a namespace package) and where the whole name is found
/// there; the outcome when the first segment exists under no root.
fn locate(
    cx: &mut ResolveCx<'_>,
    file: &str,
    segments: &[&str],
) -> Result<(String, Found), Outcome> {
    let roots = roots(cx, file);
    let first = &segments[..1];
    let mut chosen = None;
    for root in &roots {
        if let Found::File(_) | Found::Outside | Found::Blocked(_) = find(cx, root, first) {
            chosen = Some(root.clone());
            break;
        }
    }
    if chosen.is_none() {
        for root in &roots {
            if let Found::Namespace(_) = find(cx, root, first) {
                chosen = Some(root.clone());
                break;
            }
        }
    }
    let Some(root) = chosen else {
        return Err(outside_the_roots(cx, file, segments[0]));
    };
    let found = find(cx, &root, segments);
    Ok((root, found))
}

/// The outcome for a top-level name that exists under no root.
fn outside_the_roots(cx: &mut ResolveCx<'_>, file: &str, name: &str) -> Outcome {
    if let Some(path) = candidate(cx, name) {
        return Outcome::Unresolved(format!(
            "{name} is a module of the workspace outside the roots: {path}"
        ));
    }
    if let Some(skip) = cx.workspace_skipped().first() {
        return Outcome::Unresolved(format!("{} could not be read", skip.file.file));
    }
    let declared = declared(cx, file, name);
    if STDLIB_MODULES.binary_search(&name).is_ok() {
        return Outcome::External;
    }
    match declared {
        Declared::Yes => Outcome::External,
        Declared::Failed(reason) => Outcome::Unresolved(reason),
        Declared::No => Outcome::Unresolved(format!(
            "{name} is not a standard-library module, a declared dependency or a module under the roots"
        )),
    }
}

enum Declared {
    Yes,
    No,
    /// No file declares the name, and this one could not be read.
    Failed(String),
}

/// Whether a dependency file in `file`'s directory or an ancestor declares
/// a distribution whose normalized name is `name`'s.
fn declared(cx: &mut ResolveCx<'_>, file: &str, name: &str) -> Declared {
    let wanted = normalized(name);
    let mut failed = None;
    let mut dir = parent(file).to_string();
    loop {
        for (path, kind) in dependency_files(cx, &dir) {
            match declared_in(cx, &path, kind) {
                Ok(names) if names.contains(&wanted) => return Declared::Yes,
                Ok(_) => {}
                Err(reason) => {
                    failed.get_or_insert(reason);
                }
            }
        }
        if dir.is_empty() {
            break;
        }
        dir = parent(&dir).to_string();
    }
    failed.map_or(Declared::No, Declared::Failed)
}

/// The dependency files of `dir`, in byte order of name.
fn dependency_files(cx: &mut ResolveCx<'_>, dir: &str) -> Vec<(String, InputKind)> {
    let names: Vec<String> = cx
        .list_dir(dir)
        .map(|entries| entries.iter().map(|e| e.name.clone()).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for name in names {
        let kind = match name.as_str() {
            "pyproject.toml" => InputKind::PyprojectToml,
            "setup.cfg" => InputKind::SetupCfg,
            n if n.starts_with("requirements") && n.ends_with(".txt") => InputKind::Requirements,
            _ => continue,
        };
        if let Probe::File(path) = cx.probe_file(&join(dir, &name)) {
            out.push((path, kind));
        }
    }
    out
}

/// The normalized distribution names declared in the dependency file
/// `path`, read once; the `Err` is the reason it could not be read.
fn declared_in(
    cx: &mut ResolveCx<'_>,
    path: &str,
    kind: InputKind,
) -> Result<BTreeSet<String>, String> {
    if let Some(known) = cx.cache::<PyCache>().declared.get(path) {
        return known.clone();
    }
    let result = match cx.input(path, kind) {
        Err(reason) => Err(reason),
        Ok(text) => {
            let text = text.strip_prefix('\u{feff}').unwrap_or(text).to_string();
            let names = match kind {
                InputKind::PyprojectToml => pyproject_names(&text),
                InputKind::SetupCfg => Ok(setup_cfg_names(&text)),
                _ => Ok(requirements_names(&text)),
            };
            names.map_err(|error| {
                cx.input_error(path, error);
                format!("{path} could not be read")
            })
        }
    };
    let result = result.map(|names| names.iter().map(|n| normalized(n)).collect());
    cx.cache::<PyCache>()
        .declared
        .insert(path.to_string(), result.clone());
    result
}

/// The first top-level candidate named `name` in byte order: a module
/// `name.py` or a regular package `name/` whose directory has no
/// `__init__.py`, so the directory could be a `sys.path` entry.
/// Files inside vendored directories are never candidates.
fn candidate(cx: &mut ResolveCx<'_>, name: &str) -> Option<String> {
    if cx.cache::<PyCache>().candidates.is_none() {
        let files: Vec<String> = cx.workspace_files().to_vec();
        let outside_vendored: Vec<String> = files.into_iter().filter(|f| !cx.vendored(f)).collect();
        let index = candidates(&outside_vendored);
        cx.cache::<PyCache>().candidates = Some(index);
    }
    cx.cache::<PyCache>()
        .candidates
        .as_ref()
        .and_then(|index| index.get(name))
        .and_then(|paths| paths.first().cloned())
}

fn candidates(files: &[String]) -> HashMap<String, Vec<String>> {
    let all: HashSet<&str> = files.iter().map(String::as_str).collect();
    let no_init = |dir: &str| !all.contains(join(dir, "__init__.py").as_str());
    let mut index: HashMap<String, Vec<String>> = HashMap::new();
    for file in files {
        let Some(stem) = file.strip_suffix(".py") else {
            continue;
        };
        if is_init(file) {
            let package = parent(file);
            if package.is_empty() || !no_init(parent(package)) {
                continue;
            }
            index
                .entry(last(package).to_string())
                .or_default()
                .push(format!("{package}/"));
        } else if no_init(parent(file)) {
            index
                .entry(last(stem).to_string())
                .or_default()
                .push(file.clone());
        }
    }
    for paths in index.values_mut() {
        paths.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    }
    index
}

/// The names bound in the module file `path`, read once.
fn names_of(cx: &mut ResolveCx<'_>, path: &str) -> Result<ModuleNames, String> {
    if let Some(known) = cx.cache::<PyCache>().names.get(path) {
        return known.clone();
    }
    let result = cx.source(path).map(|parsed| module_names(&parsed.source));
    cx.cache::<PyCache>()
        .names
        .insert(path.to_string(), result.clone());
    result
}

/// Where `segments` exists under `base`, segment by segment as CPython
/// imports it: at each segment the package first, then the module, then a
/// namespace package. Every segment before the last must be a package; a
/// module file there is `NotPackage`. A probe that leaves the workspace is
/// `Outside`; a namespace package or a miss at or below a directory that
/// the workspace walk could not read is `Blocked`.
fn find(cx: &mut ResolveCx<'_>, base: &str, segments: &[&str]) -> Found {
    let mut path = base.to_string();
    for (i, segment) in segments.iter().enumerate() {
        path = join(&path, segment);
        let last = i + 1 == segments.len();
        match cx.probe_file(&join(&path, "__init__.py")) {
            Probe::File(target) if last => return Found::File(target),
            Probe::File(_) => continue,
            Probe::Outside => return Found::Outside,
            _ => {}
        }
        match cx.probe_file(&format!("{path}.py")) {
            Probe::File(target) if last => return Found::File(target),
            Probe::File(_) => return Found::NotPackage(segments[..=i].join(".")),
            Probe::Outside => return Found::Outside,
            _ => {}
        }
        let (found, at) = match cx.probe_file(&path) {
            Probe::Dir(_) if !last => continue,
            Probe::Dir(dir) => (Found::Namespace(dir.clone()), dir),
            Probe::Outside => return Found::Outside,
            _ => (Found::Missing, path),
        };
        return match blocked(cx, &at) {
            Some(dir) => Found::Blocked(dir),
            None => found,
        };
    }
    Found::Missing
}

/// The directory that the workspace walk could not read at or above `path`.
fn blocked(cx: &ResolveCx<'_>, path: &str) -> Option<String> {
    cx.workspace_skipped()
        .iter()
        .map(|skip| skip.file.file.as_str())
        .filter(|entry| entry.ends_with('/'))
        .find(|dir| path == dir.trim_end_matches('/') || path.starts_with(dir))
        .map(str::to_string)
}

/// The own root of `file`, then the workspace root, then `src`, without
/// duplicates.
fn roots(cx: &mut ResolveCx<'_>, file: &str) -> Vec<String> {
    let mut roots = vec![own_root(cx, file), String::new()];
    if let Probe::Dir(src) = cx.probe_file("src") {
        roots.push(src);
    }
    let mut seen = HashSet::new();
    roots.retain(|r| seen.insert(r.clone()));
    roots
}

/// The parent of the topmost package directory that holds `file`, or its
/// directory when that is no package; the workspace root at most.
fn own_root(cx: &mut ResolveCx<'_>, file: &str) -> String {
    let dir = parent(file);
    if !is_package(cx, dir) {
        return dir.to_string();
    }
    let mut top = dir;
    while !top.is_empty() && is_package(cx, parent(top)) {
        top = parent(top);
    }
    parent(top).to_string()
}

fn is_package(cx: &mut ResolveCx<'_>, dir: &str) -> bool {
    matches!(cx.probe_file(&join(dir, "__init__.py")), Probe::File(_))
}

/// The module part of a `from` specifier whose imported name is `name`.
fn module_of<'s>(specifier: &'s str, name: &str) -> &'s str {
    let rest = specifier.strip_suffix(name).unwrap_or(specifier);
    if rest.bytes().all(|b| b == b'.') {
        rest
    } else {
        rest.strip_suffix('.').unwrap_or(rest)
    }
}

/// A distribution or module name by PEP 503: lower case, each run of `-`,
/// `_` and `.` replaced by `_`.
fn normalized(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut in_run = false;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !in_run {
                out.push('_');
            }
            in_run = true;
        } else {
            out.extend(c.to_lowercase());
            in_run = false;
        }
    }
    out
}

/// The distribution name at the start of a requirement (PEP 508).
fn requirement_name(requirement: &str) -> Option<&str> {
    let text = requirement.trim_start();
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .unwrap_or(text.len());
    let name = text[..end].trim_end_matches(['.', '_', '-']);
    (!name.is_empty()).then_some(name)
}

/// The distribution a requirement line names: comments, options (`-r`,
/// `-e`, `--index-url`), `file:` and `attr:` directives, and bare URLs or
/// paths name none; `name@url` names `name`.
fn requirement_line(line: &str) -> Option<&str> {
    let line = line.split(" #").next().unwrap_or_default().trim();
    if line.is_empty()
        || line.starts_with(['#', ';', '-'])
        || line.starts_with("file:")
        || line.starts_with("attr:")
    {
        return None;
    }
    if let Some((scheme, _)) = line.split_once("://")
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    {
        return None;
    }
    requirement_name(line)
}

/// The requirements of a `requirements*.txt` file.
fn requirements_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| requirement_line(line).map(str::to_string))
        .collect()
}

/// The requirements of `setup.cfg`'s `[options] install_requires`, one per
/// line of its value; a blank line does not end the value.
fn setup_cfg_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut section = "";
    let mut in_value = false;
    for line in text.lines() {
        let trimmed = line.trim();
        let continued = line.starts_with([' ', '\t']);
        if in_value && (continued || trimmed.is_empty()) {
            if let Some(name) = requirement_line(trimmed) {
                out.push(name.to_string());
            }
            continue;
        }
        in_value = false;
        if let Some(header) = trimmed.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            section = header.trim();
            continue;
        }
        if section != "options" {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(['=', ':']) else {
            continue;
        };
        if key.trim() == "install_requires" {
            in_value = true;
            if let Some(name) = requirement_line(value) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// The distributions a `pyproject.toml` declares: `[project] dependencies`,
/// `[project.optional-dependencies]`, `[dependency-groups]`, and the keys
/// of `[tool.poetry.dependencies]`, `[tool.poetry.dev-dependencies]` and
/// each `[tool.poetry.group.<name>.dependencies]` but `python`. The `Err`
/// is the parse error.
fn pyproject_names(text: &str) -> Result<Vec<String>, String> {
    let file = SourceFile::from_content("pyproject.toml", text.to_string(), Grammar::Toml)
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
    let mut pairs = Vec::new();
    let mut cursor = root.walk();
    for item in root.named_children(&mut cursor) {
        match item.kind() {
            "pair" => toml_pair(item, &[], text, &mut |path, value| {
                pairs.push((path, value))
            }),
            "table" | "table_array_element" => {
                let mut cursor = item.walk();
                let mut prefix: Option<Vec<String>> = None;
                for child in item.named_children(&mut cursor) {
                    match (&prefix, child.kind()) {
                        (None, _) if is_key(child) => prefix = Some(key_path(child, text)),
                        (Some(prefix), "pair") => {
                            toml_pair(child, prefix, text, &mut |path, value| {
                                pairs.push((path, value))
                            })
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (path, value) in pairs {
        let path: Vec<&str> = path.iter().map(String::as_str).collect();
        match path.as_slice() {
            ["project", "dependencies"]
            | ["project", "optional-dependencies", _]
            | ["dependency-groups", _] => {
                let mut cursor = value.walk();
                for element in value.named_children(&mut cursor) {
                    if element.kind() == "string"
                        && let Some(name) = requirement_name(&unquote(&text[element.byte_range()]))
                    {
                        out.push(name.to_string());
                    }
                }
            }
            ["tool", "poetry", "dependencies" | "dev-dependencies", name]
            | ["tool", "poetry", "group", _, "dependencies", name]
                if *name != "python" =>
            {
                out.push(name.to_string());
            }
            _ => {}
        }
    }
    Ok(out)
}

fn is_init(path: &str) -> bool {
    path == "__init__.py" || path.ends_with("/__init__.py")
}

/// The directory of a workspace-relative path; `""` for the root.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn last(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

fn depth(dir: &str) -> usize {
    dir.split('/').filter(|s| !s.is_empty()).count()
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// A root as reasons show it: `.` for the workspace root, else `<dir>/`.
fn shown(root: &str) -> String {
    if root.is_empty() {
        ".".to_string()
    } else {
        format!("{root}/")
    }
}
