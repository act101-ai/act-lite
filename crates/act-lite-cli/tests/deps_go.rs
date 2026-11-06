//! Go import resolution: the module map, candidates and their validity,
//! the three outcomes and the resolution inputs.

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

fn build(root: &Path, includes: Includes) -> DepGraph {
    let cancel = CancellationToken::new();
    let request = Request::new(root, &[], &[], &[], includes, 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &includes, &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    DepGraph::build(&mut reader, &ctx, &cancel).unwrap()
}

fn graph(root: &Path) -> DepGraph {
    build(root, Includes::default())
}

/// The shop fixture: a root module, a nested module that provides a
/// sub-package of it, and an unrelated module.
fn shop(root: &Path) {
    write(root, "go.mod", "module example.com/shop\n\ngo 1.21\n");
    write(
        root,
        "main.go",
        "package main\n\nimport (\n\t\"fmt\"\n\t\"example.com/shop/store\"\n\t\"example.com/shop/gone\"\n)\n",
    );
    write(
        root,
        "store/store.go",
        "package store\n\nimport \"example.com/shop/store/db\"\n",
    );
    write(root, "store/cart.go", "package store\n");
    write(root, "store/store_test.go", "package store\n");
    write(root, "libs/db/go.mod", "module example.com/shop/store/db\n");
    write(root, "libs/db/db.go", "package db\n");
    write(root, "tools/go.mod", "module example.com/tools\n");
    write(
        root,
        "tools/gen.go",
        "package tools\n\nimport \"example.com/shop/store\"\n",
    );
}

fn pairs(graph: &DepGraph) -> Vec<(String, String)> {
    graph
        .edges()
        .iter()
        .map(|e| (e.file.clone(), e.to.clone()))
        .collect()
}

fn reasons(graph: &DepGraph) -> Vec<(String, String, String)> {
    graph
        .unresolved(&|_| true)
        .into_iter()
        .map(|u| (u.file, u.target, u.reason))
        .collect()
}

fn outcome(graph: &DepGraph, file: &str, spec: &str) -> Outcome {
    graph
        .relationships_of(file)
        .iter()
        .find(|r| r.relationship.specifier == spec)
        .unwrap_or_else(|| panic!("no relationship {spec} in {file}"))
        .outcome
        .clone()
}

fn inputs(graph: &DepGraph) -> Value {
    graph.common_details()["resolution_inputs"].clone()
}

fn edge(file: &str, to: &str) -> (String, String) {
    (file.to_string(), to.to_string())
}

#[test]
fn the_go_fixture_gives_five_edges_and_one_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    shop(ws.path());
    for includes in [
        Includes::default(),
        Includes {
            tests: true,
            ..Includes::default()
        },
    ] {
        let graph = build(ws.path(), includes);
        assert_eq!(
            pairs(&graph),
            [
                edge("main.go", "store/cart.go"),
                edge("main.go", "store/store.go"),
                edge("store/store.go", "libs/db/db.go"),
                edge("tools/gen.go", "store/cart.go"),
                edge("tools/gen.go", "store/store.go"),
            ]
        );
        assert!(graph.edges().iter().all(|e| e.to != "store/store_test.go"));
        let main_edges: Vec<Value> = graph
            .edges()
            .iter()
            .filter(|e| e.file == "main.go")
            .map(|e| serde_json::to_value(&e.evidence).unwrap())
            .collect();
        let evidence = json!([{
            "range": "5:2-5:26", "kind": "import", "specifier": "example.com/shop/store"
        }]);
        assert_eq!(main_edges, [evidence.clone(), evidence]);
        assert_eq!(outcome(&graph, "main.go", "fmt"), Outcome::External);
        assert_eq!(
            reasons(&graph),
            [(
                "main.go".to_string(),
                "example.com/shop/gone".to_string(),
                "no Go package files for example.com/shop/gone in the workspace".to_string()
            )]
        );
        assert_eq!(
            inputs(&graph),
            json!([
                {"file": "go.mod", "kind": "go.mod"},
                {"file": "libs/db/go.mod", "kind": "go.mod"},
                {"file": "tools/go.mod", "kind": "go.mod"}
            ])
        );
    }
}

#[test]
fn without_the_nested_module_the_sub_package_is_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    shop(ws.path());
    fs::remove_file(ws.path().join("libs/db/go.mod")).unwrap();
    let graph = graph(ws.path());
    assert!(
        pairs(&graph)
            .iter()
            .all(|(file, _)| file != "store/store.go")
    );
    assert!(reasons(&graph).contains(&(
        "store/store.go".to_string(),
        "example.com/shop/store/db".to_string(),
        "no Go package files for example.com/shop/store/db in the workspace".to_string()
    )));
}

