//! Acceptance tests of the `impact` analyzer, run through the built binary
//! (`act-lite --workspace <dir> analyze impact ...`); the cancellation checks
//! that need a token run on the library.
//! Every workspace is synthetic and built in a temporary directory; expected
//! dependents, depths and chains come from the fixture sources and the
//! traversal rules, not from the analyzer's output.

use act_lite_cli::analyze::deps::supports;
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::envelope::AnalyzeError;
use act_lite_cli::analyze::impact::ImpactAnalyzer;
use act_lite_cli::analyze::read::{FileSource, Parsed, Reader, SupportReader};
use act_lite_cli::analyze::run::{Analyzer, Request, RunContext, run};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// The JS/TS fixture: seven analyzed files, a CSS file and a test file.
fn ts(root: &Path) {
    fs::create_dir_all(root.join(".git")).unwrap();
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

/// The unresolved-import fixture.
fn unres(root: &Path, with_importer: bool) {
    fs::create_dir_all(root.join(".git")).unwrap();
    write(root, "src/t.ts", "export const t = 1;\n");
    if with_importer {
        write(root, "src/u.ts", "import { t } from './t-old';\n");
    }
}

/// One hundred files, each importing the next.
fn chain(root: &Path) {
    fs::create_dir_all(root.join(".git")).unwrap();
    for i in 0..100 {
        let content = if i < 99 {
            format!("import './f{:03}';\n", i + 1)
        } else {
            "export const x = 1;\n".to_string()
        };
        write(root, &format!("src/f{i:03}.ts"), &content);
    }
}

struct Call<'a> {
    paths: &'a [&'a str],
    focus: &'a [&'a str],
    max_depth: Option<u32>,
    include_tests: bool,
    limit: u32,
}

fn call<'a>(focus: &'a [&'a str]) -> Call<'a> {
    Call {
        paths: &[],
        focus,
        max_depth: None,
        include_tests: false,
        limit: 1000,
    }
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

/// Runs the `impact` analysis and returns the envelope, or the error text.
fn impact_with(root: &Path, c: &Call) -> Result<Value, String> {
    let mut args: Vec<String> = c.paths.iter().map(|s| s.to_string()).collect();
    for f in c.focus {
        args.extend(["--focus".to_string(), f.to_string()]);
    }
    if let Some(n) = c.max_depth {
        args.extend(["--max-depth".to_string(), n.to_string()]);
    }
    if c.include_tests {
        args.push("--include-tests".to_string());
    }
    args.extend(["--limit".to_string(), c.limit.to_string()]);
    binary(root, "impact", &args)
}

fn impact(root: &Path, focus: &[&str]) -> Result<Value, String> {
    impact_with(root, &call(focus))
}

fn ok(result: Result<Value, String>) -> Value {
    result.unwrap_or_else(|err| panic!("unexpected error: {err}"))
}

/// The findings as (file, depth), in result order.
fn listed(v: &Value) -> Vec<(String, u64)> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["file"].as_str().unwrap().to_string(),
                f["depth"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, u64)]) -> Vec<(String, u64)> {
    list.iter().map(|(f, d)| (f.to_string(), *d)).collect()
}

#[test]
fn the_dependents_come_in_depth_order_with_their_chains() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(impact(ws.path(), &["src/c.ts"]));
    assert_eq!(
        listed(&v),
        pairs(&[("src/main.ts", 1), ("src/a.ts", 2), ("src/view.tsx", 3)])
    );
    let view = &v["findings"][2];
    assert_eq!(view["reaches"], "src/c.ts");
    assert_eq!(view["chain_total"], 3);
    assert_eq!(
        view["chain"],
        json!([
            {"file": "src/view.tsx", "range": "2:1-2:28", "kind": "import",
             "specifier": "./a.js", "to": "src/a.ts"},
            {"file": "src/a.ts", "range": "1:1-1:31", "kind": "import",
             "specifier": "./main", "to": "src/main.ts"},
            {"file": "src/main.ts", "range": "5:11-5:28", "kind": "require",
             "specifier": "./c.js", "to": "src/c.ts"},
        ])
    );
    let main = &v["findings"][0];
    assert_eq!(main["reaches"], "src/c.ts");
    assert_eq!(main["chain_total"], 1);
    assert_eq!(main["chain"].as_array().unwrap().len(), 1);
    let mut keys: Vec<&str> = view
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["chain", "chain_total", "depth", "file", "reaches"]);
    assert_eq!(v["status"], "findings");
    assert_eq!(v["complete"], false);
    assert_eq!(v["total"], 3);
    let details = &v["details"];
    assert_eq!(details["direct"], 1);
    assert_eq!(details["transitive"], 2);
    assert_eq!(details["max_depth"], 1000);
    assert_eq!(details["focus"], json!(["src/c.ts"]));
    assert_eq!(details["focus_total"], 1);
    assert_eq!(details["focus_files"], 1);
    assert_eq!(details["focus_not_analyzed"], json!([]));
    assert_eq!(details["focus_not_analyzed_total"], 0);
    assert_eq!(details["focus_files_not_analyzed"], json!([]));
    assert_eq!(details["focus_files_not_analyzed_total"], 0);
    assert_eq!(v["incomplete"]["counts"]["unresolved"], 2);
}

