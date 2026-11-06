//! End-to-end tests of the `graph`, `analyze_cycles`, `analyze_coupling` and
//! `analyze_impact` MCP tools, run against the built `act-lite mcp serve`
//! over stdio and compared with `act-lite analyze` on the same workspace.
//! Every workspace is synthetic and built in a temporary directory; expected
//! values come from the fixture sources and the documented rules, not from
//! the binary's output.
#![cfg(unix)]

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A temporary workspace with an empty `.git/`.
fn workspace() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

// ---- the server ----

/// A running `act-lite --workspace <ws> mcp serve` and the lines it has sent.
struct Session {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
    next_id: u64,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Session {
    /// Starts the server with language servers disabled and performs the
    /// `initialize` handshake.
    fn start(ws: &Path) -> Session {
        let config = TempDir::new().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_act-lite"))
            .env("ACT_CONFIG_DIR", config.path())
            .env_remove("ACT_LITE_DISABLE_LSP")
            .arg("--disable-lsp")
            .arg("--workspace")
            .arg(ws)
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
        };
        session.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}),
        );
        session.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        session
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Sends a request without waiting; returns its id.
    fn send_request(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        id
    }

    /// Reads replies until the one for `id`; returns it and the ids of the
    /// replies before it.
    fn wait_for(&mut self, id: u64, timeout: Duration) -> (Value, Vec<Value>) {
        let mut before = Vec::new();
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = self
                .replies
                .recv_timeout(left)
                .unwrap_or_else(|e| panic!("no reply for id {id}: {e}"));
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if v["id"] == id {
                    return (v, before);
                }
                before.push(v["id"].clone());
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params);
        self.wait_for(id, Duration::from_secs(120)).0
    }

    /// A `tools/call` of `tool` that must succeed: its two text blocks, the
    /// first parsed as JSON.
    fn ok(&mut self, tool: &str, arguments: Value) -> (Value, String) {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        assert!(
            reply.get("error").is_none() && reply["result"]["isError"] != true,
            "{tool} {arguments} must succeed: {reply}"
        );
        let content = reply["result"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2, "{reply}");
        let text = content[0]["text"].as_str().unwrap();
        let json = serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"));
        (json, content[1]["text"].as_str().unwrap().to_string())
    }
}

/// The MCP tool of each analyzer.
fn tool(analyzer: &str) -> &'static str {
    match analyzer {
        "graph" => "graph",
        "cycles" => "analyze_cycles",
        "coupling" => "analyze_coupling",
        "impact" => "analyze_impact",
        _ => unreachable!(),
    }
}

/// The parsed stdout of `act-lite --workspace <ws> analyze <name> <args>`.
fn cli(ws: &Path, name: &str, args: &[&str]) -> Value {
    let config = TempDir::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_act-lite"))
        .env("ACT_CONFIG_DIR", config.path())
        .env_remove("ACT_LITE_DISABLE_LSP")
        .arg("--disable-lsp")
        .arg("--workspace")
        .arg(ws)
        .args(["analyze", name])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("stdout is one JSON value")
}

// ---- fixtures ----

