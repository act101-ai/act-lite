//! `analyze_cycles`: the strongly connected components of the file graph,
//! each with a shortest witness cycle through its first file.
//! Every case runs the built binary on a synthetic workspace; the cases that
//! need a cancellation token run on the library.

use act_lite_cli::analyze::cycles::{CyclesAnalyzer, witness};
use act_lite_cli::analyze::deps::{Edge, Evidence, supports};
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::envelope::AnalyzeError;
use act_lite_cli::analyze::read::{FileSource, Parsed, Reader, SupportReader};
use act_lite_cli::analyze::run::{Analyzer, Request, RunContext, run};
use act_lite_core::CompactRange;
use act_lite_parser::deps::RelationshipKind;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// A workspace with an empty `.git/` and `files` (path, content).
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    dir
}

fn request_for(root: &Path, paths: &[String]) -> Request {
    Request::new(root, paths, &[], &[], Includes::default(), 100).unwrap()
}

/// Runs the binary as `act-lite --workspace <root> analyze <name> <args>` with
/// an empty configuration directory. Exit 0 gives the parsed stdout; exit 1
/// gives the message of the error frame, after the frame is checked.
fn binary(root: &Path, name: &str, args: &[String]) -> Result<Value, String> {
    let config = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_act-lite"))
        .env("ACT_CONFIG_DIR", config.path())
        .env_remove("ACT_LITE_DISABLE_LSP")
        .arg("--workspace")
        .arg(root)
        .args(["analyze", name])
        .args(args)
        .output()
        .unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    match out.status.code() {
        Some(0) => Ok(serde_json::from_slice(&out.stdout).expect("stdout is one JSON value")),
        Some(1) => {
            assert!(out.stdout.is_empty(), "stdout of an error");
            let v: Value = serde_json::from_str(&stderr).expect("stderr is one JSON value");
            let message = v["error"]["message"].as_str().unwrap().to_string();
            let frame = json!({"error": {"message": message, "chain": []}});
            assert_eq!(stderr, format!("{frame}\n"));
            Err(message)
        }
        code => panic!("exit status {code:?}: {stderr}"),
    }
}

fn cycles_with(root: &Path, args: &[String]) -> Value {
    binary(root, "cycles", args).unwrap_or_else(|e| panic!("unexpected error: {e}"))
}

fn cycles_in(root: &Path, paths: &[String]) -> Value {
    cycles_with(root, paths)
}

fn cycles(root: &Path) -> Value {
    cycles_in(root, &[])
}

const TS_PACKAGE: &str =
    r#"{"name":"ts-fixture","private":true,"dependencies":{"react":"18.3.1"}}"#;