#[test]
fn an_unresolved_import_is_never_read_as_no_dependents() {
    let ws = tempfile::tempdir().unwrap();
    unres(ws.path(), true);
    let v = ok(impact(ws.path(), &["src/t.ts"]));
    assert_eq!(v["status"], "incomplete");
    assert_eq!(v["findings"], json!([]));
    assert_eq!(v["complete"], false);
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
    let ws = tempfile::tempdir().unwrap();
    unres(ws.path(), false);
    let v = ok(impact(ws.path(), &["src/t.ts"]));
    assert_eq!(v["status"], "no_findings");
    assert_eq!(v["complete"], true);
    assert_eq!(v["findings"], json!([]));
}

#[test]
fn a_long_chain_is_cut_at_twenty_steps_and_the_depth_bound_is_disclosed() {
    let ws = tempfile::tempdir().unwrap();
    chain(ws.path());
    let v = ok(impact(ws.path(), &["src/f099.ts"]));
    let found = listed(&v);
    assert_eq!(found.len(), 99);
    assert_eq!(found[0], ("src/f098.ts".to_string(), 1));
    assert_eq!(found[98], ("src/f000.ts".to_string(), 99));
    for (i, (file, depth)) in found.iter().enumerate() {
        assert_eq!(file, &format!("src/f{:03}.ts", 98 - i));
        assert_eq!(*depth, i as u64 + 1);
    }
    let last = &v["findings"][98];
    assert_eq!(last["chain_total"], 99);
    assert_eq!(last["chain"].as_array().unwrap().len(), 20);
    for i in 0..20 {
        let step = &last["chain"][i];
        assert_eq!(step["file"], format!("src/f{i:03}.ts"), "step {i}");
        assert_eq!(step["to"], format!("src/f{:03}.ts", i + 1), "step {i}");
    }
    assert_eq!(last["chain"][19]["to"], "src/f020.ts");
    assert_eq!(last["reaches"], "src/f099.ts");
    assert_eq!(v["complete"], true);
    assert_eq!(v["status"], "findings");
    assert_eq!(v["incomplete"]["limits_hit"], json!([]));
    assert_eq!(v["details"]["direct"], 1);
    assert_eq!(v["details"]["transitive"], 98);

    let mut c = call(&["src/f099.ts"]);
    c.max_depth = Some(10);
    let v = ok(impact_with(ws.path(), &c));
    let found = listed(&v);
    assert_eq!(found.len(), 10);
    assert_eq!(found[0], ("src/f098.ts".to_string(), 1));
    assert_eq!(found[9], ("src/f089.ts".to_string(), 10));
    assert_eq!(
        v["incomplete"]["limits_hit"],
        json!([{
            "limit": "max_depth",
            "value": 10,
            "detail": "1 files at depth 10 have dependents that were not followed",
        }])
    );
    assert_eq!(v["complete"], false);
    assert_eq!(v["status"], "findings");
    assert_eq!(v["details"]["max_depth"], 10);
}

#[test]
fn a_bound_that_reaches_every_dependent_is_not_a_limit_hit() {
    let ws = tempfile::tempdir().unwrap();
    chain(ws.path());
    let mut c = call(&["src/f099.ts"]);
    c.max_depth = Some(99);
    let v = ok(impact_with(ws.path(), &c));
    assert_eq!(listed(&v).len(), 99);
    assert_eq!(v["incomplete"]["limits_hit"], json!([]));
    assert_eq!(v["complete"], true);
}

