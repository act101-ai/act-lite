//! The dependency model core: outcomes, probes, edges, relevance and the
//! common `details`, with a test resolver for the JS/TS family that turns
//! each specifier into a probe or an outcome:
//!
//! - `ext:<x>` is external; `unres:<x>` is unresolved with reason `<x>`;
//! - `up:<rel>` normalizes `<rel>` from the importing file's directory and is
//!   external when it leaves the workspace;
//! - `input:<file>` reads a `package.json` input; `bad:<file>` reads a
//!   `tsconfig` input (a `go.mod` input when `<file>` is `go.mod`) and
//!   records that it does not parse; both resolve to nothing
//!   (`Resolved([])`);
//! - `failed:` is unresolved with the first failed JS/TS input as reason;
//! - `count:` reports the per-build counter after incrementing it;
//! - anything else is probed as a workspace-relative path.

use act_lite_cli::analyze::deps::{
    DepGraph, Family, InputKind, Outcome, Probe, ResolveCx, Resolver, supports,
};
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::read::{Reader, SupportReader};
use act_lite_cli::analyze::run::{Request, RunContext};
use act_lite_cli::analyze::envelope::AnalyzeError;
use act_lite_parser::Grammar;
use act_lite_parser::deps::Relationship;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::sync::CancellationToken;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

struct TestResolver {
    calls: AtomicUsize,
    cancel_on_first: Option<CancellationToken>,
}

impl TestResolver {
    fn new() -> TestResolver {
        TestResolver {
            calls: AtomicUsize::new(0),
            cancel_on_first: None,
        }
    }
}

impl Resolver for TestResolver {
    fn family(&self) -> Family {
        Family::JsTs
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0
            && let Some(cancel) = &self.cancel_on_first
        {
            cancel.cancel();
        }
        let spec = rel.specifier.as_str();
        if spec.starts_with("ext:") {
            return Outcome::External;
        }
        if let Some(reason) = spec.strip_prefix("unres:") {
            return Outcome::Unresolved(reason.to_string());
        }
        if let Some(rest) = spec.strip_prefix("up:") {
            let dir = file.rsplit_once('/').map_or("", |(d, _)| d);
            return match cx.normalize(dir, rest) {
                None => Outcome::External,
                Some(path) => Outcome::Resolved(vec![path]),
            };
        }
        if let Some(input) = spec.strip_prefix("input:") {
            return match cx.input(input, InputKind::PackageJson) {
                Ok(_) => Outcome::Resolved(Vec::new()),
                Err(err) => Outcome::Unresolved(err),
            };
        }
        if let Some(input) = spec.strip_prefix("bad:") {
            let kind = if input == "go.mod" {
                InputKind::GoMod
            } else {
                InputKind::Tsconfig
            };
            let _ = cx.input(input, kind);
            cx.input_error(input, "does not parse".to_string());
            return Outcome::Resolved(Vec::new());
        }
        if spec == "failed:" {
            let first = cx.input_failed(Family::JsTs).unwrap_or("none");
            return Outcome::Unresolved(first.to_string());
        }
        if spec == "count:" {
            let counter = cx.cache::<u32>();
            *counter += 1;
            return Outcome::Unresolved(format!("count {counter}"));
        }
        match cx.probe_file(spec) {
            Probe::File(path) => Outcome::Resolved(vec![path]),
            Probe::Outside => Outcome::External,
            other => Outcome::Unresolved(format!("{other:?}")),
        }
    }
}

fn is_js_or_ts(grammar: Grammar) -> bool {
    matches!(
        grammar,
        Grammar::JavaScript | Grammar::Jsx | Grammar::TypeScript | Grammar::Tsx
    )
}

/// Builds the graph of `root` with `resolver`, `paths` as the scope.
fn build(
    root: &Path,
    paths: &[&str],
    resolver: &TestResolver,
    cancel: &CancellationToken,
) -> Result<DepGraph, AnalyzeError> {
    let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
    let request = Request::new(root, &paths, &[], &[], Includes::default(), 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &is_js_or_ts, cancel)?,
        support: SupportReader::new(root, &Includes::default(), cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &is_js_or_ts, false, cancel);
    DepGraph::build_with(&mut reader, &ctx, &[resolver], cancel)
}

fn graph(root: &Path) -> DepGraph {
    build(root, &[], &TestResolver::new(), &CancellationToken::new()).unwrap()
}

fn edges_json(graph: &DepGraph) -> Value {
    serde_json::to_value(graph.edges()).unwrap()
}

#[test]
fn a_probe_compares_names_byte_for_byte() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/b.ts", "export const b = 1;\n");
    write(ws.path(), "src/a.ts", "import 'src/B.ts';\n");
    let graph = graph(ws.path());
    assert!(graph.edges().is_empty());
    let unresolved = graph.unresolved(&|_| true);
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].reason, "None");
}

