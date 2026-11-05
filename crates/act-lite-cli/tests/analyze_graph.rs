//! Acceptance tests of the `graph` analyzer, run through the library runner.
//! Every workspace is synthetic and built in a temporary directory; expected
//! edges, depths and limits come from the fixture sources and the traversal
//! rules, not from the analyzer's output.

use act_lite_cli::analyze::deps::DepGraph;
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::envelope::AnalyzeError;
use act_lite_cli::analyze::focus::{FocusSet, check_focus, focus_files};
use act_lite_cli::analyze::graph::{Direction, GraphAnalyzer, traverse};
use act_lite_cli::analyze::read::{Reader, SupportReader};
use act_lite_cli::analyze::run::{Request, RunContext, run};
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

/// The fixture's edges, numbered 1 to 8 in source-then-target order.
const EDGES: [(&str, &str); 8] = [
    ("src/a.ts", "src/main.ts"),
    ("src/main.ts", "src/a.ts"),
    ("src/main.ts", "src/c.ts"),
    ("src/main.ts", "src/d/index.ts"),
    ("src/main.ts", "src/lib/b.ts"),
    ("src/main.ts", "src/types.d.ts"),
    ("src/view.tsx", "src/a.ts"),
    ("src/view.tsx", "src/style.css"),
];

/// Runs the `graph` analysis and returns the envelope, or the error text.
fn graph(
    root: &Path,
    paths: &[&str],
    focus: &[&str],
    direction: Option<&str>,
    max_depth: Option<u32>,
) -> Result<Value, String> {
    let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
    let focus: Vec<String> = focus.iter().map(|s| s.to_string()).collect();
    let request = Request::new(root, &paths, &[], &[], Includes::default(), 1000)
        .map_err(|e| e.to_string())?;
    let analyzer =
        GraphAnalyzer::new(&request, &focus, direction, max_depth).map_err(|e| e.to_string())?;
    let envelope =
        run(&analyzer, &request, &CancellationToken::new()).map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(envelope).unwrap())
}

fn ok(result: Result<Value, String>) -> Value {
    result.unwrap_or_else(|err| panic!("unexpected error: {err}"))
}

/// The findings as (edge number, depth), in result order.
fn numbered(v: &Value) -> Vec<(usize, Value)> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let pair = (f["file"].as_str().unwrap(), f["to"].as_str().unwrap());
            let n = EDGES
                .iter()
                .position(|e| *e == pair)
                .unwrap_or_else(|| panic!("unexpected edge {pair:?}"));
            (n + 1, f["depth"].clone())
        })
        .collect()
}

fn at(depth: u32, edges: &[usize]) -> Vec<(usize, Value)> {
    edges.iter().map(|n| (*n, json!(depth))).collect()
}

fn max_depth_hit(value: u32, files: u32) -> Value {
    json!([{
        "limit": "max_depth",
        "value": value,
        "detail": format!("{files} files at depth {value} have relationships that were not followed"),
    }])
}

#[test]
fn without_focus_every_edge_is_reported_without_depth() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(graph(ws.path(), &[], &[], None, None));
    let all: Vec<(usize, Value)> = (1..=8).map(|n| (n, Value::Null)).collect();
    assert_eq!(numbered(&v), all);
    assert_eq!(v["status"], "findings");
    assert_eq!(v["complete"], false);
    assert_eq!(v["total"], 8);
    let lines: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let range = f["evidence"][0]["range"].as_str().unwrap();
            range.split(':').next().unwrap()
        })
        .collect();
    assert_eq!(lines, ["1", "1", "5", "6", "2", "7", "2", "1"]);
    let to_analyzed: Vec<bool> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["to_analyzed"].as_bool().unwrap())
        .collect();
    assert_eq!(
        to_analyzed,
        [true, true, true, true, true, true, true, false]
    );
    let unresolved = &v["incomplete"]["unresolved"];
    assert_eq!(v["incomplete"]["counts"]["unresolved"], 2);
    assert_eq!(unresolved[0]["target"], "./missing");
    assert_eq!(unresolved[0]["kind"], "dynamic_import");
    assert_eq!(unresolved[0]["reason"], "no file matches ./missing");
    assert!(unresolved[0]["range"].as_str().unwrap().starts_with("8:"));
    assert_eq!(unresolved[1]["target"], "name");
    assert_eq!(unresolved[1]["kind"], "require");
    assert_eq!(unresolved[1]["reason"], "specifier is not a string literal");
    assert!(unresolved[1]["range"].as_str().unwrap().starts_with("9:"));
    let details = &v["details"];
    assert_eq!(
        details["relationships"],
        json!({"total": 12, "resolved": 8, "external": 2, "unresolved": 2})
    );
    assert_eq!(
        details["kinds"],
        json!([
            "dynamic_import",
            "import",
            "reexport",
            "require",
            "type_import"
        ])
    );
    assert_eq!(
        details["resolution_inputs"],
        json!([
            {"file": "package.json", "kind": "package.json"},
            {"file": "tsconfig.json", "kind": "tsconfig"}
        ])
    );
    assert_eq!(details["files"], 8);
}