#[test]
fn test_files_are_dependents_only_with_include_tests() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(impact(ws.path(), &["src/a.ts"]));
    assert_eq!(
        listed(&v),
        pairs(&[("src/main.ts", 1), ("src/view.tsx", 1)])
    );
    assert_eq!(v["excluded"]["tests"], json!([{"path": "src/a.test.ts"}]));
    let mut c = call(&["src/a.ts"]);
    c.include_tests = true;
    let v = ok(impact_with(ws.path(), &c));
    assert_eq!(
        listed(&v),
        pairs(&[
            ("src/a.test.ts", 1),
            ("src/main.ts", 1),
            ("src/view.tsx", 1)
        ])
    );
    assert_eq!(v["excluded"]["tests"], json!([]));
}

#[test]
fn a_focus_file_that_is_not_analyzed_still_has_dependents() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(impact(ws.path(), &["src/style.css"]));
    assert_eq!(listed(&v), pairs(&[("src/view.tsx", 1)]));
    let details = &v["details"];
    assert_eq!(
        details["focus_files_not_analyzed"],
        json!([{"file": "src/style.css", "reason": "language not supported: CSS"}])
    );
    assert_eq!(details["focus_files_not_analyzed_total"], 1);
    assert_eq!(details["focus_not_analyzed"], json!(["src/style.css"]));
    assert_eq!(details["focus_not_analyzed_total"], 1);
    assert_eq!(details["focus_files"], 1);
    assert_eq!(v["complete"], false);
    assert_eq!(v["status"], "findings");
}

#[test]
fn a_focus_file_that_depends_on_another_focus_file_is_not_a_dependent() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(impact(ws.path(), &["src/a.ts", "src/main.ts"]));
    assert_eq!(listed(&v), pairs(&[("src/view.tsx", 1)]));
    assert_eq!(v["findings"][0]["reaches"], "src/a.ts");
    assert_eq!(v["details"]["focus_files"], 2);
}

#[test]
fn request_errors_are_the_documented_texts() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    assert_eq!(
        impact(ws.path(), &[]).unwrap_err(),
        "impact needs at least one focus path"
    );
    let mut c = call(&["src/a.ts"]);
    c.max_depth = Some(0);
    assert_eq!(
        impact_with(ws.path(), &c).unwrap_err(),
        "max_depth must be between 1 and 1000, got 0"
    );
    c.max_depth = Some(1001);
    assert_eq!(
        impact_with(ws.path(), &c).unwrap_err(),
        "max_depth must be between 1 and 1000, got 1001"
    );
    // The range check comes before the focus rows.
    let mut c = call(&[]);
    c.max_depth = Some(0);
    assert_eq!(
        impact_with(ws.path(), &c).unwrap_err(),
        "max_depth must be between 1 and 1000, got 0"
    );
    let mut c = call(&["src/a.ts"]);
    c.paths = &["lib"];
    let err = impact_with(ws.path(), &c).unwrap_err();
    assert!(err.contains("lib"), "{err}");
    let mut c = call(&["src/a.ts"]);
    c.paths = &["src/lib"];
    assert_eq!(
        impact_with(ws.path(), &c).unwrap_err(),
        "focus path is not inside the requested paths: src/a.ts"
    );
    let err = impact(ws.path(), &["nope.ts"]).unwrap_err();
    assert!(err.contains("nope.ts"), "{err}");
}

#[test]
fn parameter_errors_are_invalid_requests() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let request = Request::new(ws.path(), &[], &[], &[], Includes::default(), 100).unwrap();
    let focus = ["src/a.ts".to_string()];
    for (focus, max_depth) in [(&focus[..], Some(0)), (&[][..], None)] {
        let Err(err) = ImpactAnalyzer::new(&request, focus, max_depth) else {
            panic!("an error");
        };
        assert!(matches!(err, AnalyzeError::InvalidRequest(_)), "{err:?}");
    }
}

