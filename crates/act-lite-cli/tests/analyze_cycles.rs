//! `analyze_cycles`: the strongly connected components of the file graph,
//! each with a shortest witness cycle through its first file.
//! Every case runs the analyzer on a synthetic workspace.

use act_lite_cli::analyze::cycles::CyclesAnalyzer;
use act_lite_cli::analyze::deps::supports;
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::envelope::AnalyzeError;
use act_lite_cli::analyze::read::{FileSource, Parsed, Reader, SupportReader};
use act_lite_cli::analyze::run::{Analyzer, Request, RunContext, run};
use serde_json::{Value, json};
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

fn cycles_in(root: &Path, paths: &[String]) -> Value {
    let envelope = run(
        &CyclesAnalyzer,
        &request_for(root, paths),
        &CancellationToken::new(),
    )
    .unwrap();
    serde_json::to_value(envelope).unwrap()
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
    let path = finding["path"].as_array().unwrap();
    let hops: Vec<(&str, &str)> = path
        .iter()
        .map(|s| (s["file"].as_str().unwrap(), s["to"].as_str().unwrap()))
        .collect();
    assert_eq!(hops, [("src/a.ts", "src/b.ts"), ("src/b.ts", "src/a.ts")]);
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
    assert_eq!(out["total"], 1);
    assert_eq!(
        files_of(&out["findings"][0]["members"]),
        ["src/a.rs", "src/lib.rs"]
    );
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
    assert_eq!(finding["members_total"], 60);
    assert_eq!(finding["members"].as_array().unwrap().len(), 20);
    assert_eq!(finding["path_total"], 60);
    assert_eq!(finding["path"].as_array().unwrap().len(), 20);
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
