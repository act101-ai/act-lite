//! The 30-language end-to-end matrix for `act-lite query`, run against the
//! built binary. Each fixture under `tests/fixtures/langs/<dir>/` has a symbol
//! set written from its source; the expectations in `SPECS` come from that
//! source, not from the binary's output.

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn langs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/langs")
}

/// One fixture and what its source declares.
struct Spec {
    /// Subdirectory of `fixtures/langs`.
    dir: &'static str,
    /// The fixture's only source file.
    file: &'static str,
    /// Names the source declares (a symbol's name may carry extra text).
    names: &'static [&'static str],
    /// Names the source declares as functions, types, classes or tables:
    /// the ones a skeleton lists.
    decls: &'static [&'static str],
    /// A name for `symbol` and `symbols-batch`, and its kind when the source
    /// fixes one (`None` for data and markup languages).
    probe: &'static str,
    kind: Option<&'static str>,
    /// The line the probe is declared on.
    probe_line: u32,
    /// A name for `definition` and `references`: declared on `def_line` and
    /// used on `use_line` (equal when the source never uses it).
    target: &'static str,
    def_line: u32,
    use_line: u32,
}

#[allow(clippy::too_many_arguments)]
const fn spec(
    dir: &'static str,
    file: &'static str,
    names: &'static [&'static str],
    decls: &'static [&'static str],
    probe: &'static str,
    kind: Option<&'static str>,
    probe_line: u32,
    target: &'static str,
    def_line: u32,
    use_line: u32,
) -> Spec {
    Spec {
        dir,
        file,
        names,
        decls,
        probe,
        kind,
        probe_line,
        target,
        def_line,
        use_line,
    }
}

const FN: Option<&str> = Some("function");