#[test]
fn two_valid_candidates_are_an_ambiguous_import() {
    let ws = tempfile::tempdir().unwrap();
    shop(ws.path());
    write(ws.path(), "store/db/x.go", "package db\n");
    let graph = graph(ws.path());
    assert!(
        pairs(&graph)
            .iter()
            .all(|(file, _)| file != "store/store.go")
    );
    assert!(reasons(&graph).contains(&(
        "store/store.go".to_string(),
        "example.com/shop/store/db".to_string(),
        "ambiguous import: example.com/shop, example.com/shop/store/db".to_string()
    )));
}

#[test]
fn a_directory_of_another_module_is_no_candidate() {
    let ws = tempfile::tempdir().unwrap();
    shop(ws.path());
    write(
        ws.path(),
        "main.go",
        "package main\n\nimport \"example.com/shop/tools\"\n",
    );
    let graph = graph(ws.path());
    assert!(pairs(&graph).iter().all(|(_, to)| to != "tools/gen.go"));
    assert_eq!(
        reasons(&graph),
        [(
            "main.go".to_string(),
            "example.com/shop/tools".to_string(),
            "no Go package files for example.com/shop/tools in the workspace".to_string()
        )]
    );
}

#[test]
fn the_language_specific_forms() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "go.mod",
        "module example.com/f\n\nreplace example.com/ext => ./third/ext\n",
    );
    write(root, "third/ext/go.mod", "module example.com/ext\n");
    for dir in ["third/ext/e.go", "p/p.go", "q/q.go", "r/r.go", "s/s.go"] {
        let package = dir.split('/').nth_back(1).unwrap();
        write(root, dir, &format!("package {package}\n"));
    }
    write(
        root,
        "main.go",
        "package main\n\nimport \"C\"\nimport `example.com/f/p`\nimport q \"example.com/f/q\"\nimport . \"example.com/f/r\"\nimport _ \"example.com/f/s\"\nimport \"example.com/ext\"\n",
    );
    let graph = graph(root);
    assert_eq!(outcome(&graph, "main.go", "C"), Outcome::External);
    assert_eq!(
        pairs(&graph),
        [
            edge("main.go", "p/p.go"),
            edge("main.go", "q/q.go"),
            edge("main.go", "r/r.go"),
            edge("main.go", "s/s.go"),
            edge("main.go", "third/ext/e.go"),
        ]
    );
    assert!(reasons(&graph).is_empty());
}

#[test]
fn a_replace_directive_adds_its_module_path() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "go.mod",
        "module example.com/f\n\nreplace (\n\texample.com/ext v1.0.0 => ../outside\n\texample.com/loc v1.2.3 => ./loc // local\n\texample.com/up => example.com/other v1.0.0\n)\n",
    );
    write(root, "loc/go.mod", "module example.com/different\n");
    write(root, "loc/sub/l.go", "package sub\n");
    write(
        root,
        "main.go",
        "package main\n\nimport (\n\t\"example.com/loc/sub\"\n\t\"example.com/ext\"\n\t\"example.com/up\"\n)\n",
    );
    let graph = graph(root);
    assert_eq!(pairs(&graph), [edge("main.go", "loc/sub/l.go")]);
    assert_eq!(
        outcome(&graph, "main.go", "example.com/ext"),
        Outcome::External
    );
    assert_eq!(
        outcome(&graph, "main.go", "example.com/up"),
        Outcome::External
    );
}

#[test]
fn a_file_under_no_go_mod() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "a.go",
        "package a\n\nimport (\n\t\"fmt\"\n\t\"example.com/x\"\n)\n",
    );
    let graph = graph(ws.path());
    assert_eq!(outcome(&graph, "a.go", "fmt"), Outcome::External);
    assert_eq!(
        outcome(&graph, "a.go", "example.com/x"),
        Outcome::Unresolved("file is under no go.mod in the workspace".to_string())
    );
    assert_eq!(inputs(&graph), json!([]));
}

#[test]
fn a_file_of_a_module_importing_another_module_is_external() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "go.mod", "module example.com/f\n");
    write(
        ws.path(),
        "a.go",
        "package a\n\nimport \"github.com/x/y\"\n",
    );
    let graph = graph(ws.path());
    assert_eq!(outcome(&graph, "a.go", "github.com/x/y"), Outcome::External);
}