#[test]
fn an_unresolved_relationship_of_another_family_is_not_relevant() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    write(ws.path(), "tools/gen.py", "import nowhere_declared\n");
    let v = ok(impact(ws.path(), &["src/c.ts"]));
    let targets: Vec<&str> = v["incomplete"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, ["./missing", "name"]);
    assert_eq!(v["incomplete"]["counts"]["unresolved"], 2);
    assert_eq!(v["details"]["relationships"]["unresolved"], 3);

    // With nothing else missing the same Python entry leaves the result complete.
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws.path().join(".git")).unwrap();
    write(ws.path(), "src/t.ts", "export const t = 1;\n");
    write(ws.path(), "tools/gen.py", "import nowhere_declared\n");
    let v = ok(impact(ws.path(), &["src/t.ts"]));
    assert_eq!(v["status"], "no_findings");
    assert_eq!(v["complete"], true);
    assert_eq!(v["incomplete"]["unresolved"], json!([]));
}

#[test]
fn a_cancelled_token_stops_the_analysis() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let request = Request::new(ws.path(), &[], &[], &[], Includes::default(), 100).unwrap();
    let analyzer = ImpactAnalyzer::new(&request, &["src/c.ts".to_string()], None).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = run(&analyzer, &request, &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

/// A file source that cancels the token when it has no file left to give.
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
fn a_token_cancelled_after_the_last_read_stops_the_analysis() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let root = ws.path();
    let cancel = CancellationToken::new();
    let request = Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap();
    let analyzer = ImpactAnalyzer::new(&request, &["src/c.ts".to_string()], None).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut files = CancelAtEnd {
        inner: Reader::new(&ctx.discovered.files, &supports, false, &cancel),
        cancel: cancel.clone(),
    };
    let result = analyzer.analyze_in(&mut files, &ctx, &cancel);
    assert!(cancel.is_cancelled());
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

/// Two focus files, `x` and `y`: `p` imports `x`; `s` imports `p` and `y`;
/// `t` imports `s`.
fn shared(root: &Path) {
    fs::create_dir_all(root.join(".git")).unwrap();
    write(root, "src/x.ts", "export const x = 1;\n");
    write(root, "src/y.ts", "export const y = 1;\n");
    write(root, "src/p.ts", "import './x';\n");
    write(root, "src/s.ts", "import './p';\nimport './y';\n");
    write(root, "src/t.ts", "import './s';\n");
}

#[test]
fn a_dependent_of_two_focus_files_has_the_smaller_depth() {
    let ws = tempfile::tempdir().unwrap();
    shared(ws.path());
    let v = ok(impact(ws.path(), &["src/x.ts", "src/y.ts"]));
    assert_eq!(
        listed(&v),
        pairs(&[("src/p.ts", 1), ("src/s.ts", 1), ("src/t.ts", 2)])
    );
    assert_eq!(v["details"]["direct"], 2);
    assert_eq!(v["details"]["transitive"], 1);
}

#[test]
fn the_limit_cuts_findings_but_not_the_dependent_counts() {
    let ws = tempfile::tempdir().unwrap();
    shared(ws.path());
    let mut c = call(&["src/x.ts", "src/y.ts"]);
    c.limit = 1;
    let v = ok(impact_with(ws.path(), &c));
    assert_eq!(listed(&v), pairs(&[("src/p.ts", 1)]));
    assert_eq!(v["total"], 3);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["details"]["direct"], 2);
    assert_eq!(v["details"]["transitive"], 1);
}

#[test]
fn no_text_written_by_the_analyzer_claims_a_break_or_safety() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let mut results = vec![
        ok(impact(ws.path(), &["src/c.ts"])),
        ok(impact(ws.path(), &["src/a.ts"])),
        ok(impact(ws.path(), &["src/style.css"])),
    ];
    let mut c = call(&["src/c.ts"]);
    c.max_depth = Some(1);
    results.push(ok(impact_with(ws.path(), &c)));
    let ws = tempfile::tempdir().unwrap();
    unres(ws.path(), true);
    results.push(ok(impact(ws.path(), &["src/t.ts"])));
    let ws = tempfile::tempdir().unwrap();
    chain(ws.path());
    let mut c = call(&["src/f099.ts"]);
    c.max_depth = Some(10);
    results.push(ok(impact_with(ws.path(), &c)));
    for v in results {
        let text = serde_json::to_string(&v).unwrap().to_lowercase();
        assert!(!text.contains("break"), "{text}");
        assert!(!text.contains("safe"), "{text}");
    }
}