const SPECS: &[Spec] = &[
    spec(
        "bash",
        "fixture.sh",
        &["greet", "farewell"],
        &["greet", "farewell"],
        "greet",
        FN,
        2,
        "greet",
        2,
        8,
    ),
    spec(
        "c",
        "fixture.c",
        &["Point", "x", "y", "add", "main"],
        &["Point", "add", "main"],
        "add",
        FN,
        5,
        "add",
        5,
        7,
    ),
    spec(
        "csharp",
        "fixture.cs",
        &["Calculator", "Add", "Twice"],
        &["Calculator", "Add", "Twice"],
        "Calculator",
        Some("class"),
        1,
        "Add",
        3,
        4,
    ),
    spec(
        "cpp",
        "fixture.cpp",
        &["util", "twice", "Counter", "next", "n", "main"],
        &["util", "twice", "Counter", "main"],
        "Counter",
        Some("class"),
        7,
        "twice",
        4,
        14,
    ),
    spec(
        "css",
        "fixture.css",
        &[".card", "#header", "color", "margin"],
        &[".card", "#header"],
        ".card",
        None,
        1,
        "card",
        1,
        1,
    ),
    spec(
        "dart",
        "fixture.dart",
        &["Account", "balance", "deposit", "double_it", "main"],
        &["Account", "double_it", "main"],
        "double_it",
        FN,
        6,
        "double_it",
        6,
        9,
    ),
    spec(
        "dockerfile",
        "Dockerfile",
        &["build"],
        &["build"],
        "build",
        None,
        1,
        "build",
        1,
        5,
    ),
    spec(
        "elixir",
        "fixture.ex",
        &["Greeter", "hello", "twice"],
        &["Greeter", "hello", "twice"],
        "Greeter",
        Some("module"),
        1,
        "hello",
        2,
        7,
    ),
    spec(
        "go",
        "fixture.go",
        &["Point", "X", "Y", "Add", "main"],
        &["Point", "Add", "main"],
        "Add",
        FN,
        8,
        "Add",
        8,
        11,
    ),
    spec(
        "hcl",
        "fixture.tf",
        &["region", "default", "aws_s3_bucket", "logs", "bucket"],
        &["region", "logs"],
        "region",
        None,
        1,
        "region",
        1,
        6,
    ),
    spec(
        "html",
        "fixture.html",
        &["main", "note"],
        &["main"],
        "main",
        None,
        4,
        "main",
        4,
        4,
    ),
    spec(
        "java",
        "Main.java",
        &["Main", "add", "main", "Shape", "area"],
        &["Main", "add", "main", "Shape", "area"],
        "Shape",
        Some("interface"),
        5,
        "add",
        2,
        3,
    ),
    spec(
        "javascript",
        "fixture.js",
        &["add", "Box", "open", "total"],
        &["add", "Box", "open"],
        "add",
        FN,
        1,
        "add",
        1,
        3,
    ),
    spec(
        "jsx",
        "fixture.jsx",
        &["Hello", "App"],
        &["Hello"],
        "Hello",
        FN,
        1,
        "Hello",
        1,
        2,
    ),
    spec(
        "json",
        "fixture.json",
        &["name", "version"],
        &["name", "version"],
        "name",
        None,
        2,
        "name",
        2,
        2,
    ),
    spec(
        "kotlin",
        "fixture.kt",
        &["Counter", "next", "add", "main"],
        &["Counter", "add", "main"],
        "add",
        FN,
        3,
        "add",
        3,
        5,
    ),
    spec(
        "lua",
        "fixture.lua",
        &["add", "greet"],
        &["add", "greet"],
        "add",
        FN,
        1,
        "add",
        1,
        9,
    ),
    spec(
        "markdown",
        "fixture.md",
        &["Title", "Install"],
        &["Title", "Install"],
        "Install",
        None,
        3,
        "Install",
        3,
        3,
    ),
    spec(
        "php",
        "fixture.php",
        &["add", "Box", "open"],
        &["add", "Box", "open"],
        "add",
        FN,
        2,
        "add",
        2,
        3,
    ),
    spec(
        "python",
        "fixture.py",
        &["add", "Box", "open"],
        &["add", "Box", "open"],
        "add",
        FN,
        1,
        "add",
        1,
        7,
    ),
    spec(
        "ruby",
        "fixture.rb",
        &["add", "Box", "open"],
        &["add", "Box", "open"],
        "add",
        FN,
        1,
        "add",
        1,
        7,
    ),
    spec(
        "rust",
        "fixture.rs",
        &["Point", "x", "y", "add", "main"],
        &["Point", "add", "main"],
        "add",
        FN,
        3,
        "add",
        3,
        5,
    ),
    spec(
        "scala",
        "fixture.scala",
        &["Calc", "add", "twice"],
        &["Calc", "add", "twice"],
        "Calc",
        Some("class"),
        1,
        "add",
        2,
        3,
    ),
    spec(
        "scss",
        "fixture.scss",
        &["$primary", "rounded", "border-radius", ".btn", "color"],
        &["rounded", ".btn"],
        "rounded",
        None,
        3,
        "rounded",
        3,
        7,
    ),
    spec(
        "sql",
        "fixture.sql",
        &["users", "id", "name", "active_users"],
        &["users", "active_users"],
        "users",
        None,
        1,
        "users",
        1,
        3,
    ),
    spec(
        "swift",
        "fixture.swift",
        &["Point", "x", "y", "add", "total"],
        &["Point", "add"],
        "add",
        FN,
        3,
        "add",
        3,
        5,
    ),
    spec(
        "toml",
        "fixture.toml",
        &["package", "name", "dependencies", "serde"],
        &["package", "dependencies"],
        "package",
        None,
        1,
        "package",
        1,
        1,
    ),
    spec(
        "tsx",
        "fixture.tsx",
        &["Hello", "App", "Props"],
        &["Hello", "Props"],
        "Hello",
        FN,
        1,
        "Hello",
        1,
        2,
    ),
    spec(
        "typescript",
        "fixture.ts",
        &["Shape", "add", "Id"],
        &["Shape", "add", "Id"],
        "add",
        FN,
        2,
        "add",
        2,
        4,
    ),
    spec(
        "yaml",
        "fixture.yaml",
        &["name", "services", "web", "image"],
        &["name", "services", "web", "image"],
        "services",
        None,
        2,
        "services",
        2,
        2,
    ),
];