#[test]
fn a_go_mod_under_testdata_adds_no_module_path() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(root, "testdata/m/go.mod", "module example.com/copy\n");
    write(root, "testdata/m/c.go", "package m\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/copy\"\n",
    );
    let graph = build(
        root,
        Includes {
            tests: true,
            ..Includes::default()
        },
    );
    assert_eq!(
        outcome(&graph, "main.go", "example.com/copy"),
        Outcome::External
    );
    assert_eq!(
        inputs(&graph),
        json!([{"file": "go.mod", "kind": "go.mod"}])
    );
}

#[test]
fn a_go_mod_under_a_test_directory_is_read_only_as_the_nearest() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(root, "tests/go.mod", "module example.com/f/fixture\n");
    write(root, "tests/fix/x.go", "package fix\n\nimport \"fmt\"\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/f/fixture/fix\"\n",
    );
    let graph = build(
        root,
        Includes {
            tests: true,
            ..Includes::default()
        },
    );
    assert_eq!(
        outcome(&graph, "main.go", "example.com/f/fixture/fix"),
        Outcome::Unresolved(
            "no Go package files for example.com/f/fixture/fix in the workspace".to_string()
        )
    );
    assert_eq!(outcome(&graph, "tests/fix/x.go", "fmt"), Outcome::External);
    assert_eq!(
        inputs(&graph),
        json!([
            {"file": "go.mod", "kind": "go.mod"},
            {"file": "tests/go.mod", "kind": "go.mod"}
        ])
    );
}

#[test]
fn a_directory_that_the_go_command_ignores_is_no_candidate() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(root, "_x/x.go", "package x\n");
    write(root, "testdata/t/t.go", "package t\n");
    write(
        root,
        "main.go",
        "package main\n\nimport (\n\t\"example.com/f/_x\"\n\t\"example.com/f/testdata/t\"\n)\n",
    );
    let graph = graph(root);
    assert!(graph.edges().is_empty());
    let reasons: Vec<String> = reasons(&graph).into_iter().map(|r| r.2).collect();
    assert_eq!(
        reasons,
        [
            "no Go package files for example.com/f/_x in the workspace",
            "no Go package files for example.com/f/testdata/t in the workspace"
        ]
    );
}

#[test]
fn a_vendored_go_mod_is_not_read() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(
        root,
        "vendor/example.com/v/go.mod",
        "module example.com/v\n",
    );
    write(root, "vendor/example.com/v/v.go", "package v\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/v\"\n",
    );
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "main.go", "example.com/v"),
        Outcome::External
    );
    assert_eq!(
        inputs(&graph),
        json!([{"file": "go.mod", "kind": "go.mod"}])
    );
}

#[test]
fn an_unparsable_go_mod_makes_externals_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    shop(ws.path());
    write(ws.path(), "tools/go.mod", "this is not a go.mod\n");
    let graph = graph(ws.path());
    assert_eq!(
        outcome(&graph, "main.go", "fmt"),
        Outcome::Unresolved("tools/go.mod could not be read".to_string())
    );
    let listed = inputs(&graph);
    assert_eq!(listed[2]["file"], "tools/go.mod");
    assert!(listed[2]["error"].is_string());
}

#[test]
fn an_unreadable_own_go_mod_makes_a_missing_package_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(root, "sub/go.mod", "this is not a go.mod\n");
    write(
        root,
        "sub/a.go",
        "package sub\n\nimport \"example.com/f/gone\"\n",
    );
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "sub/a.go", "example.com/f/gone"),
        Outcome::Unresolved("sub/go.mod could not be read".to_string())
    );
}

#[cfg(unix)]
#[test]
fn a_package_directory_that_leads_outside_is_external() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    write(&root, "go.mod", "module example.com/f\n");
    write(tmp.path(), "out/o.go", "package out\n");
    std::os::unix::fs::symlink(tmp.path().join("out"), root.join("out")).unwrap();
    write(
        &root,
        "main.go",
        "package main\n\nimport \"example.com/f/out\"\n",
    );
    let graph = graph(&root);
    assert_eq!(
        outcome(&graph, "main.go", "example.com/f/out"),
        Outcome::External
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_makes_an_import_under_no_module_path_unresolved() {
    use std::os::unix::fs::PermissionsExt;
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(root, "locked/go.mod", "module example.com/hid\n");
    write(
        root,
        "main.go",
        "package main\n\nimport (\n\t\"fmt\"\n\t\"example.com/hid\"\n)\n",
    );
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let result = std::panic::catch_unwind(|| graph(root));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let graph = result.unwrap();
    assert_eq!(
        outcome(&graph, "main.go", "example.com/hid"),
        Outcome::Unresolved("locked/ could not be read".to_string())
    );
    assert_eq!(outcome(&graph, "main.go", "fmt"), Outcome::External);
}

#[test]
fn a_vendored_go_mod_is_not_read_when_vendored_files_are_included() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(
        root,
        "vendor/example.com/v/go.mod",
        "module example.com/v\n",
    );
    write(root, "vendor/example.com/v/v.go", "package v\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/v\"\n",
    );
    let graph = build(
        root,
        Includes {
            vendored: true,
            ..Includes::default()
        },
    );
    assert_eq!(
        outcome(&graph, "main.go", "example.com/v"),
        Outcome::External
    );
    assert_eq!(
        inputs(&graph),
        json!([{"file": "go.mod", "kind": "go.mod"}])
    );
}

