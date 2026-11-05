//! JavaScript, JSX, TypeScript and TSX resolution: relative probes,
//! `tsconfig`/`jsconfig` `paths` and `baseUrl`, workspace packages,
//! dependency tables, built-ins and `node_modules`.

use act_lite_cli::analyze::deps::{DepGraph, Outcome, supports};
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::read::{Reader, SupportReader};
use act_lite_cli::analyze::run::{Request, RunContext};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A workspace with an empty `.git/` and `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir(ws.path().join(".git")).unwrap();
    for (rel, content) in files {
        write(ws.path(), rel, content);
    }
    ws
}

fn build(root: &Path) -> DepGraph {
    let cancel = CancellationToken::new();
    let request = Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    DepGraph::build(&mut reader, &ctx, &cancel).unwrap()
}

/// Each edge as (file, to, to_analyzed, kinds, [(line, specifier)]).
type EdgeRow = (String, String, bool, Vec<String>, Vec<(u64, String)>);

fn edge_rows(graph: &DepGraph) -> Vec<EdgeRow> {
    let edges = serde_json::to_value(graph.edges()).unwrap();
    edges
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let evidence = e["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    let range = v["range"].as_str().unwrap();
                    let line = range.split(':').next().unwrap().parse().unwrap();
                    (line, v["specifier"].as_str().unwrap().to_string())
                })
                .collect();
            (
                e["file"].as_str().unwrap().to_string(),
                e["to"].as_str().unwrap().to_string(),
                e["to_analyzed"].as_bool().unwrap(),
                e["kinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| k.as_str().unwrap().to_string())
                    .collect(),
                evidence,
            )
        })
        .collect()
}

fn row(file: &str, to: &str, analyzed: bool, kind: &str, line: u64, spec: &str) -> EdgeRow {
    (
        file.to_string(),
        to.to_string(),
        analyzed,
        vec![kind.to_string()],
        vec![(line, spec.to_string())],
    )
}

/// The outcome of the only relationship of `file`.
fn outcome(graph: &DepGraph, file: &str) -> Outcome {
    let rels = graph.relationships_of(file);
    assert_eq!(rels.len(), 1, "{file}");
    rels[0].outcome.clone()
}

fn resolved(target: &str) -> Outcome {
    Outcome::Resolved(vec![target.to_string()])
}

fn unresolved(reason: &str) -> Outcome {
    Outcome::Unresolved(reason.to_string())
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

#[test]
fn the_ts_fixture() {
    let ws = workspace(&ts_fixture());
    let graph = build(ws.path());
    assert_eq!(
        edge_rows(&graph),
        [
            row("src/a.ts", "src/main.ts", true, "import", 1, "./main"),
            row("src/main.ts", "src/a.ts", true, "import", 1, "./a"),
            row("src/main.ts", "src/c.ts", true, "require", 5, "./c.js"),
            row("src/main.ts", "src/d/index.ts", true, "reexport", 6, "./d"),
            row("src/main.ts", "src/lib/b.ts", true, "import", 2, "@lib/b"),
            row(
                "src/main.ts",
                "src/types.d.ts",
                true,
                "type_import",
                7,
                "./types"
            ),
            row("src/view.tsx", "src/a.ts", true, "import", 2, "./a.js"),
            row(
                "src/view.tsx",
                "src/style.css",
                false,
                "import",
                1,
                "./style.css"
            ),
        ]
    );
    let edges = serde_json::to_value(graph.edges()).unwrap();
    assert_eq!(
        edges[2]["evidence"][0],
        json!({"range": "5:11-5:28", "kind": "require", "specifier": "./c.js"})
    );
    assert_eq!(
        edges[1]["evidence"][0],
        json!({"range": "1:1-1:25", "kind": "import", "specifier": "./a"})
    );
    let main = graph.relationships_of("src/main.ts");
    assert_eq!(main[2].outcome, Outcome::External);
    assert_eq!(main[3].outcome, Outcome::External);
    let unresolved: Vec<(String, String, String, String, u32)> = graph
        .unresolved(&|_| true)
        .into_iter()
        .map(|u| (u.file, u.kind, u.target, u.reason, u.range.start_line()))
        .collect();
    let entry = |kind: &str, target: &str, reason: &str, line: u32| {
        (
            "src/main.ts".to_string(),
            kind.to_string(),
            target.to_string(),
            reason.to_string(),
            line,
        )
    };
    assert_eq!(
        unresolved,
        [
            entry(
                "dynamic_import",
                "./missing",
                "no file matches ./missing",
                8
            ),
            entry("require", "name", "specifier is not a string literal", 9),
        ]
    );
    let details = Value::Object(graph.common_details());
    assert_eq!(
        details,
        json!({
            "relationships": {"total": 12, "resolved": 8, "external": 2, "unresolved": 2},
            "kinds": ["dynamic_import", "import", "reexport", "require", "type_import"],
            "resolution_inputs": [
                {"file": "package.json", "kind": "package.json"},
                {"file": "tsconfig.json", "kind": "tsconfig"}
            ],
            "resolution_inputs_total": 2
        })
    );
}

#[test]
fn the_acyclic_and_unres_fixtures() {
    let ws = workspace(&[
        (
            "src/a.ts",
            "import { b } from './b';\nimport fs from 'node:fs';\n",
        ),
        ("src/b.ts", "import { c } from './c';\n"),
        ("src/c.ts", "export const c = 1;\n"),
        ("src/d.ts", "import './b';\nimport './c';\n"),
    ]);
    let graph = build(ws.path());
    let pairs: Vec<(String, String)> = graph
        .edges()
        .iter()
        .map(|e| (e.file.clone(), e.to.clone()))
        .collect();
    let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
    assert_eq!(
        pairs,
        [
            pair("src/a.ts", "src/b.ts"),
            pair("src/b.ts", "src/c.ts"),
            pair("src/d.ts", "src/b.ts"),
            pair("src/d.ts", "src/c.ts"),
        ]
    );
    assert!(graph.unresolved(&|_| true).is_empty());
    assert_eq!(graph.common_details()["relationships"]["external"], 1);

    let ws = workspace(&[
        ("src/t.ts", "export const t = 1;\n"),
        ("src/u.ts", "import { t } from './t-old';\n"),
    ]);
    let graph = build(ws.path());
    assert!(graph.edges().is_empty());
    let entries = serde_json::to_value(graph.unresolved(&|_| true)).unwrap();
    assert_eq!(
        entries,
        json!([{"file": "src/u.ts", "range": "1:1-1:29", "kind": "import",
                "target": "./t-old", "reason": "no file matches ./t-old"}])
    );
}

#[test]
fn bare_specifiers_need_a_declaration() {
    let mut files = ts_fixture();
    files.retain(|(rel, _)| *rel != "src/c.ts");
    files.push(("src/c.ts", "import lodash from 'lodash';\n"));
    let ws = workspace(&files);
    assert_eq!(
        outcome(&build(ws.path()), "src/c.ts"),
        unresolved("bare specifier is not a declared dependency: lodash")
    );
    write(
        ws.path(),
        "package.json",
        r#"{"name":"ts-fixture","private":true,"dependencies":{"react":"18.3.1","lodash":"4.17.21"}}"#,
    );
    assert_eq!(outcome(&build(ws.path()), "src/c.ts"), Outcome::External);
    write(ws.path(), "src/c.ts", "import fs from 'fs';\n");
    assert_eq!(outcome(&build(ws.path()), "src/c.ts"), Outcome::External);
}

#[test]
fn language_specific_forms() {
    let ws = workspace(&[
        ("src/y.js", ""),
        ("src/x.js", "const y = require('./y');\n"),
        ("src/x.jsx", "export * from './y';\n"),
        ("src/t.ts", "import y = require('./y');\n"),
        ("src/u.ts", "const m = import(`./y`);\n"),
        ("src/v.ts", "import '../../outside';\n"),
        ("src/w.ts", "import '#internal';\n"),
        ("src/r.ts", "import '/src/y.js';\n"),
        ("packages/p/package.json", r#"{"name":"p","main":"lib.js"}"#),
        ("packages/p/lib.js", ""),
        ("src/q.ts", "import 'p';\n"),
        (
            "packages/e/package.json",
            r#"{"name":"e","exports":"./i.js"}"#,
        ),
        ("src/q2.ts", "import 'e';\n"),
        (
            "cfg/tsconfig.json",
            r#"{"compilerOptions":{"paths":{"@t/*":["t/*"]}}}"#,
        ),
        (
            "cfg/jsconfig.json",
            r#"{"compilerOptions":{"paths":{"@t/*":["j/*"]}}}"#,
        ),
        ("cfg/t/a.ts", ""),
        ("cfg/j/a.ts", ""),
        ("cfg/m.ts", "import '@t/a';\n"),
        ("ext/tsconfig.json", r#"{"extends":"./base.json"}"#),
        (
            "ext/base.json",
            r#"{"compilerOptions":{"paths":{"@b/*":["b/*"]}}}"#,
        ),
        ("ext/b/k.ts", ""),
        ("ext/m.ts", "import '@b/k';\n"),
    ]);
    let graph = build(ws.path());
    let kind = |file: &str| graph.relationships_of(file)[0].relationship.kind.as_str();
    assert_eq!(kind("src/x.js"), "require");
    assert_eq!(outcome(&graph, "src/x.js"), resolved("src/y.js"));
    assert_eq!(kind("src/x.jsx"), "reexport");
    assert_eq!(outcome(&graph, "src/x.jsx"), resolved("src/y.js"));
    assert_eq!(kind("src/t.ts"), "require");
    assert_eq!(outcome(&graph, "src/t.ts"), resolved("src/y.js"));
    assert_eq!(kind("src/u.ts"), "dynamic_import");
    assert_eq!(outcome(&graph, "src/u.ts"), resolved("src/y.js"));
    assert_eq!(outcome(&graph, "src/v.ts"), Outcome::External);
    assert_eq!(
        outcome(&graph, "src/w.ts"),
        unresolved("package.json \"imports\" is not applied")
    );
    assert_eq!(
        outcome(&graph, "src/r.ts"),
        unresolved("root-relative specifier is not applied")
    );
    assert_eq!(outcome(&graph, "src/q.ts"), resolved("packages/p/lib.js"));
    assert_eq!(
        outcome(&graph, "src/q2.ts"),
        unresolved("package.json \"exports\" of e is not applied")
    );
    assert_eq!(outcome(&graph, "cfg/m.ts"), resolved("cfg/t/a.ts"));
    assert_eq!(outcome(&graph, "ext/m.ts"), resolved("ext/b/k.ts"));
    let inputs = &graph.common_details()["resolution_inputs"];
    assert!(
        inputs
            .as_array()
            .unwrap()
            .contains(&json!({"file": "ext/base.json", "kind": "tsconfig"})),
        "{inputs}"
    );
}

#[test]
fn an_emitted_extension_resolves_to_its_source_first() {
    let ws = workspace(&[
        ("src/a.js", ""),
        ("src/a.ts", ""),
        ("src/types.d.ts", ""),
        ("src/m.ts", "import './a.js';\n"),
        ("src/n.ts", "import './types.js';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/m.ts"), resolved("src/a.ts"));
    assert_eq!(outcome(&graph, "src/n.ts"), resolved("src/types.d.ts"));
}

#[test]
fn a_paths_key_without_a_file_falls_through() {
    let ws = workspace(&[
        (
            "tsconfig.json",
            r#"{
  // comments and trailing commas are accepted
  "compilerOptions": {
    "baseUrl": "src",
    "paths": { "@x/*": ["nowhere/*",], },
  },
}"#,
        ),
        ("package.json", r#"{"dependencies":{"@x/dep":"1.0.0"}}"#),
        ("src/@x/k.ts", ""),
        ("src/a.ts", "import '@x/k';\n"),
        ("src/b.ts", "import '@x/dep';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/a.ts"), resolved("src/@x/k.ts"));
    assert_eq!(outcome(&graph, "src/b.ts"), Outcome::External);
}

#[test]
fn workspace_package_names() {
    let ws = workspace(&[
        ("a/package.json", r#"{"name":"p"}"#),
        ("a/index.js", ""),
        ("b/package.json", r#"{"name":"p"}"#),
        ("b/index.js", ""),
        ("tests/fixtures/c/package.json", r#"{"name":"q"}"#),
        ("tests/fixtures/c/index.js", ""),
        ("src/m.ts", "import 'p';\n"),
        ("src/n.ts", "import 'q';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        outcome(&graph, "src/m.ts"),
        unresolved("workspace package name p is declared by more than one package.json")
    );
    assert_eq!(
        outcome(&graph, "src/n.ts"),
        unresolved("bare specifier is not a declared dependency: q")
    );
}

#[test]
fn dependencies_that_name_workspace_locations() {
    let ws = workspace(&[
        (
            "app/package.json",
            r#"{"name":"app","dependencies":{"ui":"workspace:@acme/ui@*","lib":"file:../lib"},
               "devDependencies":{"l2":"link:../lib2","p3":"portal:../lib3","out":"file:../../out"}}"#,
        ),
        (
            "packages/ui/package.json",
            r#"{"name":"@acme/ui","main":"index.js"}"#,
        ),
        ("packages/ui/index.js", ""),
        ("lib/index.ts", ""),
        ("lib2/index.ts", ""),
        ("lib3/index.ts", ""),
        ("app/node_modules/vend/index.js", ""),
        ("app/src/ui.ts", "import 'ui';\n"),
        ("app/src/lib.ts", "import 'lib';\n"),
        ("app/src/l2.ts", "import 'l2';\n"),
        ("app/src/p3.ts", "import 'p3';\n"),
        ("app/src/out.ts", "import 'out';\n"),
        ("app/src/vend.ts", "import 'vend';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        outcome(&graph, "app/src/ui.ts"),
        resolved("packages/ui/index.js")
    );
    assert_eq!(outcome(&graph, "app/src/lib.ts"), resolved("lib/index.ts"));
    assert_eq!(outcome(&graph, "app/src/l2.ts"), resolved("lib2/index.ts"));
    assert_eq!(outcome(&graph, "app/src/p3.ts"), resolved("lib3/index.ts"));
    assert_eq!(outcome(&graph, "app/src/out.ts"), Outcome::External);
    assert_eq!(outcome(&graph, "app/src/vend.ts"), Outcome::External);
}

#[test]
fn a_tsconfig_that_does_not_parse() {
    let ws = workspace(&[
        ("tsconfig.json", "{ broken\n"),
        ("package.json", r#"{"dependencies":{"react":"18.3.1"}}"#),
        ("src/b.ts", ""),
        (
            "src/a.ts",
            "import React from 'react';\nimport { x } from '@lib/x';\nimport './b';\n",
        ),
        ("other/c.js", "const fs = require('node:fs');\n"),
    ]);
    let graph = build(ws.path());
    let details = graph.common_details();
    let tsconfig = details["resolution_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["file"] == "tsconfig.json")
        .unwrap()
        .clone();
    assert_eq!(tsconfig["kind"], "tsconfig");
    assert!(tsconfig["error"].is_string(), "{tsconfig}");
    let a = graph.relationships_of("src/a.ts");
    let unread = unresolved("tsconfig.json could not be read");
    assert_eq!(a[0].outcome, unread);
    assert_eq!(a[1].outcome, unread);
    assert_eq!(a[2].outcome, resolved("src/b.ts"));
    assert_eq!(outcome(&graph, "other/c.js"), unread);
}

fn inputs(graph: &DepGraph) -> Vec<Value> {
    graph.common_details()["resolution_inputs"]
        .as_array()
        .unwrap()
        .clone()
}

fn input_error(graph: &DepGraph, file: &str) -> Option<String> {
    inputs(graph)
        .iter()
        .find(|i| i["file"] == file)
        .and_then(|i| i["error"].as_str().map(str::to_string))
}

#[test]
fn a_workspace_range_names_the_key() {
    let ws = workspace(&[
        (
            "package.json",
            r#"{"dependencies":{"w":"workspace:*","v":"workspace:^1.0.0","g":"github:o/g"}}"#,
        ),
        ("src/w.ts", "import 'w';\n"),
        ("src/v.ts", "import 'v';\n"),
        ("src/g.ts", "import 'g';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        outcome(&graph, "src/w.ts"),
        unresolved("no workspace package is named w")
    );
    assert_eq!(
        outcome(&graph, "src/v.ts"),
        unresolved("no workspace package is named v")
    );
    assert_eq!(outcome(&graph, "src/g.ts"), Outcome::External);
}

#[test]
fn a_paths_replacement_that_leaves_the_workspace_is_external() {
    let ws = workspace(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"paths":{"@o/*":["../../out/*"],"@m/*":["../../out/*","src/*"]}}}"#,
        ),
        ("src/k.ts", ""),
        ("src/a.ts", "import '@o/k';\n"),
        ("src/b.ts", "import '@m/k';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/a.ts"), Outcome::External);
    assert_eq!(outcome(&graph, "src/b.ts"), resolved("src/k.ts"));
}

#[test]
fn a_base_url_that_leaves_the_workspace_is_external() {
    let ws = workspace(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":"..","paths":{"@a/*":["src/*"]}}}"#,
        ),
        ("src/k.ts", ""),
        ("src/a.ts", "import '@a/k';\n"),
        ("src/b.ts", "import 'zz';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/a.ts"), Outcome::External);
    assert_eq!(outcome(&graph, "src/b.ts"), Outcome::External);
}

#[test]
fn a_missing_extends_target_breaks_the_config() {
    let ws = workspace(&[
        (
            "tsconfig.json",
            r#"{"extends":"./nope","compilerOptions":{"paths":{"@p/*":["src/*"]}}}"#,
        ),
        ("o/tsconfig.json", r#"{"extends":"../../up.json"}"#),
        ("src/k.ts", ""),
        (
            "src/a.ts",
            "import '@p/k';\nimport 'zz';\nimport 'node:fs';\n",
        ),
        ("o/b.ts", "import 'yy';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        input_error(&graph, "tsconfig.json").as_deref(),
        Some("extends target nope does not exist")
    );
    assert_eq!(
        input_error(&graph, "o/tsconfig.json").as_deref(),
        Some("extends target ../../up.json leaves the workspace")
    );
    let a = graph.relationships_of("src/a.ts");
    assert_eq!(a[0].outcome, resolved("src/k.ts"));
    assert_eq!(a[1].outcome, unresolved("tsconfig.json could not be read"));
    assert_eq!(
        a[2].outcome,
        unresolved("o/tsconfig.json could not be read")
    );
    assert_eq!(
        outcome(&graph, "o/b.ts"),
        unresolved("o/tsconfig.json could not be read")
    );
}

#[test]
fn an_extends_member_that_does_not_parse() {
    let ws = workspace(&[
        (
            "good.json",
            r#"{"compilerOptions":{"paths":{"@g/*":["g/*"]}}}"#,
        ),
        ("bad.json", "{ broken\n"),
        ("g/k.ts", ""),
        (
            "x/tsconfig.json",
            r#"{"extends":["../good.json","../bad.json"]}"#,
        ),
        ("x/m.ts", "import '@g/k';\n"),
        (
            "y/tsconfig.json",
            r#"{"extends":["../bad.json","../good.json"]}"#,
        ),
        ("y/m.ts", "import '@g/k';\nimport 'zz';\n"),
    ]);
    let graph = build(ws.path());
    assert!(input_error(&graph, "bad.json").is_some());
    assert_eq!(
        outcome(&graph, "x/m.ts"),
        unresolved("bad.json could not be read")
    );
    let y = graph.relationships_of("y/m.ts");
    assert_eq!(y[0].outcome, resolved("g/k.ts"));
    assert_eq!(y[1].outcome, unresolved("bad.json could not be read"));
}

#[test]
fn an_extends_cycle() {
    let ws = workspace(&[
        (
            "tsconfig.json",
            r#"{"extends":"./base.json","compilerOptions":{"paths":{"@p/*":["src/*"]}}}"#,
        ),
        ("base.json", r#"{"extends":"./tsconfig.json"}"#),
        ("src/k.ts", ""),
        ("src/a.ts", "import '@p/k';\nimport 'zz';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        input_error(&graph, "base.json").as_deref(),
        Some("extends cycle at tsconfig.json")
    );
    let a = graph.relationships_of("src/a.ts");
    assert_eq!(a[0].outcome, resolved("src/k.ts"));
    assert_eq!(a[1].outcome, unresolved("base.json could not be read"));
}

#[test]
fn an_empty_package_name_is_never_in_node_modules() {
    let ws = workspace(&[
        ("node_modules/@s/x/index.js", ""),
        ("src/a.ts", "import '';\n"),
        ("src/b.ts", "import '@s/..';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        outcome(&graph, "src/a.ts"),
        unresolved("bare specifier is not a declared dependency: ")
    );
    assert_eq!(
        outcome(&graph, "src/b.ts"),
        unresolved("bare specifier is not a declared dependency: @s/..")
    );
}

#[test]
fn a_package_json_in_a_vendored_directory_is_not_read() {
    let ws = workspace(&[
        ("node_modules/pkg/package.json", r#"{"main":"lib.js"}"#),
        ("node_modules/pkg/lib.js", ""),
        ("node_modules/pkg/index.js", ""),
        ("src/m.ts", "import '../node_modules/pkg';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(
        outcome(&graph, "src/m.ts"),
        resolved("node_modules/pkg/index.js")
    );
    assert!(inputs(&graph).is_empty(), "{:?}", inputs(&graph));
}

#[test]
fn a_main_that_names_a_directory() {
    let ws = workspace(&[
        ("packages/p/package.json", r#"{"name":"p","main":"."}"#),
        ("packages/p/index.ts", ""),
        ("packages/p.ts", ""),
        ("packages/q/package.json", r#"{"name":"q","main":"sub/"}"#),
        ("packages/q/sub.ts", ""),
        ("packages/q/sub/index.ts", ""),
        ("src/p.ts", "import 'p';\n"),
        ("src/q.ts", "import 'q';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/p.ts"), resolved("packages/p/index.ts"));
    assert_eq!(
        outcome(&graph, "src/q.ts"),
        resolved("packages/q/sub/index.ts")
    );
}

#[test]
fn a_trailing_slash_names_a_directory() {
    let ws = workspace(&[
        ("src/u.ts", ""),
        ("src/u/index.ts", ""),
        ("src/m.ts", "import './u/';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/m.ts"), resolved("src/u/index.ts"));
}

#[cfg(unix)]
#[test]
fn a_node_modules_symlink_out_of_the_workspace_is_external() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("node_modules")).unwrap();
    fs::create_dir_all(tmp.path().join("store/ext")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("store/ext"), root.join("node_modules/ext"))
        .unwrap();
    write(&root, "src/m.ts", "import 'ext';\n");
    assert_eq!(outcome(&build(&root), "src/m.ts"), Outcome::External);
}

#[test]
fn a_main_that_leaves_the_workspace_is_external() {
    let ws = workspace(&[
        (
            "packages/p/package.json",
            r#"{"name":"p","main":"../../../x.js"}"#,
        ),
        ("packages/p/index.ts", ""),
        ("src/p.ts", "import 'p';\n"),
    ]);
    assert_eq!(outcome(&build(ws.path()), "src/p.ts"), Outcome::External);
}

#[test]
fn an_unreadable_package_json() {
    let ws = workspace(&[
        ("app/package.json", "{ broken\n"),
        ("app/src/m.ts", "import 'react';\n"),
    ]);
    assert_eq!(
        outcome(&build(ws.path()), "app/src/m.ts"),
        unresolved("app/package.json could not be read")
    );
    let ws = workspace(&[
        ("packages/x/package.json", "{ broken\n"),
        ("packages/x/index.ts", ""),
        ("src/n.ts", "import 'x';\n"),
    ]);
    assert_eq!(
        outcome(&build(ws.path()), "src/n.ts"),
        unresolved("bare specifier is not a declared dependency: x")
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_makes_dependency_externals_unresolved() {
    use std::os::unix::fs::PermissionsExt;
    let ws = workspace(&[
        ("package.json", r#"{"dependencies":{"react":"18.3.1"}}"#),
        ("locked/package.json", r#"{"name":"react"}"#),
        ("src/m.ts", "import 'react';\nimport 'node:fs';\n"),
    ]);
    let locked = ws.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let graph = build(ws.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let m = graph.relationships_of("src/m.ts");
    assert_eq!(m[0].outcome, unresolved("locked/ could not be read"));
    assert_eq!(m[1].outcome, Outcome::External);
}

#[test]
fn a_package_json_in_a_virtual_environment_is_not_read() {
    let ws = workspace(&[
        ("env/pyvenv.cfg", "home = /usr/bin\n"),
        (
            "env/pkg/package.json",
            r#"{"name":"venvpkg","main":"lib.js"}"#,
        ),
        ("env/pkg/lib.js", ""),
        ("env/pkg/index.js", ""),
        ("src/m.ts", "import '../env/pkg';\n"),
        ("src/n.ts", "import 'venvpkg';\n"),
    ]);
    let graph = build(ws.path());
    assert_eq!(outcome(&graph, "src/m.ts"), resolved("env/pkg/index.js"));
    assert_eq!(
        outcome(&graph, "src/n.ts"),
        unresolved("bare specifier is not a declared dependency: venvpkg")
    );
    assert!(inputs(&graph).is_empty(), "{:?}", inputs(&graph));
}