fn ts_fixture() -> Vec<(&'static str, &'static str)> {
    vec![
        ("package.json", TS_PACKAGE),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@lib/*":["src/lib/*"]}}}"#,
        ),
        (
            "src/main.ts",
            "import { a } from './a';\nimport { b } from '@lib/b';\nimport fs from 'node:fs';\nimport React from 'react';\nconst c = require('./c.js');\nexport { d } from './d';\nimport type { T } from './types';\nconst m = import('./missing');\nconst n = require(name);\n",
        ),
        ("src/a.ts", "import { main } from './main';\n"),
        ("src/lib/b.ts", "export const b = 1;\n"),
        ("src/c.ts", "export const c = 1;\n"),
        ("src/d/index.ts", "export const d = 1;\n"),
        ("src/types.d.ts", "export type T = number;\n"),
        ("src/style.css", ".x {}\n"),
        (
            "src/view.tsx",
            "import './style.css';\nimport { a } from './a.js';\n",
        ),
        ("src/a.test.ts", "import { a } from './a';\n"),
    ]
}

fn acyclic_fixture() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "src/a.ts",
            "import { b } from './b';\nimport fs from 'node:fs';\n",
        ),
        ("src/b.ts", "import { c } from './c';\n"),
        ("src/c.ts", "export const c = 1;\n"),
        ("src/d.ts", "import './b';\nimport './c';\n"),
    ]
}

fn files_of(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap())
        .collect()
}

#[test]
fn the_acyclic_fixture_has_no_cycle_on_a_complete_result() {
    let ws = workspace(&acyclic_fixture());
    let out = cycles(ws.path());
    assert_eq!(out["analysis"], "cycles");
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["complete"], true);
    assert_eq!(out["findings"], json!([]));
    assert_eq!(out["details"]["cyclic_files"], 0);
}

#[test]
fn the_ts_fixture_has_one_cycle_and_an_incomplete_result() {
    let ws = workspace(&ts_fixture());
    let out = cycles(ws.path());
    assert_eq!(out["status"], "findings");
    assert_eq!(out["complete"], false);
    assert_eq!(out["total"], 1);
    assert_eq!(
        out["findings"],
        json!([{
            "file": "src/a.ts",
            "members": [{"file": "src/a.ts"}, {"file": "src/main.ts"}],
            "members_total": 2,
            "edges_total": 2,
            "path": [
                {"file": "src/a.ts", "range": "1:1-1:31", "kind": "import",
                 "specifier": "./main", "to": "src/main.ts"},
                {"file": "src/main.ts", "range": "1:1-1:25", "kind": "import",
                 "specifier": "./a", "to": "src/a.ts"},
            ],
            "path_total": 2,
        }])
    );
    assert_eq!(out["details"]["cyclic_files"], 2);
    assert_eq!(
        out["details"]["kinds_total"],
        out["details"]["kinds"].as_array().unwrap().len()
    );
    let unresolved = out["incomplete"]["unresolved"].as_array().unwrap();
    assert_eq!(unresolved.len(), 2);
    assert_eq!(
        out["details"]["not_followed"],
        json!([{"file": "src/style.css"}])
    );
    assert_eq!(out["details"]["not_followed_total"], 1);
}

#[test]
fn an_unresolved_relationship_hides_a_possible_cycle() {
    let ws = workspace(&[
        ("src/t.ts", "import './u';\n"),
        ("src/u.ts", "import { t } from './t-old';\n"),
    ]);
    let out = cycles(ws.path());
    assert_eq!(out["status"], "incomplete");
    assert_eq!(out["complete"], false);
    assert_eq!(out["findings"], json!([]));
}

#[test]
fn the_witness_is_the_shortest_cycle_through_the_first_file() {
    let ws = workspace(&[
        ("src/a.ts", "import './b';\n"),
        ("src/b.ts", "import './c';\nimport './a';\n"),
        ("src/c.ts", "import './a';\n"),
    ]);
    let out = cycles(ws.path());
    assert_eq!(out["total"], 1);
    let finding = &out["findings"][0];
    assert_eq!(finding["members_total"], 3);
    assert_eq!(finding["edges_total"], 4);
    assert_eq!(
        finding["path"],
        json!([
            {"file": "src/a.ts", "range": "1:1-1:14", "kind": "import",
             "specifier": "./b", "to": "src/b.ts"},
            {"file": "src/b.ts", "range": "2:1-2:14", "kind": "import",
             "specifier": "./a", "to": "src/a.ts"},
        ])
    );
    assert_eq!(finding["path_total"], 2);
}

#[test]
fn findings_are_ordered_by_size_then_file() {
    let ws = workspace(&[
        ("src/a.ts", "import './b';\n"),
        ("src/b.ts", "import './a';\n"),
        ("src/c.ts", "import './d';\n"),
        ("src/d.ts", "import './e';\n"),
        ("src/e.ts", "import './c';\n"),
    ]);
    let out = cycles(ws.path());
    let findings = out["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["file"], "src/c.ts");
    assert_eq!(findings[0]["members_total"], 3);
    assert_eq!(findings[1]["file"], "src/a.ts");
    assert_eq!(findings[1]["members_total"], 2);
    assert_eq!(out["details"]["cyclic_files"], 5);
}

#[test]
fn the_limit_cuts_findings_but_not_the_totals() {
    let ws = workspace(&[
        ("src/a.ts", "import './b';\n"),
        ("src/b.ts", "import './a';\n"),
        ("src/c.ts", "import './d';\n"),
        ("src/d.ts", "import './e';\n"),
        ("src/e.ts", "import './c';\n"),
    ]);
    let out = cycles_with(ws.path(), &["--limit".to_string(), "1".to_string()]);
    let findings = out["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["file"], "src/c.ts");
    assert_eq!(out["total"], 2);
    assert_eq!(out["details"]["cyclic_files"], 5);
}

fn crate_files(lib: &'static str, a: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "Cargo.toml",
            "[package]\nname = \"c\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        ("src/lib.rs", lib),
        ("src/a.rs", a),
    ]
}

#[test]
fn rust_containment_is_no_cycle_until_the_parent_uses_the_child() {
    let ws = workspace(&crate_files("mod a;\npub struct X;\n", "use crate::X;\n"));
    let out = cycles(ws.path());
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["complete"], true);

    let ws = workspace(&crate_files(
        "mod a;\npub use a::Y;\npub struct X;\n",
        "use crate::X;\npub struct Y;\n",
    ));
    let out = cycles(ws.path());
    assert_eq!(out["status"], "findings");
    assert_eq!(out["complete"], true);
    assert_eq!(out["total"], 1);
    let finding = &out["findings"][0];
    assert_eq!(files_of(&finding["members"]), ["src/a.rs", "src/lib.rs"]);
    assert_eq!(finding["edges_total"], 2);
    assert_eq!(
        finding["path"],
        json!([
            {"file": "src/a.rs", "range": "1:1-1:14", "kind": "use",
             "specifier": "crate::X", "to": "src/lib.rs"},
            {"file": "src/lib.rs", "range": "1:1-1:7", "kind": "module",
             "specifier": "a", "to": "src/a.rs"},
        ])
    );
    assert_eq!(finding["path_total"], 2);
}

#[test]
fn a_cycle_of_sixty_files_is_bounded_to_twenty_entries() {
    let contents: Vec<(String, String)> = (0..60)
        .map(|i| {
            (
                format!("src/g{i:02}.ts"),
                format!("import './g{:02}';\n", (i + 1) % 60),
            )
        })
        .collect();
    let files: Vec<(&str, &str)> = contents
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let ws = workspace(&files);
    let out = cycles(ws.path());
    let finding = &out["findings"][0];
    let first_twenty: Vec<String> = (0..20).map(|i| format!("src/g{i:02}.ts")).collect();
    assert_eq!(files_of(&finding["members"]), first_twenty);
    assert_eq!(finding["members_total"], 60);
    let path = finding["path"].as_array().unwrap();
    assert_eq!(path.len(), 20);
    assert_eq!(path[0]["file"], "src/g00.ts");
    for (i, step) in path.iter().enumerate() {
        assert_eq!(step["file"], format!("src/g{i:02}.ts"));
        assert_eq!(step["to"], format!("src/g{:02}.ts", i + 1));
    }
    for pair in path.windows(2) {
        assert_eq!(pair[0]["to"], pair[1]["file"]);
    }
    assert_eq!(finding["path_total"], 60);
    assert_eq!(out["details"]["cyclic_files"], 60);
}

#[test]
fn a_relationship_to_the_file_itself_is_no_cycle() {
    let ws = workspace(&[("src/s.ts", "import './s';\n")]);
    let out = cycles(ws.path());
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["details"]["relationships"]["resolved"], 1);
}

#[test]
fn an_unresolved_module_relationship_alone_leaves_the_result_complete() {
    let ws = workspace(&crate_files("mod a;\nmod gone;\n", "pub struct Y;\n"));
    let out = cycles(ws.path());
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["complete"], true);
    assert_eq!(out["details"]["relationships"]["unresolved"], 1);
}

#[test]
fn a_cycle_through_a_file_outside_the_paths_is_not_reported_and_is_listed() {
    let ws = workspace(&[
        ("in/a.ts", "import '../out/b';\n"),
        ("out/b.ts", "import '../in/a';\n"),
    ]);
    let out = cycles_in(ws.path(), &["in".to_string()]);
    assert_eq!(out["status"], "no_findings");
    assert_eq!(
        out["details"]["not_followed"],
        json!([{"file": "out/b.ts"}])
    );
    assert_eq!(out["details"]["not_followed_total"], 1);
}

/// A file source that cancels the token as it reports that no file is left,
/// so the cancellation arrives after the last file read and its resolution.
struct CancelAtEnd {
    inner: Reader,
    cancel: CancellationToken,
}

impl FileSource for CancelAtEnd {
    fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
        let next = self.inner.next_file()?;
        if next.is_none() {
            self.cancel.cancel();
        }
        Ok(next)
    }

    fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
        self.inner.reread(rel)
    }
}

#[test]
fn a_cancelled_token_stops_the_component_search() {
    let ws = workspace(&[
        ("src/a.ts", "import './b';\n"),
        ("src/b.ts", "import './a';\n"),
    ]);
    let root = ws.path();
    let cancel = CancellationToken::new();
    let request = request_for(root, &[]);
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut files = CancelAtEnd {
        inner: Reader::new(&ctx.discovered.files, &supports, false, &cancel),
        cancel: cancel.clone(),
    };
    let result = CyclesAnalyzer.analyze_in(&mut files, &ctx, &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

#[test]
fn a_token_cancelled_before_the_run_gives_the_cancelled_error() {
    let ws = workspace(&acyclic_fixture());
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = run(&CyclesAnalyzer, &request_for(ws.path(), &[]), &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

#[test]
fn not_followed_lists_at_most_one_hundred_targets() {
    let imports: String = (0..101)
        .map(|i| format!("import '../out/b{i:03}';\n"))
        .collect();
    let targets: Vec<(String, String)> = (0..101)
        .map(|i| {
            (
                format!("out/b{i:03}.ts"),
                "export const b = 1;\n".to_string(),
            )
        })
        .collect();
    let mut files: Vec<(&str, &str)> = vec![("in/a.ts", imports.as_str())];
    files.extend(targets.iter().map(|(p, c)| (p.as_str(), c.as_str())));
    let ws = workspace(&files);
    let out = cycles_in(ws.path(), &["in".to_string()]);
    let listed = files_of(&out["details"]["not_followed"]);
    let expected: Vec<String> = (0..100).map(|i| format!("out/b{i:03}.ts")).collect();
    assert_eq!(listed, expected);
    assert_eq!(out["details"]["not_followed_total"], 101);
}

#[test]
fn a_module_only_edge_to_a_file_outside_the_paths_is_listed_as_not_followed() {
    let ws = workspace(&crate_files("mod a;\npub struct X;\n", "use crate::X;\n"));
    let out = cycles_in(ws.path(), &["src/lib.rs".to_string()]);
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["complete"], true);
    assert_eq!(
        out["details"]["not_followed"],
        json!([{"file": "src/a.rs"}])
    );
    assert_eq!(out["details"]["not_followed_total"], 1);
}

#[test]
fn a_cancelled_token_stops_the_path_search() {
    let (edges, out) = two_cycles(1);
    let refs: Vec<&Edge> = edges.iter().collect();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = witness(&out, &refs, &[0, 1], &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

/// `pairs` independent two-file cycles: node `2k` and node `2k + 1` import
/// each other, and edge `n` leaves node `n`.
fn two_cycles(pairs: usize) -> (Vec<Edge>, Vec<Vec<(usize, usize)>>) {
    let n = 2 * pairs;
    let edges = (0..n)
        .map(|i| Edge {
            file: format!("src/f{i:05}.js"),
            to: format!("src/f{:05}.js", i ^ 1),
            to_analyzed: true,
            kinds: BTreeSet::from([RelationshipKind::Import]),
            evidence: vec![Evidence {
                range: CompactRange::new(1, 1, 1, 14),
                kind: RelationshipKind::Import,
                specifier: "./x".to_string(),
            }],
            evidence_total: 1,
        })
        .collect();
    let out = (0..n).map(|i| vec![(i ^ 1, i)]).collect();
    (edges, out)
}

#[test]
fn the_path_search_costs_the_component_not_the_graph() {
    let pairs = 10_000;
    let (edges, out) = two_cycles(pairs);
    let refs: Vec<&Edge> = edges.iter().collect();
    let cancel = CancellationToken::new();
    let start = std::time::Instant::now();
    for k in 0..pairs {
        let path = witness(&out, &refs, &[2 * k, 2 * k + 1], &cancel).unwrap();
        assert_eq!(path.len(), 2);
    }
    let elapsed = start.elapsed();
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: ACT_LITE_CHECK_PERF not set");
        return;
    }
    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "{pairs} path searches took {elapsed:?}"
    );
}