fn ts(root: &Path) {
    write(
        root,
        "package.json",
        r#"{"name":"ts-fixture","private":true,"dependencies":{"react":"18.3.1"}}"#,
    );
    write(
        root,
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@lib/*":["src/lib/*"]}}}"#,
    );
    write(
        root,
        "src/main.ts",
        "import { a } from './a';\n\
         import { b } from '@lib/b';\n\
         import fs from 'node:fs';\n\
         import React from 'react';\n\
         const c = require('./c.js');\n\
         export { d } from './d';\n\
         import type { T } from './types';\n\
         const m = import('./missing');\n\
         const n = require(name);\n",
    );
    write(root, "src/a.ts", "import { main } from './main';\n");
    write(root, "src/lib/b.ts", "export const b = 1;\n");
    write(root, "src/c.ts", "export const c = 1;\n");
    write(root, "src/d/index.ts", "export const d = 1;\n");
    write(root, "src/types.d.ts", "export type T = number;\n");
    write(root, "src/style.css", ".x {}\n");
    write(
        root,
        "src/view.tsx",
        "import './style.css';\nimport { a } from './a.js';\n",
    );
    write(root, "src/a.test.ts", "import { a } from './a';\n");
}

fn acyclic(root: &Path) {
    write(
        root,
        "src/a.ts",
        "import { b } from './b';\nimport fs from 'node:fs';\n",
    );
    write(root, "src/b.ts", "import { c } from './c';\n");
    write(root, "src/c.ts", "export const c = 1;\n");
    write(root, "src/d.ts", "import './b';\nimport './c';\n");
}

fn unres(root: &Path, with_importer: bool) {
    write(root, "src/t.ts", "export const t = 1;\n");
    if with_importer {
        write(root, "src/u.ts", "import { t } from './t-old';\n");
    }
}

/// One hundred files, each importing the next.
fn chain(root: &Path) {
    for i in 0..100 {
        let content = if i < 99 {
            format!("import './f{:03}';\n", i + 1)
        } else {
            "export const x = 1;\n".to_string()
        };
        write(root, &format!("src/f{i:03}.ts"), &content);
    }
}

// ---- equivalence ----

#[test]
fn the_first_block_equals_the_command_line_output() {
    let ts_ws = workspace();
    ts(ts_ws.path());
    let chain_ws = workspace();
    chain(chain_ws.path());
    let mut on_ts = Session::start(ts_ws.path());
    let mut on_chain = Session::start(chain_ws.path());
    let cases: [(bool, &str, Value, Vec<&str>); 7] = [
        (true, "graph", json!({}), vec![]),
        (
            true,
            "graph",
            json!({"focus": ["src/main.ts"], "direction": "out"}),
            vec!["--focus", "src/main.ts", "--direction", "out"],
        ),
        (true, "cycles", json!({}), vec![]),
        (true, "coupling", json!({}), vec![]),
        (
            true,
            "impact",
            json!({"focus": ["src/c.ts"]}),
            vec!["--focus", "src/c.ts"],
        ),
        (
            false,
            "impact",
            json!({"focus": ["src/f099.ts"], "limit": 1000}),
            vec!["--focus", "src/f099.ts", "--limit", "1000"],
        ),
        (
            false,
            "impact",
            json!({"focus": ["src/f099.ts"], "limit": 1000, "max_depth": 10}),
            vec![
                "--focus",
                "src/f099.ts",
                "--limit",
                "1000",
                "--max-depth",
                "10",
            ],
        ),
    ];
    for (is_ts, name, arguments, args) in cases {
        let (ws, session) = if is_ts {
            (ts_ws.path(), &mut on_ts)
        } else {
            (chain_ws.path(), &mut on_chain)
        };
        let (mcp, _) = session.ok(tool(name), arguments.clone());
        assert_eq!(mcp, cli(ws, name, &args), "{name} {arguments}");
    }
    // Spot checks that the compared results are the documented ones.
    let (v, _) = on_ts.ok("graph", json!({}));
    assert_eq!(v["total"], 8);
    let (v, _) = on_ts.ok("analyze_cycles", json!({}));
    assert_eq!(v["findings"][0]["file"], "src/a.ts");
    let (v, _) = on_chain.ok(
        "analyze_impact",
        json!({"focus": ["src/f099.ts"], "limit": 1000}),
    );
    assert_eq!(v["findings"].as_array().unwrap().len(), 99);
}

// ---- truncation ----

#[test]
fn a_truncated_graph_names_the_narrowing_parameters_and_not_max_depth() {
    let ws = workspace();
    ts(ws.path());
    let mut session = Session::start(ws.path());
    let (v, hint) = session.ok("graph", json!({"limit": 3}));
    assert_eq!(v["total"], 8);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["findings"].as_array().unwrap().len(), 3);
    assert_eq!(v, cli(ws.path(), "graph", &["--limit", "3"]));
    for word in ["`paths`", "`include`", "`exclude`", "`limit`", "`focus`"] {
        assert!(hint.contains(word), "{word} missing: {hint}");
    }
    assert!(!hint.contains("max_depth"), "{hint}");
    let (v, _) = session.ok("graph", json!({"limit": 0}));
    assert_eq!(v["findings"], json!([]));
    assert_eq!(v["total"], 8);
    assert_eq!(v["truncated"], true);
}

// ---- nested bounds ----

#[test]
fn nested_lists_are_bounded_over_mcp() {
    let ws = workspace();
    write(ws.path(), "src/x.ts", &"import './y';\n".repeat(7));
    write(ws.path(), "src/y.ts", "export const y = 1;\n");
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("graph", json!({}));
    assert_eq!(v["findings"][0]["evidence"].as_array().unwrap().len(), 5);
    assert_eq!(v["findings"][0]["evidence_total"], 7);

    let ws = workspace();
    for i in 0..60 {
        write(
            ws.path(),
            &format!("src/g{i:02}.ts"),
            &format!("import './g{:02}';\n", (i + 1) % 60),
        );
    }
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("analyze_cycles", json!({}));
    let f = &v["findings"][0];
    assert_eq!(f["members_total"], 60);
    assert_eq!(f["members"].as_array().unwrap().len(), 20);
    assert_eq!(f["path_total"], 60);
    assert_eq!(f["path"].as_array().unwrap().len(), 20);
}

fn check_focus_caps(v: &Value, context: &str) {
    let d = &v["details"];
    assert_eq!(d["focus"].as_array().unwrap().len(), 30, "{context}");
    assert_eq!(d["focus_total"], 31, "{context}");
    assert_eq!(d["focus"][0], "d00/", "{context}");
    assert_eq!(d["focus_files"], 31, "{context}");
    assert_eq!(d["focus_files_not_analyzed"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_files_not_analyzed_total"], 31, "{context}");
    assert_eq!(d["focus_not_analyzed"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_not_analyzed_total"], 31, "{context}");
}

#[test]
fn details_lists_are_capped_with_their_totals_over_mcp() {
    let ws = workspace();
    write(ws.path(), "src/x.ts", "export const x = 1;\n");
    let focus: Vec<String> = (0..31)
        .map(|i| {
            write(ws.path(), &format!("d{i:02}/s.css"), ".x {}\n");
            format!("d{i:02}")
        })
        .collect();
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("graph", json!({"focus": focus, "direction": "in"}));
    check_focus_caps(&v, "graph");
    let (v, _) = session.ok("analyze_impact", json!({"focus": focus}));
    check_focus_caps(&v, "impact");

    let ws = workspace();
    let mut source = String::new();
    for i in 0..101 {
        write(ws.path(), &format!("src/c{i:03}.css"), ".x {}\n");
        source.push_str(&format!("import './c{i:03}.css';\n"));
    }
    write(ws.path(), "src/x.ts", &source);
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("graph", json!({"focus": ["src/x.ts"], "direction": "out"}));
    let d = &v["details"];
    assert_eq!(d["not_followed"].as_array().unwrap().len(), 100);
    assert_eq!(d["not_followed_total"], 101);
    assert_eq!(d["not_followed"][0], json!({"file": "src/c000.css"}));

    let ws = workspace();
    let imports: String = (0..101)
        .map(|i| format!("import '../out/b{i:03}';\n"))
        .collect();
    write(ws.path(), "in/a.ts", &imports);
    for i in 0..101 {
        write(
            ws.path(),
            &format!("out/b{i:03}.ts"),
            "export const b = 1;\n",
        );
    }
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("analyze_cycles", json!({"paths": ["in"]}));
    assert_eq!(v["details"]["not_followed"].as_array().unwrap().len(), 100);
    assert_eq!(v["details"]["not_followed_total"], 101);
}

// ---- the two counterexamples ----

#[test]
fn a_diamond_of_imports_is_no_cycle_over_mcp() {
    let ws = workspace();
    acyclic(ws.path());
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("analyze_cycles", json!({}));
    assert_eq!(v["status"], "no_findings");
    assert_eq!(v["complete"], true);
    assert_eq!(v["findings"], json!([]));
}

#[test]
fn an_unresolved_import_is_never_read_as_no_dependents_over_mcp() {
    let ws = workspace();
    unres(ws.path(), true);
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("analyze_impact", json!({"focus": ["src/t.ts"]}));
    assert_eq!(v["status"], "incomplete");
    assert_eq!(v["findings"], json!([]));
    assert_eq!(
        v["incomplete"]["unresolved"],
        json!([{
            "file": "src/u.ts",
            "range": "1:1-1:29",
            "kind": "import",
            "target": "./t-old",
            "reason": "no file matches ./t-old",
        }])
    );
    let ws = workspace();
    unres(ws.path(), false);
    let mut session = Session::start(ws.path());
    let (v, _) = session.ok("analyze_impact", json!({"focus": ["src/t.ts"]}));
    assert_eq!(v["status"], "no_findings");
    assert_eq!(v["complete"], true);
}

// ---- cancellation ----

/// Files of a workspace that `analyze_cycles` takes several seconds to read:
/// 15,000 files in 100 directories, each importing 20 files of its directory.
fn slow_workspace(root: &Path) {
    let total = 15_000;
    for i in 0..total {
        let mut source = String::new();
        for k in 0..20 {
            source.push_str(&format!(
                "import {{ v{k} }} from './f{}';\n",
                (i + 100 * (k + 1)) % total
            ));
        }
        write(root, &format!("d{:03}/f{i}.ts", i % 100), &source);
    }
    let start = Instant::now();
    cli(root, "cycles", &["--limit", "0"]);
    assert!(
        start.elapsed() > Duration::from_secs(5),
        "the workspace is too small: {:?}",
        start.elapsed()
    );
}

/// CPU time of process `pid`, in seconds.
#[cfg(target_os = "linux")]
fn cpu_seconds(pid: u32) -> f64 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let fields: Vec<&str> = stat
        .rsplit(')')
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    let ticks: f64 = fields[11].parse::<f64>().unwrap() + fields[12].parse::<f64>().unwrap();
    ticks / 100.0
}

/// CPU time of process `pid`, in seconds, from `ps -o time=` (`[[h:]m:]s.cc`).
#[cfg(not(target_os = "linux"))]
fn cpu_seconds(pid: u32) -> f64 {
    let out = Command::new("ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .trim()
        .split(':')
        .fold(0.0, |acc, part| acc * 60.0 + part.parse::<f64>().unwrap())
}

#[test]
fn a_cancelled_cycle_analysis_stops_and_gets_no_response() {
    // Local only: it needs a workspace that takes more than 5 seconds to analyze.
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: ACT_LITE_CHECK_PERF not set");
        return;
    }
    let ws = workspace();
    slow_workspace(ws.path());
    let mut session = Session::start(ws.path());
    let pid = session.child.id();
    let call = session.send_request(
        "tools/call",
        json!({"name": "analyze_cycles", "arguments": {}}),
    );
    std::thread::sleep(Duration::from_millis(500));
    session.send(&json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId": call, "reason": "test"}}));
    let cancelled = Instant::now();
    std::thread::sleep(Duration::from_secs(1));
    let at_one = cpu_seconds(pid);
    std::thread::sleep(Duration::from_secs(3).saturating_sub(cancelled.elapsed()));
    let at_three = cpu_seconds(pid);
    assert!(
        at_three - at_one < 0.5,
        "the analysis kept running: {at_one} s, then {at_three} s"
    );
    let status = session.send_request("tools/call", json!({"name": "status", "arguments": {}}));
    let (_, before) = session.wait_for(status, Duration::from_secs(60));
    assert!(
        !before.contains(&json!(call)),
        "a response for the cancelled id arrived"
    );
}

#[test]
fn running_cycle_analyses_do_not_block_the_serve_loop() {
    // Local only: it needs a workspace that takes more than 5 seconds to analyze.
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: ACT_LITE_CHECK_PERF not set");
        return;
    }
    let ws = workspace();
    slow_workspace(ws.path());
    let mut session = Session::start(ws.path());
    let cores = std::thread::available_parallelism().unwrap().get();
    let calls: Vec<u64> = (0..cores)
        .map(|_| {
            session.send_request(
                "tools/call",
                json!({"name": "analyze_cycles", "arguments": {}}),
            )
        })
        .collect();
    std::thread::sleep(Duration::from_millis(500));
    let status = session.send_request("tools/call", json!({"name": "status", "arguments": {}}));
    let sent = Instant::now();
    let (reply, before) = session.wait_for(status, Duration::from_secs(60));
    assert!(
        sent.elapsed() < Duration::from_secs(1),
        "status took {:?}",
        sent.elapsed()
    );
    assert!(reply["result"].is_object(), "{reply}");
    assert!(
        calls.iter().all(|id| !before.contains(&json!(id))),
        "an analysis finished before status was answered"
    );
}