#[test]
fn a_relationship_to_its_own_file_is_no_edge() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/s.ts", "import 'src/s.ts';\n");
    let graph = graph(ws.path());
    assert!(graph.edges().is_empty());
    assert_eq!(
        graph.common_details()["relationships"],
        json!({"total": 1, "resolved": 1, "external": 0, "unresolved": 0})
    );
}

#[test]
fn relationships_to_one_file_are_one_edge() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "src/x.ts",
        "import 'src/y.ts';\nimport { a } from 'src/y.ts';\nexport * from 'src/y.ts';\n",
    );
    write(ws.path(), "src/y.ts", "export const a = 1;\n");
    let graph = graph(ws.path());
    assert_eq!(
        edges_json(&graph),
        json!([{
            "file": "src/x.ts", "to": "src/y.ts", "to_analyzed": true,
            "kinds": ["import", "reexport"],
            "evidence": [
                {"range": "1:1-1:19", "kind": "import", "specifier": "src/y.ts"},
                {"range": "2:1-2:30", "kind": "import", "specifier": "src/y.ts"},
                {"range": "3:1-3:26", "kind": "reexport", "specifier": "src/y.ts"}
            ],
            "evidence_total": 3
        }])
    );
    let step = graph.edges()[0].step();
    assert_eq!(step.file, "src/x.ts");
    assert_eq!(step.to, "src/y.ts");
    assert_eq!(step.range.to_string(), "1:1-1:19");
}

#[test]
fn evidence_is_capped_at_five() {
    let ws = tempfile::tempdir().unwrap();
    let source: String = (0..7).map(|_| "import 'src/y.ts';\n").collect();
    write(ws.path(), "src/x.ts", &source);
    write(ws.path(), "src/y.ts", "export const a = 1;\n");
    let graph = graph(ws.path());
    let edge = &edges_json(&graph)[0];
    assert_eq!(edge["evidence_total"], 7);
    let lines: Vec<String> = edge["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["range"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(lines, ["1:1-1:19", "2:1-2:19", "3:1-3:19", "4:1-4:19", "5:1-5:19"]);
}

#[test]
fn targets_outside_the_analyzed_set_and_the_workspace() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "src/x.ts",
        "import 'src/y.test.ts';\nimport 'other/z.ts';\nimport 'up:../../x';\n",
    );
    write(ws.path(), "src/y.test.ts", "export const t = 1;\n");
    write(ws.path(), "other/z.ts", "export const z = 1;\n");
    let graph = build(ws.path(), &["src"], &TestResolver::new(), &CancellationToken::new()).unwrap();
    let edges = edges_json(&graph);
    let to_analyzed: Vec<(String, bool)> = edges
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["to"].as_str().unwrap().to_string(),
                e["to_analyzed"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        to_analyzed,
        [("other/z.ts".to_string(), false), ("src/y.test.ts".to_string(), false)]
    );
    assert_eq!(graph.common_details()["relationships"]["external"], 1);
}

#[cfg(unix)]
#[test]
fn symlink_probes_resolve_into_the_workspace_or_are_outside() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    write(&root, "src/real.ts", "export const r = 1;\n");
    write(tmp.path(), "out.ts", "export const o = 1;\n");
    std::os::unix::fs::symlink(root.join("src/real.ts"), root.join("src/link.ts")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("out.ts"), root.join("src/out.ts")).unwrap();
    write(&root, "src/x.ts", "import 'src/link.ts';\nimport 'src/out.ts';\n");
    let graph = graph(&root);
    let targets: Vec<&str> = graph.edges().iter().map(|e| e.to.as_str()).collect();
    assert_eq!(targets, ["src/real.ts"]);
    assert_eq!(graph.common_details()["relationships"]["external"], 1);
}

#[test]
fn common_details_add_up() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "package.json", "{\"name\":\"p\"}\n");
    write(ws.path(), "go.mod", "not a go.mod\n");
    write(ws.path(), "src/y.ts", "export const a = 1;\n");
    write(
        ws.path(),
        "src/x.ts",
        "import 'src/y.ts';\nexport * from 'src/y.ts';\nimport 'ext:react';\nimport 'unres:no file';\nimport 'input:package.json';\nimport 'bad:go.mod';\n",
    );
    let graph = graph(ws.path());
    let details = graph.common_details();
    assert_eq!(
        details["relationships"],
        json!({"total": 6, "resolved": 4, "external": 1, "unresolved": 1})
    );
    assert_eq!(details["kinds"], json!(["import", "reexport"]));
    assert_eq!(
        details["resolution_inputs"],
        json!([
            {"file": "go.mod", "kind": "go.mod", "error": "does not parse"},
            {"file": "package.json", "kind": "package.json"}
        ])
    );
    assert_eq!(details["resolution_inputs_total"], 2);
}