fn spec_for(dir: &str) -> &'static Spec {
    SPECS
        .iter()
        .find(|s| s.dir == dir)
        .unwrap_or_else(|| panic!("no spec for {dir}"))
}

/// An empty directory for `PATH`, so no language server can be found.
fn empty_path_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Runs `act-lite <args>` from `cwd` with an isolated `PATH`, asserts success
/// and returns the parsed stdout JSON.
fn run_json(cwd: &Path, envs: &[(&str, &str)], args: &[&str]) -> Value {
    let path = empty_path_dir();
    let mut cmd = Command::cargo_bin("act-lite").expect("the act-lite binary must be built");
    cmd.current_dir(cwd).env("PATH", path.path());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd.args(args).output().unwrap();
    assert!(
        output.status.success(),
        "act-lite {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// A query against one fixture, run from its own directory.
fn query(s: &Spec, args: &[&str]) -> Value {
    let mut full = vec!["query"];
    full.extend_from_slice(args);
    run_json(&langs().join(s.dir), &[], &full)
}

fn names_of(result: &Value, key: &str) -> Vec<String> {
    result[key]
        .as_array()
        .unwrap_or_else(|| panic!("no {key} array in {result}"))
        .iter()
        .map(|d| d["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// A symbol name without the JSON string quotes and the Markdown heading's
/// leading space, the only decoration a name may carry.
fn norm(name: &str) -> &str {
    name.trim_matches('"').trim_start_matches(' ')
}

fn assert_has(found: &[String], expected: &[&str], what: &str) {
    for want in expected {
        assert!(
            found.iter().any(|n| norm(n) == *want),
            "{what}: expected a symbol named `{want}`, got {found:?}"
        );
    }
}

/// The 1-based column of the first whole-word `word` on 1-based `line`.
fn column_of(s: &Spec, line: u32, word: &str) -> u32 {
    let src = std::fs::read_to_string(langs().join(s.dir).join(s.file)).unwrap();
    let text = src.lines().nth(line as usize - 1).unwrap();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let at = from + i;
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if !before.is_some_and(is_word) && !after.is_some_and(is_word) {
            return text[..at].chars().count() as u32 + 1;
        }
        from = at + word.len();
    }
    panic!("`{word}` not on line {line} of {}", s.file);
}

fn position_args(s: &Spec, line: u32) -> [String; 6] {
    [
        "--file".into(),
        s.file.into(),
        "--line".into(),
        line.to_string(),
        "--column".into(),
        column_of(s, line, s.target).to_string(),
    ]
}

mod ops {
    use super::*;

    pub fn skeleton(s: &Spec) {
        let result = query(s, &["skeleton", s.file]);
        assert_has(&names_of(&result, "declarations"), s.decls, "skeleton");
    }

    pub fn symbols(s: &Spec) {
        let result = query(s, &["symbols", s.file]);
        assert_has(&names_of(&result, "symbols"), s.names, "symbols");
    }

    pub fn symbol(s: &Spec) {
        let result = query(s, &["symbol", s.probe, "--file", s.file]);
        let name = result["symbol"]["name"].as_str().unwrap_or_default();
        assert_eq!(norm(name), s.probe, "{result}");
        assert_eq!(
            result["symbol"]["range"]["start"]["line"], s.probe_line,
            "`{}` is declared on line {}: {result}",
            s.probe, s.probe_line
        );
        assert!(
            !result["source"].as_str().unwrap_or_default().is_empty(),
            "the source text must not be empty: {result}"
        );
        if let Some(kind) = s.kind {
            assert_eq!(result["symbol"]["kind"], kind, "{result}");
        }
    }

    pub fn batch(s: &Spec) {
        // --files: the probe is listed, with an id.
        let result = query(s, &["symbols-batch", "--files", s.file]);
        let symbols = result["results"][0]["symbols"]
            .as_array()
            .unwrap_or_else(|| {
                panic!("no results[0].symbols in {result}");
            });
        let found: Vec<String> = symbols
            .iter()
            .map(|y| y["name"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_has(&found, s.names, "symbols-batch --files");
        let probe = symbols
            .iter()
            .find(|y| y["name"].as_str().is_some_and(|n| norm(n) == s.probe))
            .unwrap_or_else(|| panic!("probe `{}` missing in {result}", s.probe));
        let id = probe["id"].as_str().expect("a batch symbol has an id");
        // --kinds: the probe's kind keeps it; a kind the source lacks drops all.
        if let Some(kind) = s.kind {
            let result = query(s, &["symbols-batch", "--files", s.file, "--kinds", kind]);
            let only = result["results"][0]["symbols"].as_array().unwrap();
            assert!(
                only.iter().all(|y| y["kind"] == kind)
                    && only
                        .iter()
                        .any(|y| y["name"].as_str().is_some_and(|n| norm(n) == s.probe)),
                "--kinds {kind} must keep `{}` and only kind {kind}: {result}",
                s.probe
            );
        }
        let result = query(s, &["symbols-batch", "--files", s.file, "--kinds", "trait"]);
        assert_eq!(
            result["total_symbols"], 0,
            "no trait in the source: {result}"
        );
        // --ids: the id from --files selects that symbol.
        let result = query(s, &["symbols-batch", "--ids", id]);
        assert!(
            result["results"][0]["symbols"].as_array().is_some_and(|l| l
                .iter()
                .any(|y| y["name"].as_str().is_some_and(|n| norm(n) == s.probe))),
            "--ids {id} must return `{}`: {result}",
            s.probe
        );
    }

    pub fn definition(s: &Spec) {
        let pos = position_args(s, s.use_line);
        let mut args = vec!["definition", s.target];
        args.extend(pos.iter().map(String::as_str));
        args.push("--disable-lsp");
        let result = query(s, &args);
        let def = &result["definition"];
        assert!(
            def["file"].as_str().is_some_and(|f| f.ends_with(s.file)),
            "definition must be in {}: {result}",
            s.file
        );
        assert_eq!(def["line"], s.def_line, "definition line: {result}");
    }

    fn references_with(s: &Spec, flag: &[&str], envs: &[(&str, &str)]) {
        let pos = position_args(s, s.def_line);
        let mut full = vec!["query", "references", s.target];
        full.extend(pos.iter().map(String::as_str));
        full.extend_from_slice(flag);
        let result = run_json(&langs().join(s.dir), envs, &full);
        assert_eq!(result["source"], "tree-sitter", "{result}");
        let lines: Vec<u64> = result["references"]
            .as_array()
            .unwrap_or_else(|| panic!("no references in {result}"))
            .iter()
            .filter(|r| r["file"].as_str().is_some_and(|f| f.ends_with(s.file)))
            .filter_map(|r| r["line"].as_u64())
            .collect();
        for line in [s.def_line, s.use_line] {
            assert!(
                lines.contains(&(line as u64)),
                "references must include line {line}, got {lines:?}"
            );
        }
    }

    pub fn references(s: &Spec) {
        references_with(s, &["--disable-lsp"], &[]);
    }

    pub fn references_env(s: &Spec) {
        references_with(s, &[], &[("ACT_LITE_DISABLE_LSP", "1")]);
    }
}

macro_rules! lang {
    ($dir:ident { $( $(#[$m:meta])* $op:ident ),* $(,)? }) => {
        mod $dir {
            use super::*;
            $(
                #[test]
                $(#[$m])*
                fn $op() {
                    ops::$op(spec_for(stringify!($dir)));
                }
            )*
        }
    };
}

lang!(bash {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(c {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(csharp {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(cpp {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(css {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(dart {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(dockerfile {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(elixir {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(go {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(hcl {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(html {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(java {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(javascript {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(jsx {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(json {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(kotlin {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(lua {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(markdown {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(php {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(python {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(ruby {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(rust {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(scala {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(scss {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(sql {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(swift {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(toml {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(tsx {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(typescript {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});
lang!(yaml {
    skeleton,
    symbols,
    symbol,
    batch,
    definition,
    references,
    references_env,
});

/// A name the source never declares is an error, not a made-up symbol.
#[test]
fn symbol_unknown_name_is_an_error() {
    let path = empty_path_dir();
    let output = act_lite_in(&langs().join("python"), path.path())
        .args(["query", "symbol", "zzz", "--file", "fixture.py"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "`zzz` is not in fixture.py: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ---- Fixture tree, global flags, errors ----

fn act_lite_in(cwd: &Path, path: &Path) -> Command {
    let mut cmd = Command::cargo_bin("act-lite").expect("the act-lite binary must be built");
    cmd.current_dir(cwd).env("PATH", path);
    cmd
}

fn outline(args: &[&str]) -> Value {
    let mut full = vec!["query", "repo-outline"];
    full.extend_from_slice(args);
    run_json(&langs(), &[], &full)
}

fn outline_paths(result: &Value) -> Vec<String> {
    result["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no files in {result}"))
        .iter()
        .map(|f| f["path"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn outline_lists_every_fixture() {
    let result = outline(&["--max-files", "1000"]);
    assert_eq!(result["total_files"], 30, "{result}");
    assert_eq!(result["truncated"], false, "{result}");
    let paths = outline_paths(&result);
    for s in SPECS {
        assert!(
            paths
                .iter()
                .any(|p| p.ends_with(&format!("{}/{}", s.dir, s.file))),
            "{}/{} missing from {paths:?}",
            s.dir,
            s.file
        );
    }
}

#[test]
fn outline_max_files_truncates() {
    let result = outline(&["--max-files", "5"]);
    assert_eq!(outline_paths(&result).len(), 5, "{result}");
    assert_eq!(result["truncated"], true, "{result}");
}

#[test]
fn outline_include_and_exclude() {
    let result = outline(&["--include", "*.py"]);
    assert_eq!(
        outline_paths(&result),
        vec!["python/fixture.py"],
        "{result}"
    );
    let result = outline(&["--exclude", "*.py", "--max-files", "1000"]);
    let paths = outline_paths(&result);
    assert_eq!(paths.len(), 29, "{paths:?}");
    assert!(paths.iter().all(|p| !p.ends_with(".py")), "{paths:?}");
}

#[test]
fn outline_symbols_lists_top_level_names() {
    let result = outline(&["--include", "*.rs", "--symbols"]);
    let symbols: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols in {result}"))
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for want in ["Point", "add", "main"] {
        assert!(symbols.contains(&want), "{want} missing in {symbols:?}");
    }
    let result = outline(&["--include", "*.rs"]);
    assert!(result["files"][0].get("symbols").is_none(), "{result}");
}

/// A temporary tree: `top.py`, `a/mid.py`, `a/b/deep.py`, and one `x.py` in
/// each of `.git`, `target` and `node_modules`.
fn tree() -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    for rel in [
        "top.py",
        "a/mid.py",
        "a/b/deep.py",
        ".git/x.py",
        "target/x.py",
        "node_modules/x.py",
    ] {
        let p = ws.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "def f():\n    pass\n").unwrap();
    }
    ws
}

#[test]
fn outline_depth_limits_directories() {
    let ws = tree();
    let result = run_json(ws.path(), &[], &["query", "repo-outline", "--depth", "1"]);
    let paths = outline_paths(&result);
    assert!(paths.contains(&"top.py".to_string()), "{paths:?}");
    assert!(
        !paths.iter().any(|p| p.ends_with("deep.py")),
        "--depth 1 must not reach a/b/deep.py: {paths:?}"
    );
}

#[test]
fn outline_skips_git_target_and_node_modules() {
    let ws = tree();
    let result = run_json(ws.path(), &[], &["query", "repo-outline"]);
    let paths = outline_paths(&result);
    assert_eq!(paths.len(), 3, "only top, mid and deep: {paths:?}");
    for skipped in [".git/", "target/", "node_modules/"] {
        assert!(!paths.iter().any(|p| p.contains(skipped)), "{paths:?}");
    }
}

#[test]
fn workspace_flag_sets_the_root() {
    let ws = tree();
    let elsewhere = tempfile::tempdir().unwrap();
    let result = run_json(
        elsewhere.path(),
        &[],
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "query",
            "repo-outline",
        ],
    );
    assert_eq!(outline_paths(&result).len(), 3, "{result}");
}

#[test]
fn format_text_is_indented_json() {
    let path = empty_path_dir();
    let output = act_lite_in(&langs().join("python"), path.path())
        .args(["--format", "text", "query", "skeleton", "fixture.py"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().count() > 1 && stdout.contains("\n  "),
        "--format text must indent: {stdout}"
    );
    let result: Value = serde_json::from_str(&stdout).unwrap();
    assert_has(&names_of(&result, "declarations"), &["add", "Box"], "text");
}

#[test]
fn version_prints_the_workspace_version() {
    let output = Command::cargo_bin("act-lite")
        .unwrap()
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("act-lite {}", env!("CARGO_PKG_VERSION"))
    );
}

/// Runs `act-lite <args>` from the `python` fixture with an isolated `PATH`
/// and returns its output, asserting failure without a panic.
fn run_error(cwd: &Path, args: &[&str]) -> String {
    let path = empty_path_dir();
    let output = act_lite_in(cwd, path.path()).args(args).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(!output.status.success(), "act-lite {args:?} must fail");
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
    assert!(
        stderr.contains("\"error\""),
        "JSON error on stderr: {stderr}"
    );
    stderr
}

#[test]
fn unsupported_extension_names_it_and_lists_30_languages() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join("a.cob"), "x\n").unwrap();
    let stderr = run_error(ws.path(), &["query", "skeleton", "a.cob"]);
    assert!(stderr.contains(".cob"), "{stderr}");
    for lang in [
        "Bash",
        "C",
        "C#",
        "C++",
        "CSS",
        "Dart",
        "Dockerfile",
        "Elixir",
        "Go",
        "HCL/Terraform",
        "HTML",
        "Java",
        "JavaScript",
        "JSX",
        "JSON",
        "Kotlin",
        "Lua",
        "Markdown",
        "PHP",
        "Python",
        "Ruby",
        "Rust",
        "Scala",
        "SCSS",
        "SQL",
        "Swift",
        "TOML",
        "TSX",
        "TypeScript",
        "YAML",
    ] {
        assert!(stderr.contains(lang), "{lang} missing from {stderr}");
    }
}

#[test]
fn missing_file_is_an_error() {
    let stderr = run_error(&langs().join("python"), &["query", "skeleton", "nope.py"]);
    assert!(stderr.contains("nope.py"), "{stderr}");
}

#[test]
fn unparsable_file_is_an_error() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join("bad.py"), [0xff, 0xfe, 0x00, 0xff]).unwrap();
    run_error(ws.path(), &["query", "skeleton", "bad.py"]);
}

#[test]
fn path_outside_workspace_is_an_error() {
    let ws = tempfile::tempdir().unwrap();
    let outside = langs().join("python").join("fixture.py");
    run_error(
        ws.path(),
        &[
            "--workspace",
            ws.path().to_str().unwrap(),
            "query",
            "skeleton",
            outside.to_str().unwrap(),
        ],
    );
}

#[test]
fn closed_stdout_exits_141_quietly() {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let path = empty_path_dir();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_act-lite"))
        .current_dir(langs().join("python"))
        .env("PATH", path.path())
        .args(["query", "skeleton", "fixture.py"])
        .stdout(std::process::Stdio::from(writer))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(141), "{:?}", output);
    assert!(output.stderr.is_empty(), "{:?}", output);
}

#[test]
fn server_queries_without_a_server_name_the_binary_and_install_command() {
    for op in ["callers", "callees", "implementations"] {
        let stderr = run_error(
            &langs().join("typescript"),
            &["query", op, "add", "--file", "fixture.ts"],
        );
        assert!(
            stderr.contains("typescript-language-server"),
            "{op}: {stderr}"
        );
        assert!(stderr.contains("npm install"), "{op}: {stderr}");
    }
}