#[test]
fn out_from_a_focus_file_reports_the_depth_and_the_limit() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(graph(ws.path(), &[], &["src/main.ts"], Some("out"), None));
    assert_eq!(numbered(&v), at(1, &[2, 3, 4, 5, 6]));
    assert_eq!(v["incomplete"]["limits_hit"], max_depth_hit(1, 1));
    let v = ok(graph(
        ws.path(),
        &[],
        &["src/main.ts"],
        Some("out"),
        Some(2),
    ));
    let mut expected = at(1, &[2, 3, 4, 5, 6]);
    expected.extend(at(2, &[1]));
    assert_eq!(numbered(&v), expected);
    assert_eq!(v["incomplete"]["limits_hit"], json!([]));
}

#[test]
fn in_and_both_follow_dependents_and_report_the_union() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(graph(ws.path(), &[], &["src/a.ts"], Some("in"), None));
    assert_eq!(numbered(&v), at(1, &[2, 7]));
    assert_eq!(v["incomplete"]["limits_hit"], max_depth_hit(1, 1));
    let v = ok(graph(ws.path(), &[], &["src/a.ts"], Some("both"), None));
    assert_eq!(numbered(&v), at(1, &[1, 2, 7]));
    assert_eq!(v["incomplete"]["limits_hit"], max_depth_hit(1, 1));
    let v = ok(graph(ws.path(), &[], &["src/a.ts"], Some("both"), Some(2)));
    let mut expected = at(1, &[1, 2, 7]);
    expected.extend(at(2, &[3, 4, 5, 6]));
    assert_eq!(numbered(&v), expected);
    assert_eq!(v["incomplete"]["limits_hit"], json!([]));
    let default = ok(graph(ws.path(), &[], &["src/a.ts"], None, Some(2)));
    assert_eq!(default["details"]["direction"], "both");
    assert_eq!(default["findings"], v["findings"]);
}

#[test]
fn a_target_outside_the_analyzed_set_is_reported_and_not_followed() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(graph(
        ws.path(),
        &[],
        &["src/view.tsx"],
        Some("out"),
        Some(2),
    ));
    let mut expected = at(1, &[7, 8]);
    expected.extend(at(2, &[1]));
    assert_eq!(numbered(&v), expected);
    assert_eq!(
        v["details"]["not_followed"],
        json!([{"file": "src/style.css"}])
    );
    assert_eq!(v["details"]["not_followed_total"], 1);
    assert_eq!(v["incomplete"]["limits_hit"], max_depth_hit(2, 1));
}

#[test]
fn unresolved_relationships_count_only_where_the_traversal_reads() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(graph(ws.path(), &[], &["src/c.ts"], Some("out"), None));
    assert_eq!(numbered(&v), []);
    assert_eq!(v["incomplete"]["counts"]["unresolved"], 0);
    assert_eq!(v["complete"], true);
    assert_eq!(v["status"], "no_findings");
    let v = ok(graph(ws.path(), &[], &["src/c.ts"], Some("in"), None));
    assert_eq!(numbered(&v), at(1, &[3]));
    let targets: Vec<&str> = v["incomplete"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, ["./missing", "name"]);
}

