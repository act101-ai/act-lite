//! MCP over stdio, end to end: every test starts the built `act-lite mcp
//! serve` as a child process and speaks JSON-RPC lines to it. The per-language
//! expectations in `ROWS` are the `SPECS` of `lang_matrix.rs`, written from the
//! fixture sources under `tests/fixtures/langs/`.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn langs() -> PathBuf {
    fixtures().join("langs")
}

/// What a language server needs to run: nothing on `PATH` unless a test adds it.
struct Setup {
    cwd: PathBuf,
    workspace: Option<PathBuf>,
    path: PathBuf,
    lsp: bool,
}

/// A running `act-lite mcp serve` and the replies it has sent.
struct Session {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
    next_id: u64,
    _path: Option<tempfile::TempDir>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Session {
    /// Starts the server and performs the `initialize` handshake; returns the
    /// session and the `initialize` reply.
    fn start(setup: &Setup, keep: Option<tempfile::TempDir>) -> (Session, Value) {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("act-lite"));
        cmd.current_dir(&setup.cwd).env("PATH", &setup.path);
        if !setup.lsp {
            cmd.arg("--disable-lsp");
        }
        if let Some(ws) = &setup.workspace {
            cmd.arg("--workspace").arg(ws);
        }
        let mut child = cmd
            .args(["mcp", "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the act-lite binary must start");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, replies) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut session = Session {
            child,
            stdin,
            replies,
            next_id: 1,
            _path: keep,
        };
        let init = session.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}),
        );
        session.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        (session, init)
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Sends a request and returns the whole JSON-RPC reply for its id.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let line = self
                .replies
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|e| panic!("no reply to {method} (id {id}): {e}"));
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && v["id"] == id
            {
                return v;
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": arguments}))
    }

    /// A call that must succeed: the first text block parsed as JSON (text
    /// after the JSON value, such as a note, is ignored).
    fn ok(&mut self, tool: &str, arguments: Value) -> Value {
        let reply = self.call(tool, arguments.clone());
        assert!(
            reply.get("error").is_none() && reply["result"]["isError"] != true,
            "{tool} {arguments} must succeed: {reply}"
        );
        let text = reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool}: no text block in {reply}"));
        serde_json::Deserializer::from_str(text)
            .into_iter::<Value>()
            .next()
            .and_then(Result::ok)
            .unwrap_or_else(|| panic!("{tool}: output is not JSON: {text}"))
    }

    /// A call that must fail; returns the error message.
    fn err(&mut self, tool: &str, arguments: Value) -> String {
        let reply = self.call(tool, arguments.clone());
        if let Some(e) = reply.get("error") {
            return e["message"].as_str().unwrap_or_default().to_string();
        }
        assert_eq!(
            reply["result"]["isError"], true,
            "{tool} {arguments} must fail: {reply}"
        );
        reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

/// A server over the 30-language fixture tree, with no language server.
fn serve_langs() -> Session {
    serve_in(&langs())
}

/// A server rooted at `root` (given as `--workspace`), no language server.
fn serve_in(root: &Path) -> Session {
    let empty = tempfile::tempdir().unwrap();
    let setup = Setup {
        cwd: root.to_path_buf(),
        workspace: Some(root.to_path_buf()),
        path: empty.path().to_path_buf(),
        lsp: false,
    };
    Session::start(&setup, Some(empty)).0
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

fn paths_of(result: &Value) -> Vec<String> {
    result["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no files in {result}"))
        .iter()
        .map(|f| f["path"].as_str().unwrap_or_default().to_string())
        .collect()
}

// ---- The 30 fixtures ----

/// One fixture and what its source declares (the `Spec` of `lang_matrix.rs`).
struct Row {
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
}

const FN: Option<&str> = Some("function");

#[allow(clippy::too_many_arguments)]
const fn row(
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
) -> Row {
    Row {
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

const ROWS: &[Row] = &[
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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
    row(
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

fn row_for(dir: &str) -> &'static Row {
    ROWS.iter()
        .find(|r| r.dir == dir)
        .unwrap_or_else(|| panic!("no row for {dir}"))
}

/// The workspace-relative path of a fixture's source file.
fn rel(r: &Row) -> String {
    format!("{}/{}", r.dir, r.file)
}

/// The 1-based column of the first whole-word `word` on 1-based `line`.
fn column_of(r: &Row, line: u32, word: &str) -> u32 {
    let src = std::fs::read_to_string(langs().join(r.dir).join(r.file)).unwrap();
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
    panic!("`{word}` not on line {line} of {}", r.file);
}

/// A regex that matches names containing `text` literally.
fn literal_regex(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_string()
            } else {
                format!("\\{c}")
            }
        })
        .collect()
}

mod ops {
    use super::*;

    pub fn skeleton(r: &Row) {
        let result = serve_langs().ok("skeleton", json!({ "file": rel(r) }));
        assert_has(&names_of(&result, "declarations"), r.decls, "skeleton");
    }

    pub fn symbols(r: &Row) {
        let result = serve_langs().ok("symbols", json!({ "file": rel(r) }));
        assert_has(&names_of(&result, "symbols"), r.names, "symbols");
        let probe = result["symbols"]
            .as_array()
            .and_then(|l| {
                l.iter()
                    .find(|y| y["name"].as_str().is_some_and(|n| norm(n) == r.probe))
            })
            .unwrap_or_else(|| panic!("probe `{}` missing in {result}", r.probe));
        assert_eq!(
            probe["line"], r.probe_line,
            "`{}` is declared on line {}: {result}",
            r.probe, r.probe_line
        );
    }

    pub fn batch(r: &Row) {
        let mut s = serve_langs();
        let file = rel(r);
        // files: every declared name is listed, the probe with an id.
        let result = s.ok("symbols_batch", json!({ "files": [file] }));
        assert_eq!(result["results"][0]["file"], file.as_str(), "{result}");
        let symbols = result["results"][0]["symbols"]
            .as_array()
            .unwrap_or_else(|| panic!("no results[0].symbols in {result}"));
        let found: Vec<String> = symbols
            .iter()
            .map(|y| y["name"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_has(&found, r.names, "symbols_batch files");
        let probe = symbols
            .iter()
            .find(|y| y["name"].as_str().is_some_and(|n| norm(n) == r.probe))
            .unwrap_or_else(|| panic!("probe `{}` missing in {result}", r.probe));
        let id = probe["id"].as_str().expect("a batch symbol has an id");
        // kinds: the probe's kind keeps it; a kind the source lacks drops all.
        if let Some(kind) = r.kind {
            let result = s.ok("symbols_batch", json!({ "files": [file], "kinds": [kind] }));
            let only = result["results"][0]["symbols"].as_array().unwrap();
            assert!(
                only.iter().all(|y| y["kind"] == kind)
                    && only
                        .iter()
                        .any(|y| y["name"].as_str().is_some_and(|n| norm(n) == r.probe)),
                "kinds [{kind}] must keep `{}` and only kind {kind}: {result}",
                r.probe
            );
        }
        let result = s.ok(
            "symbols_batch",
            json!({ "files": [file], "kinds": ["trait"] }),
        );
        let left: usize = result["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["symbols"].as_array().map_or(0, Vec::len))
            .sum();
        assert_eq!(left, 0, "the source has no trait: {result}");
        // pattern: only names matching it stay, the probe among them.
        let result = s.ok(
            "symbols_batch",
            json!({ "files": [file], "pattern": literal_regex(r.probe) }),
        );
        let kept = names_of(&result["results"][0], "symbols");
        assert!(
            !kept.is_empty() && kept.iter().all(|n| n.contains(r.probe)),
            "pattern for `{}` must keep only matching names: {result}",
            r.probe
        );
        assert_has(&kept, &[r.probe], "symbols_batch pattern");
        // ids: the id from files selects that symbol.
        let result = s.ok("symbols_batch", json!({ "ids": [id] }));
        assert!(
            result["results"][0]["symbols"].as_array().is_some_and(|l| l
                .iter()
                .any(|y| y["name"].as_str().is_some_and(|n| norm(n) == r.probe))),
            "ids [{id}] must return `{}`: {result}",
            r.probe
        );
    }

    pub fn definition(r: &Row) {
        let column = column_of(r, r.use_line, r.target);
        let result = serve_langs().ok(
            "definition",
            json!({"symbol": r.target, "file": rel(r), "line": r.use_line, "column": column}),
        );
        let def = &result["definition"];
        assert!(
            def["file"].as_str().is_some_and(|f| f.ends_with(r.file)),
            "definition must be in {}: {result}",
            r.file
        );
        assert_eq!(def["line"], r.def_line, "definition line: {result}");
    }

    pub fn references(r: &Row) {
        let result = serve_langs().ok("references", json!({"symbol": r.target, "file": rel(r)}));
        assert!(
            result["resolved_by"]
                .as_str()
                .is_some_and(|s| s.starts_with("tree-sitter")),
            "no server runs, so the answer must say tree-sitter: {result}"
        );
        let lines: Vec<u64> = result["references"]
            .as_array()
            .unwrap_or_else(|| panic!("no references in {result}"))
            .iter()
            .filter(|x| x["file"].as_str().is_some_and(|f| f.ends_with(r.file)))
            .filter_map(|x| x["line"].as_u64())
            .collect();
        for line in [r.def_line, r.use_line] {
            assert!(
                lines.contains(&(line as u64)),
                "references must include line {line}, got {lines:?}"
            );
        }
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
                    ops::$op(row_for(stringify!($dir)));
                }
            )*
        }
    };
}

lang!(bash {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(c {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(csharp {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(cpp {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(css {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(dart {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(dockerfile {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(elixir {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(go {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(hcl {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(html {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(java {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(javascript {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(jsx {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(json {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(kotlin {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(lua {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(markdown {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(php {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(python {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(ruby {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(rust {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(scala {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(scss {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(sql {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(swift {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(toml {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(tsx {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(typescript {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

lang!(yaml {
    skeleton,
    symbols,
    batch,
    definition,
    references,
});

// ---- initialize, tools/list ----

/// The eight tools of the README's `mcp serve` section.
const README_TOOLS: [&str; 8] = [
    "callers",
    "definition",
    "references",
    "repo_outline",
    "skeleton",
    "status",
    "symbols",
    "symbols_batch",
];

#[test]
fn initialize_answers_as_act_lite() {
    let empty = tempfile::tempdir().unwrap();
    let setup = Setup {
        cwd: langs(),
        workspace: Some(langs()),
        path: empty.path().to_path_buf(),
        lsp: false,
    };
    let (_session, init) = Session::start(&setup, None);
    assert_eq!(init["jsonrpc"], "2.0", "{init}");
    let result = &init["result"];
    assert_eq!(result["serverInfo"]["name"], "act-lite", "{init}");
    assert_eq!(
        result["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION"),
        "{init}"
    );
    assert!(result["capabilities"]["tools"].is_object(), "{init}");
    assert!(result["protocolVersion"].is_string(), "{init}");
}

#[test]
fn tools_list_is_the_readme_eight() {
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md"))
            .unwrap();
    for name in README_TOOLS {
        assert!(readme.contains(&format!("`{name}`")), "README lacks {name}");
    }
    let reply = serve_langs().request("tools/list", json!({}));
    let tools = reply["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("no tools in {reply}"));
    let mut names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    names.sort();
    assert_eq!(names, README_TOOLS, "{reply}");
    for tool in tools {
        assert!(
            tool["description"].as_str().is_some_and(|d| !d.is_empty()),
            "{tool}"
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
}

#[test]
fn unknown_tool_is_an_error() {
    let message = serve_langs().err("no_such_tool", json!({}));
    assert!(!message.is_empty(), "the error must carry a message");
}

// ---- status ----

/// The README's 30 languages.
const README_LANGUAGES: [&str; 30] = [
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
];

/// The 15 language-server binaries of `crates/act-lite-lsp/src/servers.rs`.
const SERVER_BINARIES: [&str; 15] = [
    "typescript-language-server",
    "OmniSharp",
    "pyright-langserver",
    "rust-analyzer",
    "gopls",
    "elixir-ls",
    "jdtls",
    "kotlin-language-server",
    "clangd",
    "ruby-lsp",
    "sqls",
    "intelephense",
    "metals",
    "sourcekit-lsp",
    "dart",
];

/// Starts a server with language servers enabled and `path` as `PATH`.
fn serve_with_path(path: &Path) -> Session {
    let setup = Setup {
        cwd: langs(),
        workspace: Some(langs()),
        path: path.to_path_buf(),
        lsp: true,
    };
    Session::start(&setup, None).0
}

fn status_of(path: &Path) -> Value {
    serve_with_path(path).ok("status", json!({}))
}

#[test]
fn status_lists_the_thirty_languages() {
    let empty = tempfile::tempdir().unwrap();
    let status = status_of(empty.path());
    let mut languages: Vec<&str> = status["languages"]
        .as_array()
        .unwrap_or_else(|| panic!("no languages in {status}"))
        .iter()
        .filter_map(Value::as_str)
        .collect();
    languages.sort();
    let mut expected = README_LANGUAGES.to_vec();
    expected.sort();
    assert_eq!(languages, expected, "{status}");
}

/// With nothing on `PATH`, every server is `not_installed`.
#[test]
fn status_reports_every_server_not_installed_on_an_empty_path() {
    let empty = tempfile::tempdir().unwrap();
    let status = status_of(empty.path());
    let lsp = status["lsp"]
        .as_object()
        .unwrap_or_else(|| panic!("no lsp object in {status}"));
    let mut keys: Vec<&str> = lsp.keys().map(String::as_str).collect();
    keys.sort();
    let mut expected = SERVER_BINARIES.to_vec();
    expected.sort();
    assert_eq!(keys, expected, "{status}");
    for (binary, state) in lsp {
        assert_eq!(state, "not_installed", "{binary} is not on PATH: {status}");
    }
}

/// Executables for `gopls`, `clangd` and `dart` on `PATH`: exactly those three
/// are `ready`. A file named `sqls` without the execute bit is not on `PATH`.
#[cfg(unix)]
#[test]
fn status_reports_ready_exactly_for_binaries_on_path() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    for (name, mode) in [
        ("gopls", 0o755),
        ("clangd", 0o755),
        ("dart", 0o755),
        ("sqls", 0o644),
    ] {
        let p = dir.path().join(name);
        std::fs::write(&p, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    let status = status_of(dir.path());
    let lsp = status["lsp"].as_object().unwrap();
    for binary in SERVER_BINARIES {
        let want = if ["gopls", "clangd", "dart"].contains(&binary) {
            "ready"
        } else {
            "not_installed"
        };
        assert_eq!(lsp[binary], want, "{binary}: {status}");
    }
}

// ---- skeleton, symbols ----

#[test]
fn skeleton_errors() {
    let mut s = serve_langs();
    let message = s.err("skeleton", json!({ "file": "python/nope.py" }));
    assert!(
        message.contains("python/nope.py") && message.contains("does not exist"),
        "{message}"
    );
    let message = s.err("skeleton", json!({ "file": "python" }));
    assert!(message.contains("single source file"), "{message}");
    let message = s.err("skeleton", json!({ "file": "" }));
    assert!(message.contains("single source file"), "{message}");
    // An extension act-lite does not support: names it and lists the 30.
    let message = serve_in(&fixtures()).err("skeleton", json!({ "file": "sample.cob" }));
    assert!(message.contains(".cob"), "{message}");
    for language in README_LANGUAGES {
        assert!(message.contains(language), "{language} missing: {message}");
    }
}

#[test]
fn symbols_params_limit_pattern_and_kind() {
    let mut s = serve_langs();
    let result = s.ok("symbols", json!({"file": "python/fixture.py", "limit": 1}));
    assert_eq!(
        result["symbols"].as_array().map(Vec::len),
        Some(1),
        "{result}"
    );
    let result = s.ok(
        "symbols",
        json!({"file": "python/fixture.py", "pattern": "^bo"}),
    );
    assert_eq!(
        names_of(&result, "symbols"),
        ["Box"],
        "case-insensitive: {result}"
    );
    let result = s.ok(
        "symbols",
        json!({"file": "python/fixture.py", "kind": ["class"]}),
    );
    assert_eq!(names_of(&result, "symbols"), ["Box"], "{result}");
}

#[test]
fn symbols_errors() {
    let mut s = serve_langs();
    let message = s.err("symbols", json!({ "file": "python" }));
    assert!(message.contains("single source file"), "{message}");
    let message = s.err("symbols", json!({ "file": "" }));
    assert!(message.contains("single source file"), "{message}");
    let message = s.err("symbols", json!({ "file": "python/nope.py" }));
    assert!(message.contains("does not exist"), "{message}");
    let message = s.err(
        "symbols",
        json!({"file": "python/fixture.py", "pattern": "("}),
    );
    assert!(message.to_lowercase().contains("regex"), "{message}");
}

// ---- symbols_batch ----

#[test]
fn symbols_batch_over_several_files_with_filters() {
    let mut s = serve_langs();
    let files = json!(["python/fixture.py", "rust/fixture.rs"]);
    let result = s.ok("symbols_batch", json!({ "files": files }));
    let groups = result["results"].as_array().unwrap();
    assert_eq!(groups.len(), 2, "{result}");
    assert_has(
        &names_of(&groups[0], "symbols"),
        &["add", "Box", "open"],
        "python",
    );
    assert_has(
        &names_of(&groups[1], "symbols"),
        &["Point", "add", "main"],
        "rust",
    );

    // pattern keeps only names that match it, in every file.
    let result = s.ok("symbols_batch", json!({"files": files, "pattern": "^add$"}));
    for group in result["results"].as_array().unwrap() {
        assert_eq!(names_of(group, "symbols"), ["add"], "{result}");
    }

    // kinds keeps only that kind.
    let result = s.ok(
        "symbols_batch",
        json!({"files": ["python/fixture.py"], "kinds": ["class"]}),
    );
    assert_eq!(
        names_of(&result["results"][0], "symbols"),
        ["Box"],
        "{result}"
    );

    // ids select the symbol an earlier answer named.
    let id = "python/fixture.py::add#function";
    let result = s.ok("symbols_batch", json!({ "ids": [id] }));
    assert_eq!(
        names_of(&result["results"][0], "symbols"),
        ["add"],
        "{result}"
    );
}

/// `total_symbols` counts the symbols the answer lists, after `pattern`.
#[test]
fn symbols_batch_pattern_total_counts_returned_symbols() {
    let result = serve_langs().ok(
        "symbols_batch",
        json!({"files": ["python/fixture.py", "rust/fixture.rs"], "pattern": "^add$"}),
    );
    let listed: usize = result["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["symbols"].as_array().map_or(0, Vec::len))
        .sum();
    assert_eq!(listed, 2, "{result}");
    assert_eq!(result["total_symbols"], 2, "{result}");
}

#[test]
fn symbols_batch_errors() {
    let mut s = serve_langs();
    let message = s.err("symbols_batch", json!({}));
    assert!(
        message.contains("files") && message.contains("ids"),
        "{message}"
    );
    let message = s.err(
        "symbols_batch",
        json!({"files": ["python/fixture.py"], "pattern": "("}),
    );
    assert!(message.to_lowercase().contains("pattern"), "{message}");
    // A bad file is reported by name; the good file still answers.
    let result = s.ok(
        "symbols_batch",
        json!({"files": ["python/fixture.py", "python/nope.py"]}),
    );
    assert_eq!(
        result["results"].as_array().map(Vec::len),
        Some(1),
        "{result}"
    );
    let errors = result["path_errors"].as_array().unwrap();
    assert_eq!(errors[0]["file"], "python/nope.py", "{result}");
    assert!(
        errors[0]["error"]
            .as_str()
            .unwrap()
            .contains("does not exist"),
        "{result}"
    );
}

// ---- repo_outline ----

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
fn repo_outline_lists_every_fixture() {
    let result = serve_langs().ok("repo_outline", json!({"max_files": 1000}));
    assert_eq!(result["total_files"], 30, "{result}");
    assert_eq!(result["truncated"], false, "{result}");
    let paths = paths_of(&result);
    for r in ROWS {
        assert!(paths.contains(&rel(r)), "{} missing from {paths:?}", rel(r));
    }
}

#[test]
fn repo_outline_max_files_truncates() {
    let result = serve_langs().ok("repo_outline", json!({"max_files": 5}));
    assert_eq!(paths_of(&result).len(), 5, "{result}");
    assert_eq!(result["truncated"], true, "{result}");
}

#[test]
fn repo_outline_include_and_exclude() {
    let mut s = serve_langs();
    let result = s.ok("repo_outline", json!({"include": "*.py"}));
    assert_eq!(paths_of(&result), ["python/fixture.py"], "{result}");
    let result = s.ok(
        "repo_outline",
        json!({"exclude": "*.py", "max_files": 1000}),
    );
    let paths = paths_of(&result);
    assert_eq!(paths.len(), 29, "{paths:?}");
    assert!(paths.iter().all(|p| !p.ends_with(".py")), "{paths:?}");
}

#[test]
fn repo_outline_symbols_lists_top_level_names() {
    let mut s = serve_langs();
    let result = s.ok("repo_outline", json!({"include": "*.rs", "symbols": true}));
    let symbols: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols in {result}"))
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for want in ["Point", "add", "main"] {
        assert!(symbols.contains(&want), "{want} missing in {symbols:?}");
    }
    let result = s.ok("repo_outline", json!({"include": "*.rs"}));
    assert!(result["files"][0].get("symbols").is_none(), "{result}");
}

#[test]
fn repo_outline_depth_and_skipped_directories() {
    let ws = tree();
    let mut s = serve_in(ws.path());
    let result = s.ok("repo_outline", json!({}));
    let mut paths = paths_of(&result);
    paths.sort();
    assert_eq!(paths, ["a/b/deep.py", "a/mid.py", "top.py"], "{result}");
    let result = s.ok("repo_outline", json!({"depth": 1}));
    let paths = paths_of(&result);
    assert!(paths.contains(&"top.py".to_string()), "{paths:?}");
    assert!(!paths.iter().any(|p| p.ends_with("deep.py")), "{paths:?}");
}

#[test]
fn repo_outline_path_and_relative() {
    let mut s = serve_langs();
    let result = s.ok("repo_outline", json!({"path": "python"}));
    assert_eq!(paths_of(&result), ["python/fixture.py"], "{result}");
    let result = s.ok("repo_outline", json!({"path": "python", "relative": true}));
    assert_eq!(paths_of(&result), ["fixture.py"], "{result}");
}

#[test]
fn repo_outline_errors() {
    let mut s = serve_langs();
    let message = s.err("repo_outline", json!({"path": "../"}));
    assert!(message.contains("inside the workspace"), "{message}");
    let message = s.err("repo_outline", json!({"path": "nope"}));
    assert!(message.contains("does not exist"), "{message}");
    let message = s.err("repo_outline", json!({"path": "python/fixture.py"}));
    assert!(message.contains("not a directory"), "{message}");
}

// ---- definition ----

#[test]
fn definition_errors() {
    let mut s = serve_langs();
    // Line 3 of python/fixture.py is blank.
    let message = s.err(
        "definition",
        json!({"symbol": "add", "file": "python/fixture.py", "line": 3, "column": 1}),
    );
    assert!(message.contains("No symbol found at"), "{message}");
    // The name at 7:16 is `add`; the fixture declares no `zzz`.
    let message = s.err(
        "definition",
        json!({"symbol": "zzz", "file": "python/fixture.py", "line": 7, "column": 16}),
    );
    assert!(message.contains("zzz"), "{message}");
    let message = s.err(
        "definition",
        json!({"symbol": "add", "file": "python/nope.py", "line": 7, "column": 16}),
    );
    assert!(message.contains("does not exist"), "{message}");
}

// ---- references ----

#[test]
fn references_errors() {
    let mut s = serve_langs();
    let message = s.err(
        "references",
        json!({"symbol": "zzz", "file": "python/fixture.py"}),
    );
    assert!(
        message.contains("zzz") && message.contains("python/fixture.py"),
        "{message}"
    );
    let message = s.err(
        "references",
        json!({"symbol": "add", "file": "python/nope.py"}),
    );
    assert!(message.contains("does not exist"), "{message}");
    let message =
        serve_in(&fixtures()).err("references", json!({"symbol": "add", "file": "sample.cob"}));
    assert!(message.contains(".cob"), "{message}");
}

// ---- callers ----

/// With language servers enabled and none on `PATH`, `callers` fails naming
/// the Python server binary, and installs nothing.
#[test]
fn callers_without_server_names_the_binary() {
    let empty = tempfile::tempdir().unwrap();
    let message = serve_with_path(empty.path()).err(
        "callers",
        json!({"symbol": "add", "file": "python/fixture.py"}),
    );
    assert!(message.contains("pyright-langserver"), "{message}");
    assert_eq!(std::fs::read_dir(empty.path()).unwrap().count(), 0);
}

#[test]
fn callers_with_servers_disabled_says_so() {
    let message = serve_langs().err(
        "callers",
        json!({"symbol": "add", "file": "python/fixture.py"}),
    );
    assert!(message.to_lowercase().contains("disabled"), "{message}");
}

#[test]
fn callers_errors_on_bad_input() {
    let mut s = serve_langs();
    let message = s.err(
        "callers",
        json!({"symbol": "zzz", "file": "python/fixture.py"}),
    );
    assert!(message.contains("zzz"), "{message}");
    let message = s.err(
        "callers",
        json!({"symbol": "add", "file": "python/nope.py"}),
    );
    assert!(message.contains("does not exist"), "{message}");
}

/// A stand-in language server for Python on `PATH`: it answers
/// `prepareCallHierarchy` with `add` and `incomingCalls` with the method
/// `open` of `Box`, declared on line 6 of the fixture (0-based line 5).
#[cfg(unix)]
const FAKE_PYRIGHT: &str = r#"
import json, sys

def read():
    n = None
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.strip()
        if not line:
            break
        k, v = line.split(b":", 1)
        if k.lower() == b"content-length":
            n = int(v)
    return json.loads(sys.stdin.buffer.read(n))

def send(o):
    b = json.dumps(o).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
    sys.stdout.buffer.flush()

def item(uri, name, line):
    pos = lambda l, c: {"line": l, "character": c}
    return {"name": name, "kind": 12, "uri": uri,
            "range": {"start": pos(line, 0), "end": pos(line + 1, 0)},
            "selectionRange": {"start": pos(line, 4), "end": pos(line, 4 + len(name))}}

uri = None
while True:
    m = read()
    if m is None:
        break
    method = m.get("method")
    if method == "exit":
        break
    if "id" not in m or method is None:
        continue
    result = None
    if method == "initialize":
        result = {"capabilities": {"callHierarchyProvider": True, "referencesProvider": True, "textDocumentSync": 1}}
    elif method == "textDocument/prepareCallHierarchy":
        uri = m["params"]["textDocument"]["uri"]
        result = [item(uri, "add", 0)]
    elif method == "callHierarchy/incomingCalls":
        result = [{"from": item(uri, "open", 5), "fromRanges": []}]
    send({"jsonrpc": "2.0", "id": m["id"], "result": result})
"#;

/// A directory holding `script` as an executable `pyright-langserver`.
#[cfg(unix)]
fn fake_pyright(script: &str) -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let python3 = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|d| d.join("python3"))
        .find(|p| p.is_file())
        .expect("the fake language server needs python3 on PATH");
    let dir = tempfile::tempdir().unwrap();
    let server = dir.path().join("pyright-langserver");
    std::fs::write(&server, format!("#!{}\n{script}", python3.display())).unwrap();
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

#[cfg(unix)]
#[test]
fn callers_through_a_language_server_returns_the_caller() {
    let dir = fake_pyright(FAKE_PYRIGHT);
    let result = serve_with_path(dir.path()).ok(
        "callers",
        json!({"symbol": "add", "file": "python/fixture.py"}),
    );
    assert!(
        result["resolved_by"]
            .as_str()
            .is_some_and(|s| s.contains("pyright")),
        "{result}"
    );
    let callers = result["callers"].as_array().unwrap();
    assert_eq!(callers.len(), 1, "{result}");
    assert_eq!(callers[0]["name"], "open", "{result}");
    assert_eq!(
        callers[0]["line"], 6,
        "`open` is declared on line 6: {result}"
    );
    assert!(
        callers[0]["file"]
            .as_str()
            .unwrap()
            .ends_with("python/fixture.py"),
        "{result}"
    );
}

#[cfg(unix)]
#[test]
fn callers_the_server_does_not_advertise_is_an_invalid_request() {
    let script = FAKE_PYRIGHT.replace("\"callHierarchyProvider\": True, ", "");
    assert_ne!(script, FAKE_PYRIGHT);
    let dir = fake_pyright(&script);
    let reply = serve_with_path(dir.path()).call(
        "callers",
        json!({"symbol": "add", "file": "python/fixture.py"}),
    );
    let unavailable = tempfile::tempdir().unwrap();
    let unavailable = serve_with_path(unavailable.path()).call(
        "callers",
        json!({"symbol": "add", "file": "python/fixture.py"}),
    );
    assert_eq!(
        reply["error"]["code"], unavailable["error"]["code"],
        "{reply} / {unavailable}"
    );
    assert_eq!(
        reply["error"]["message"],
        "the language server does not support callers: its initialize result does not \
         advertise callHierarchyProvider",
        "{reply}"
    );
}

// ---- mcp serve with no --workspace ----

/// A session from `cwd` with no `--workspace`.
fn serve_from(cwd: &Path) -> Session {
    let empty = tempfile::tempdir().unwrap();
    let setup = Setup {
        cwd: cwd.to_path_buf(),
        workspace: None,
        path: empty.path().to_path_buf(),
        lsp: false,
    };
    Session::start(&setup, Some(empty)).0
}

fn root_of(status: &Value) -> PathBuf {
    std::fs::canonicalize(
        status["root"]
            .as_str()
            .unwrap_or_else(|| panic!("no root in {status}")),
    )
    .unwrap()
}

#[test]
fn default_workspace_is_the_nearest_git_root() {
    let repo = tempfile::tempdir().unwrap();
    let init = Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(repo.path())
        .status()
        .unwrap();
    assert!(init.success());
    let inner = repo.path().join("sub/dir");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(repo.path().join("top.py"), "def f():\n    pass\n").unwrap();
    std::fs::write(inner.join("x.py"), "def g():\n    pass\n").unwrap();
    let mut s = serve_from(&inner);
    let status = s.ok("status", json!({}));
    assert_eq!(
        root_of(&status),
        std::fs::canonicalize(repo.path()).unwrap(),
        "{status}"
    );
    // The root decides what `repo_outline` walks.
    let result = s.ok("repo_outline", json!({}));
    let mut paths = paths_of(&result);
    paths.sort();
    assert_eq!(paths, ["sub/dir/x.py", "top.py"], "{result}");
}

#[test]
fn default_workspace_without_git_is_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("sub");
    std::fs::create_dir(&inner).unwrap();
    let status = serve_from(&inner).ok("status", json!({}));
    assert_eq!(
        root_of(&status),
        std::fs::canonicalize(&inner).unwrap(),
        "{status}"
    );
}
