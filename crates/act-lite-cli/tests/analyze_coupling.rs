//! `analyze_coupling`: per file, the number of files it depends on and the
//! number of analyzed files that depend on it, with the instability they give.
//! Every case runs the analyzer on a synthetic workspace.

use act_lite_cli::analyze::coupling::CouplingAnalyzer;
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

fn request(root: &Path) -> Request {
    Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap()
}

/// The result of the analyzer on `root`, as the JSON it serializes to.
fn coupling(root: &Path) -> Value {
    let envelope = run(&CouplingAnalyzer, &request(root), &CancellationToken::new()).unwrap();
    serde_json::to_value(envelope).unwrap()
}

/// One finding as (file, fan_in, fan_out, degree, instability, external, unresolved).
fn row(file: &str, numbers: [u64; 3], instability: Value, rest: [u64; 2]) -> Value {
    json!({
        "file": file,
        "fan_in": numbers[0],
        "fan_out": numbers[1],
        "degree": numbers[2],
        "instability": instability,
        "external": rest[0],
        "unresolved": rest[1],
    })
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

#[test]
fn the_ts_fixture_gives_seven_items_in_order() {
    let ws = workspace(&ts_fixture());
    let out = coupling(ws.path());
    assert_eq!(out["analysis"], "coupling");
    assert_eq!(out["status"], "findings");
    assert_eq!(
        out["findings"],
        json!([
            row("src/main.ts", [1, 5, 6], json!(0.833), [2, 2]),
            row("src/a.ts", [2, 1, 3], json!(0.333), [0, 0]),
            row("src/view.tsx", [0, 2, 2], json!(1), [0, 0]),
            row("src/c.ts", [1, 0, 1], json!(0), [0, 0]),
            row("src/d/index.ts", [1, 0, 1], json!(0), [0, 0]),
            row("src/lib/b.ts", [1, 0, 1], json!(0), [0, 0]),
            row("src/types.d.ts", [1, 0, 1], json!(0), [0, 0]),
        ])
    );
    assert_eq!(out["total"], 7);
    assert_eq!(out["coverage"]["analyzed_files"], 7);
    let details = &out["details"];
    assert_eq!(details["edges"], 8);
    assert_eq!(
        details["kinds_total"],
        details["kinds"].as_array().unwrap().len()
    );
    assert_eq!(
        details["formulas"],
        json!({
            "fan_in": "distinct analyzed files with an edge to the file",
            "fan_out": "distinct workspace files the file has an edge to",
            "degree": "fan_in + fan_out",
            "instability": "fan_out / (fan_in + fan_out)",
        })
    );
    assert_eq!(
        details["relationships"],
        json!({"total": 12, "resolved": 8, "external": 2, "unresolved": 2})
    );
}

#[test]
fn every_unresolved_relationship_is_listed_and_the_result_is_incomplete() {
    let ws = workspace(&ts_fixture());
    let out = coupling(ws.path());
    assert_eq!(out["complete"], false);
    let unresolved = out["incomplete"]["unresolved"].as_array().unwrap();
    let rows: Vec<(&str, &str, &str)> = unresolved
        .iter()
        .map(|u| {
            (
                u["file"].as_str().unwrap(),
                u["kind"].as_str().unwrap(),
                u["target"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("src/main.ts", "dynamic_import", "./missing"),
            ("src/main.ts", "require", "name"),
        ]
    );
}

#[test]
fn a_target_that_is_not_analyzed_counts_in_fan_out_but_is_no_item() {
    let ws = workspace(&ts_fixture());
    let out = coupling(ws.path());
    let files: Vec<&str> = out["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert!(!files.contains(&"src/style.css"));
    let view = &out["findings"][2];
    assert_eq!(view["file"], "src/view.tsx");
    assert_eq!(view["fan_out"], 2);
}

#[test]
fn repeated_relationships_to_one_file_count_once() {
    let ws = workspace(&[
        (
            "src/x.ts",
            "import { a } from './y';\nimport { b } from './y';\nexport * from './y';\n",
        ),
        ("src/y.ts", "export const a = 1;\nexport const b = 2;\n"),
    ]);
    let out = coupling(ws.path());
    assert_eq!(
        out["findings"],
        json!([
            row("src/y.ts", [1, 0, 1], json!(0), [0, 0]),
            row("src/x.ts", [0, 1, 1], json!(1), [0, 0]),
        ])
    );
    assert_eq!(out["details"]["edges"], 1);
}

#[test]
fn the_acyclic_fixture_counts_fan_in_on_a_complete_result() {
    let ws = workspace(&acyclic_fixture());
    let out = coupling(ws.path());
    assert_eq!(out["status"], "findings");
    assert_eq!(out["complete"], true);
    let findings = out["findings"].as_array().unwrap();
    for file in ["src/b.ts", "src/c.ts"] {
        let item = findings.iter().find(|f| f["file"] == file).unwrap();
        assert_eq!(item["fan_in"], 2, "{file}");
    }
    assert_eq!(out["details"]["edges"], 4);
}

#[test]
fn a_scope_with_no_edge_gives_no_findings() {
    let ws = workspace(&[
        ("src/a.ts", "import fs from 'node:fs';\n"),
        ("src/b.ts", "export const b = 1;\n"),
    ]);
    let out = coupling(ws.path());
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["complete"], true);
    assert_eq!(out["findings"], json!([]));
    assert_eq!(out["coverage"]["analyzed_files"], 2);
    assert_eq!(out["details"]["edges"], 0);
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
fn a_cancelled_token_stops_the_analysis_before_the_sort() {
    let ws = workspace(&acyclic_fixture());
    let root = ws.path();
    let cancel = CancellationToken::new();
    let request = request(root);
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut files = CancelAtEnd {
        inner: Reader::new(&ctx.discovered.files, &supports, false, &cancel),
        cancel: cancel.clone(),
    };
    let result = CouplingAnalyzer.analyze_in(&mut files, &ctx, &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

#[test]
fn a_token_cancelled_before_the_run_gives_the_cancelled_error() {
    let ws = workspace(&acyclic_fixture());
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = run(&CouplingAnalyzer, &request(ws.path()), &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

#[test]
fn instability_is_serialized_with_three_decimals_at_most_and_no_trailing_zeros() {
    let ws = workspace(&ts_fixture());
    let envelope = run(
        &CouplingAnalyzer,
        &request(ws.path()),
        &CancellationToken::new(),
    )
    .unwrap();
    let text = serde_json::to_string(&envelope).unwrap();
    for expected in [
        r#""instability":0.833"#,
        r#""instability":0.333"#,
        r#""instability":1,"#,
        r#""instability":0,"#,
    ] {
        assert!(text.contains(expected), "{expected} in {text}");
    }
    assert!(!text.contains(r#""instability":1.0"#));
    assert!(!text.contains(r#""instability":0.0"#));
}