#[test]
fn request_errors_in_their_order() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let root = ws.path();
    let err = |r: Result<Value, String>| r.expect_err("an error");
    assert_eq!(
        err(graph(root, &[], &["src/a.ts"], None, Some(0))),
        "max_depth must be between 1 and 1000, got 0"
    );
    assert_eq!(
        err(graph(root, &[], &["src/a.ts"], None, Some(1001))),
        "max_depth must be between 1 and 1000, got 1001"
    );
    assert_eq!(
        err(graph(root, &[], &[], None, Some(2))),
        "max_depth needs a focus path"
    );
    assert_eq!(
        err(graph(root, &[], &[], Some("in"), None)),
        "direction needs a focus path"
    );
    assert_eq!(
        err(graph(root, &[], &["src/a.ts"], Some("up"), None)),
        "direction must be one of in, out, both, got up"
    );
    assert_eq!(
        err(graph(root, &[], &[], Some("up"), Some(0))),
        "max_depth must be between 1 and 1000, got 0"
    );
    assert_eq!(
        err(graph(root, &[], &[], Some("up"), Some(2))),
        "direction must be one of in, out, both, got up"
    );
    assert_eq!(
        err(graph(root, &[], &[], Some("in"), Some(2))),
        "max_depth needs a focus path"
    );
    assert_eq!(
        err(graph(root, &[], &["src/style.css"], Some("out"), None)),
        "no focus file can be analyzed: src/style.css (language not supported: CSS)"
    );
    assert_eq!(
        err(graph(root, &["src/lib"], &["src/a.ts"], None, None)),
        "focus path is not inside the requested paths: src/a.ts"
    );
}

#[test]
fn details_hold_every_key_also_when_empty() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let v = ok(graph(ws.path(), &[], &[], None, None));
    let details = v["details"].as_object().unwrap();
    for key in [
        "relationships",
        "kinds",
        "resolution_inputs",
        "resolution_inputs_total",
        "direction",
        "max_depth",
        "focus",
        "focus_total",
        "focus_files",
        "focus_not_analyzed",
        "focus_not_analyzed_total",
        "focus_files_not_analyzed",
        "focus_files_not_analyzed_total",
        "files",
        "not_followed",
        "not_followed_total",
    ] {
        assert!(details.contains_key(key), "details has no `{key}`");
    }
    assert_eq!(details["direction"], Value::Null);
    assert_eq!(details["max_depth"], Value::Null);
    assert_eq!(details["focus"], json!([]));
    assert_eq!(details["focus_total"], 0);
    assert_eq!(details["focus_files"], 0);
    assert_eq!(details["not_followed"], json!([]));
    let v = ok(graph(ws.path(), &[], &["src/main.ts"], Some("out"), None));
    let d = &v["details"];
    assert_eq!(d["direction"], "out");
    assert_eq!(d["max_depth"], 1);
    assert_eq!(d["focus"], json!(["src/main.ts"]));
    assert_eq!(d["focus_total"], 1);
    assert_eq!(d["focus_files"], 1);
    assert_eq!(d["focus_not_analyzed"], json!([]));
    assert_eq!(d["focus_files_not_analyzed"], json!([]));
    assert_eq!(d["files"], 6);
    let v = ok(graph(ws.path(), &[], &["src/style.css"], Some("in"), None));
    let d = &v["details"];
    assert_eq!(
        d["focus_files_not_analyzed"],
        json!([{"file": "src/style.css", "reason": "language not supported: CSS"}])
    );
    assert_eq!(d["focus_files_not_analyzed_total"], 1);
    assert_eq!(d["focus_not_analyzed"], json!(["src/style.css"]));
    assert_eq!(d["focus_not_analyzed_total"], 1);
}