#[test]
fn an_unreadable_input_makes_externals_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "tsconfig.json", "{ broken\n");
    write(
        ws.path(),
        "src/x.ts",
        "import 'bad:tsconfig.json';\nimport 'ext:react';\n",
    );
    let graph = graph(ws.path());
    let unresolved = graph.unresolved(&|_| true);
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].target, "ext:react");
    assert_eq!(unresolved[0].reason, "tsconfig.json could not be read");
}

#[test]
fn resolution_inputs_are_capped_at_one_hundred() {
    let ws = tempfile::tempdir().unwrap();
    let mut source = String::new();
    for i in 0..101 {
        write(ws.path(), &format!("cfg/{i:03}.json"), "{}\n");
        source.push_str(&format!("import 'input:cfg/{i:03}.json';\n"));
    }
    write(ws.path(), "src/x.ts", &source);
    let details = graph(ws.path()).common_details();
    assert_eq!(details["resolution_inputs"].as_array().unwrap().len(), 100);
    assert_eq!(details["resolution_inputs_total"], 101);
    assert_eq!(details["resolution_inputs"][0]["file"], "cfg/000.json");
}

#[test]
fn the_unresolved_filter_keeps_the_matching_entries_in_order() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/b.ts", "import 'unres:b2';\nimport 'unres:b1';\n");
    write(ws.path(), "src/a.ts", "import 'unres:a';\n");
    let graph = graph(ws.path());
    let all: Vec<(String, String)> = graph
        .unresolved(&|_| true)
        .into_iter()
        .map(|u| (u.file, u.target))
        .collect();
    assert_eq!(
        all,
        [
            ("src/a.ts".to_string(), "unres:a".to_string()),
            ("src/b.ts".to_string(), "unres:b2".to_string()),
            ("src/b.ts".to_string(), "unres:b1".to_string()),
        ]
    );
    let only_b = graph.unresolved(&|u| u.file == "src/b.ts" && u.family == Family::JsTs);
    assert_eq!(only_b.len(), 2);
    assert_eq!(only_b[0].kind, "import");
}

#[test]
fn supports_exactly_the_seven_entries() {
    let supported: Vec<&str> = Grammar::all()
        .iter()
        .filter(|g| supports(**g))
        .map(|g| g.name())
        .collect();
    let mut supported = supported;
    supported.sort();
    assert_eq!(
        supported,
        ["Go", "JSX", "JavaScript", "Python", "Rust", "TSX", "TypeScript"]
    );
}

#[test]
fn input_errors_mark_the_family_and_the_cache_lasts_the_build() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "tsconfig.json", "{ broken\n");
    write(
        ws.path(),
        "src/x.ts",
        "import 'count:';\nimport 'count:';\nimport 'bad:tsconfig.json';\nimport 'failed:';\n",
    );
    let graph = graph(ws.path());
    let reasons: Vec<String> = graph
        .unresolved(&|_| true)
        .into_iter()
        .map(|u| u.reason)
        .collect();
    assert_eq!(reasons, ["count 1", "count 2", "tsconfig.json"]);
    assert_eq!(
        graph.common_details()["resolution_inputs"][0]["error"],
        "does not parse"
    );
}

#[test]
fn a_cancelled_token_stops_the_build_before_the_next_file() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/a.ts", "import 'unres:a';\n");
    write(ws.path(), "src/b.ts", "import 'unres:b';\n");
    let cancel = CancellationToken::new();
    let resolver = TestResolver {
        calls: AtomicUsize::new(0),
        cancel_on_first: Some(cancel.clone()),
    };
    let result = build(ws.path(), &[], &resolver, &cancel);
    assert!(matches!(result, Err(AnalyzeError::Cancelled)));
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn the_families_and_the_stubs() {
    assert_eq!(Family::of(Grammar::Tsx), Some(Family::JsTs));
    assert_eq!(Family::of(Grammar::Python), Some(Family::Python));
    assert_eq!(Family::of(Grammar::Css), None);
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/a.ts", "import 'x';\n");
    write(ws.path(), "lib.py", "import os\n");
    let request = Request::new(ws.path(), &[], &[], &[], Includes::default(), 100).unwrap();
    let cancel = CancellationToken::new();
    let ctx = RunContext {
        root: ws.path().to_path_buf(),
        discovered: discover(ws.path(), &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(ws.path(), &Includes::default(), &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    let graph = DepGraph::build(&mut reader, &ctx, &cancel).unwrap();
    let reasons: Vec<String> = graph
        .unresolved(&|_| true)
        .into_iter()
        .map(|u| u.reason)
        .collect();
    assert_eq!(reasons, ["resolution not implemented", "resolution not implemented"]);
    assert_eq!(graph.family_of("lib.py"), Some(Family::Python));
    assert_eq!(graph.family_of("README.md"), None);
    assert!(graph.analyzed().contains("src/a.ts"));
    assert_eq!(graph.relationships_of("src/a.ts").len(), 1);
}