#[test]
fn an_import_path_with_an_empty_or_dot_segment_is_invalid() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/m\n");
    write(root, "p/p.go", "package p\n");
    write(root, "u/u.go", "package u\n");
    write(
        root,
        "main.go",
        "package main\n\nimport (\n\t\"example.com/m/p/\"\n\t\"example.com/m/p/../u\"\n)\n",
    );
    let graph = graph(root);
    assert!(graph.edges().is_empty());
    for path in ["example.com/m/p/", "example.com/m/p/../u"] {
        assert_eq!(
            outcome(&graph, "main.go", path),
            Outcome::Unresolved(format!("invalid import path: {path}"))
        );
    }
}

#[test]
fn files_that_the_go_command_ignores_make_no_package() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    write(root, "p/_a.go", "package p\n");
    write(root, "p/.b.go", "package p\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/f/p\"\n",
    );
    let graph = graph(root);
    assert!(graph.edges().is_empty());
    assert_eq!(
        outcome(&graph, "main.go", "example.com/f/p"),
        Outcome::Unresolved("no Go package files for example.com/f/p in the workspace".to_string())
    );
}

#[test]
fn an_ambiguous_import_names_each_module_path_once() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/f\n");
    for dir in ["a", "b"] {
        write(root, &format!("{dir}/go.mod"), "module example.com/d\n");
        write(root, &format!("{dir}/d.go"), "package d\n");
    }
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/d\"\n",
    );
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "main.go", "example.com/d"),
        Outcome::Unresolved("ambiguous import: example.com/d".to_string())
    );
}

#[test]
fn a_single_line_replace_with_a_version() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "go.mod",
        "module example.com/f\n\nreplace example.com/a v1.2.3 => ./b\n",
    );
    write(root, "b/go.mod", "module example.com/other\n");
    write(root, "b/b.go", "package a\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/a\"\n",
    );
    let graph = graph(root);
    assert_eq!(pairs(&graph), [edge("main.go", "b/b.go")]);
}

#[test]
fn a_major_version_module_path() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "go.mod", "module example.com/m/v2\n");
    write(root, "p/p.go", "package p\n");
    write(
        root,
        "main.go",
        "package main\n\nimport \"example.com/m/v2/p\"\n",
    );
    let graph = graph(root);
    assert_eq!(pairs(&graph), [edge("main.go", "p/p.go")]);
    assert!(reasons(&graph).is_empty());
}

#[test]
fn a_replace_directive_replaces_the_workspace_modules_of_its_path() {
    for replace in [
        "replace example.com/lib => ../fork\n",
        "replace example.com/lib v1.0.0 => ../fork\n",
    ] {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        write(root, "original/go.mod", "module example.com/lib\n");
        write(root, "original/lib.go", "package lib\n");
        write(root, "original/sub/s.go", "package sub\n");
        write(root, "fork/go.mod", "module example.com/lib\n");
        write(root, "fork/lib.go", "package lib\n");
        write(root, "fork/sub/s.go", "package sub\n");
        write(
            root,
            "app/go.mod",
            &format!("module example.com/app\n\nrequire example.com/lib v1.0.0\n\n{replace}"),
        );
        write(
            root,
            "app/main.go",
            "package main\n\nimport (\n\t\"example.com/lib\"\n\t\"example.com/lib/sub\"\n)\n",
        );
        let graph = graph(root);
        assert_eq!(
            outcome(&graph, "app/main.go", "example.com/lib"),
            Outcome::Resolved(vec!["fork/lib.go".to_string()]),
            "{replace}"
        );
        assert_eq!(
            outcome(&graph, "app/main.go", "example.com/lib/sub"),
            Outcome::Resolved(vec!["fork/sub/s.go".to_string()]),
            "{replace}"
        );
    }
}