#[test]
fn focus_lists_are_capped_at_thirty() {
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws.path().join(".git")).unwrap();
    write(ws.path(), "src/x.ts", "export const x = 1;\n");
    let mut focus = Vec::new();
    for i in 0..31 {
        write(ws.path(), &format!("d{i:02}/s.css"), ".x {}\n");
        focus.push(format!("d{i:02}"));
    }
    let focus: Vec<&str> = focus.iter().map(String::as_str).collect();
    let v = ok(graph(ws.path(), &[], &focus, Some("in"), None));
    let d = &v["details"];
    assert_eq!(d["focus"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_total"], 31);
    assert_eq!(d["focus"][0], "d00/");
    assert_eq!(d["focus_files"], 31);
    assert_eq!(d["focus_files_not_analyzed"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_files_not_analyzed_total"], 31);
    assert_eq!(d["focus_not_analyzed"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_not_analyzed_total"], 31);
}

#[test]
fn not_followed_is_capped_at_one_hundred() {
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws.path().join(".git")).unwrap();
    let mut source = String::new();
    for i in 0..101 {
        write(ws.path(), &format!("src/c{i:03}.css"), ".x {}\n");
        source.push_str(&format!("import './c{i:03}.css';\n"));
    }
    write(ws.path(), "src/x.ts", &source);
    let v = ok(graph(ws.path(), &[], &["src/x.ts"], Some("out"), None));
    let d = &v["details"];
    assert_eq!(d["not_followed"].as_array().unwrap().len(), 100);
    assert_eq!(d["not_followed_total"], 101);
    assert_eq!(d["not_followed"][0], json!({"file": "src/c000.css"}));
    assert_eq!(d["files"], 102);
    assert_eq!(v["total"], 101);
}

#[test]
fn the_no_focus_text_lists_ten_files_and_empty_directories() {
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws.path().join(".git")).unwrap();
    write(ws.path(), "src/x.ts", "export const x = 1;\n");
    let mut focus = Vec::new();
    for i in 0..11 {
        write(ws.path(), &format!("css/s{i:02}.css"), ".x {}\n");
        focus.push(format!("css/s{i:02}.css"));
    }
    let focus: Vec<&str> = focus.iter().map(String::as_str).collect();
    let err = graph(ws.path(), &[], &focus, Some("out"), None).unwrap_err();
    let listed: Vec<String> = (0..10)
        .map(|i| format!("css/s{i:02}.css (language not supported: CSS)"))
        .collect();
    assert_eq!(
        err,
        format!("no focus file can be analyzed: {}, …", listed.join(", "))
    );
    fs::create_dir_all(ws.path().join("empty")).unwrap();
    assert_eq!(
        graph(ws.path(), &[], &["empty"], Some("out"), None).unwrap_err(),
        "no focus file can be analyzed: empty/ (no file)"
    );
}

#[test]
fn an_empty_scope_wins_over_the_no_focus_error() {
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws.path().join(".git")).unwrap();
    write(ws.path(), "src/a.test.ts", "export const t = 1;\n");
    let err = graph(ws.path(), &[], &["src"], Some("out"), None).unwrap_err();
    assert!(
        err.starts_with("no files to analyze in the requested scope"),
        "{err}"
    );
}

/// The model of the `ts` fixture and the focus set of `focus`.
fn model(
    root: &Path,
    focus: &[&str],
    cancel: &CancellationToken,
) -> (DepGraph, act_lite_cli::analyze::focus::FocusSet) {
    let request = Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap();
    let supported = act_lite_cli::analyze::deps::supports;
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supported, cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supported, false, cancel);
    let graph = DepGraph::build(&mut reader, &ctx, cancel).unwrap();
    let focus: Vec<String> = focus.iter().map(|s| s.to_string()).collect();
    let paths = check_focus(root, &request.scope, &focus).unwrap();
    let set = focus_files(
        root,
        &paths,
        &ctx.discovered,
        &Includes::default(),
        &supported,
        cancel,
    )
    .unwrap();
    (graph, set)
}

#[test]
fn traverse_keeps_distances_first_edges_and_both_limit_counts() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let cancel = CancellationToken::new();
    let (graph, set) = model(ws.path(), &["src/main.ts"], &cancel);
    let out = traverse(&graph, &set, Direction::Out, 1, &cancel).unwrap();
    let reported: Vec<(&str, &str, u32)> = out
        .edges
        .iter()
        .map(|t| (t.edge.file.as_str(), t.edge.to.as_str(), t.depth))
        .collect();
    assert_eq!(
        reported,
        [
            ("src/main.ts", "src/a.ts", 1),
            ("src/main.ts", "src/c.ts", 1),
            ("src/main.ts", "src/d/index.ts", 1),
            ("src/main.ts", "src/lib/b.ts", 1),
            ("src/main.ts", "src/types.d.ts", 1),
        ]
    );
    assert_eq!(out.distance["src/main.ts"], 0);
    assert_eq!(out.distance["src/a.ts"], 1);
    assert_eq!(out.first_edge["src/a.ts"].file, "src/main.ts");
    assert_eq!(out.edge_limit_hit, Some(1));
    assert_eq!(out.file_limit_hit, None);
    assert!(out.not_followed.is_empty());

    let (graph, set) = model(ws.path(), &["src/c.ts"], &cancel);
    let dependents = traverse(&graph, &set, Direction::In, 1, &cancel).unwrap();
    assert_eq!(dependents.distance["src/main.ts"], 1);
    assert_eq!(dependents.first_edge["src/main.ts"].to, "src/c.ts");
    assert_eq!(dependents.file_limit_hit, Some(1));
    let deeper = traverse(&graph, &set, Direction::In, 3, &cancel).unwrap();
    assert_eq!(deeper.distance["src/a.ts"], 2);
    assert_eq!(deeper.distance["src/view.tsx"], 3);
    assert_eq!(deeper.file_limit_hit, None);
}

#[test]
fn a_cancelled_token_stops_the_traversal() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let (graph, set) = model(ws.path(), &["src/a.ts"], &CancellationToken::new());
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = traverse(&graph, &set, Direction::Both, 2, &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

#[test]
fn directions_parse_from_their_names() {
    assert_eq!("in".parse::<Direction>(), Ok(Direction::In));
    assert_eq!("out".parse::<Direction>(), Ok(Direction::Out));
    assert_eq!("both".parse::<Direction>(), Ok(Direction::Both));
    assert!("up".parse::<Direction>().is_err());
}

#[test]
fn parameter_errors_are_invalid_requests() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let request = Request::new(ws.path(), &[], &[], &[], Includes::default(), 100).unwrap();
    let focus = vec!["src/a.ts".to_string()];
    let cases: [(&[String], Option<&str>, Option<u32>); 4] = [
        (&focus, None, Some(0)),
        (&[], None, Some(2)),
        (&[], Some("in"), None),
        (&focus, Some("up"), None),
    ];
    for (focus, direction, max_depth) in cases {
        let Err(err) = GraphAnalyzer::new(&request, focus, direction, max_depth) else {
            panic!("an error");
        };
        assert!(matches!(err, AnalyzeError::InvalidRequest(_)), "{err:?}");
    }
    let css = vec!["src/style.css".to_string()];
    let analyzer = GraphAnalyzer::new(&request, &css, Some("out"), None).unwrap();
    let Err(err) = run(&analyzer, &request, &CancellationToken::new()) else {
        panic!("an error");
    };
    assert!(matches!(err, AnalyzeError::InvalidRequest(_)), "{err:?}");
}

#[test]
fn a_cancelled_token_stops_the_traversal_before_its_sort() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let (graph, _) = model(ws.path(), &["src/a.ts"], &CancellationToken::new());
    let nothing = FocusSet {
        paths: Vec::new(),
        files: Vec::new(),
        paths_not_analyzed: Vec::new(),
    };
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = traverse(&graph, &nothing, Direction::Both, 1, &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
}

#[test]
fn the_limit_cuts_findings_but_not_the_file_count() {
    let ws = tempfile::tempdir().unwrap();
    ts(ws.path());
    let request = Request::new(ws.path(), &[], &[], &[], Includes::default(), 3).unwrap();
    let analyzer = GraphAnalyzer::new(&request, &[], None, None).unwrap();
    let envelope = run(&analyzer, &request, &CancellationToken::new()).unwrap();
    let v = serde_json::to_value(envelope).unwrap();
    assert_eq!(
        numbered(&v),
        (1..=3).map(|n| (n, Value::Null)).collect::<Vec<_>>()
    );
    assert_eq!(v["total"], 8);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["details"]["files"], 8);
}
