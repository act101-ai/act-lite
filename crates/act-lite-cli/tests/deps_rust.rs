//! Rust resolution: crates and roots, module trees, `use` paths, macro
//! forms, `extern crate`, `include!` and the `Cargo.toml` inputs.

use act_lite_cli::analyze::deps::rust::RustResolver;
use act_lite_cli::analyze::deps::{DepGraph, Family, Outcome, ResolveCx, Resolver, supports};
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::read::{Reader, SupportReader};
use act_lite_cli::analyze::run::{Request, RunContext};
use act_lite_parser::deps::Relationship;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn build(root: &Path, includes: Includes) -> DepGraph {
    build_with(root, includes, &RustResolver)
}

fn build_with(root: &Path, includes: Includes, resolver: &dyn Resolver) -> DepGraph {
    let cancel = CancellationToken::new();
    let request = Request::new(root, &[], &[], &[], includes, 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &includes, &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, includes.tests, &cancel);
    DepGraph::build_with(&mut reader, &ctx, &[resolver], &cancel).unwrap()
}

fn graph(root: &Path) -> DepGraph {
    build(root, Includes::default())
}

fn with_tests() -> Includes {
    Includes {
        tests: true,
        ..Includes::default()
    }
}

/// The `rs` fixture.
fn rs(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"rs-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nserde = \"1\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "mod net;\nmod util;\nmod gone;\npub use crate::net::http::Client;\nuse serde::Serialize;\nuse std::fmt;\nuse mystery::Thing;\n",
    );
    write(root, "src/net/mod.rs", "pub mod http;\n");
    write(
        root,
        "src/net/http.rs",
        "use super::super::util::helper;\npub struct Client;\n",
    );
    write(root, "src/util.rs", "pub fn helper() {}\n");
    write(root, "src/main.rs", "use rs_fixture::util::helper;\n");
    write(root, "src/extra.rs", "use crate::util::helper;\n");
}

fn pairs(graph: &DepGraph) -> Vec<(String, String)> {
    graph
        .edges()
        .iter()
        .map(|e| (e.file.clone(), e.to.clone()))
        .collect()
}

fn edge(file: &str, to: &str) -> (String, String) {
    (file.to_string(), to.to_string())
}

fn reasons(graph: &DepGraph) -> Vec<(String, String, String)> {
    graph
        .unresolved(&|_| true)
        .into_iter()
        .map(|u| (u.file, u.target, u.reason))
        .collect()
}

fn reason(file: &str, target: &str, reason: &str) -> (String, String, String) {
    (file.to_string(), target.to_string(), reason.to_string())
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

fn resolved(files: &[&str]) -> Outcome {
    Outcome::Resolved(files.iter().map(|f| f.to_string()).collect())
}

fn unresolved(reason: &str) -> Outcome {
    Outcome::Unresolved(reason.to_string())
}

fn counts(graph: &DepGraph) -> Value {
    graph.common_details()["relationships"].clone()
}

fn kinds_of(graph: &DepGraph, file: &str, to: &str) -> Vec<&'static str> {
    let edge = graph
        .edges()
        .iter()
        .find(|e| e.file == file && e.to == to)
        .unwrap_or_else(|| panic!("no edge {file} -> {to}"));
    let mut kinds: Vec<&str> = edge.kinds.iter().map(|k| k.as_str()).collect();
    kinds.sort_unstable();
    kinds
}

#[test]
fn the_rs_fixture_gives_six_edges_two_externals_and_three_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    rs(ws.path());
    let graph = graph(ws.path());
    assert_eq!(
        pairs(&graph),
        [
            edge("src/lib.rs", "src/net/http.rs"),
            edge("src/lib.rs", "src/net/mod.rs"),
            edge("src/lib.rs", "src/util.rs"),
            edge("src/main.rs", "src/util.rs"),
            edge("src/net/http.rs", "src/util.rs"),
            edge("src/net/mod.rs", "src/net/http.rs"),
        ]
    );
    let ranges: Vec<(String, String, Vec<String>)> = graph
        .edges()
        .iter()
        .map(|e| {
            let ranges = e.evidence.iter().map(|v| v.range.to_string()).collect();
            (e.file.clone(), e.to.clone(), ranges)
        })
        .collect();
    let range =
        |file: &str, to: &str, r: &str| (file.to_string(), to.to_string(), vec![r.to_string()]);
    assert_eq!(
        ranges,
        [
            range("src/lib.rs", "src/net/http.rs", "4:1-4:34"),
            range("src/lib.rs", "src/net/mod.rs", "1:1-1:9"),
            range("src/lib.rs", "src/util.rs", "2:1-2:10"),
            range("src/main.rs", "src/util.rs", "1:1-1:30"),
            range("src/net/http.rs", "src/util.rs", "1:1-1:32"),
            range("src/net/mod.rs", "src/net/http.rs", "1:1-1:14"),
        ]
    );
    let unresolved_ranges: Vec<String> = graph
        .unresolved(&|_| true)
        .iter()
        .map(|u| u.range.to_string())
        .collect();
    assert_eq!(unresolved_ranges, ["1:1-1:25", "3:1-3:10", "7:1-7:20"]);
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/net/http.rs"), ["use"]);
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/net/mod.rs"), ["module"]);
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/util.rs"), ["module"]);
    assert_eq!(
        kinds_of(&graph, "src/net/mod.rs", "src/net/http.rs"),
        ["module"]
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "serde::Serialize"),
        Outcome::External
    );
    assert_eq!(outcome(&graph, "src/lib.rs", "std::fmt"), Outcome::External);
    assert_eq!(
        reasons(&graph),
        [
            reason(
                "src/extra.rs",
                "crate::util::helper",
                "file is not reached from a crate root"
            ),
            reason(
                "src/lib.rs",
                "gone",
                "neither src/gone.rs nor src/gone/mod.rs exists"
            ),
            reason(
                "src/lib.rs",
                "mystery::Thing",
                "first path segment is not a module, crate or dependency: mystery"
            ),
        ]
    );
    assert_eq!(
        counts(&graph),
        json!({"total": 11, "resolved": 6, "external": 2, "unresolved": 3})
    );
    assert_eq!(
        graph.common_details()["resolution_inputs"],
        json!([{"file": "Cargo.toml", "kind": "Cargo.toml"}])
    );
}

#[test]
fn an_unparsable_cargo_toml_leaves_every_relationship_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    rs(ws.path());
    write(ws.path(), "Cargo.toml", "[package");
    let graph = graph(ws.path());
    assert!(graph.edges().is_empty());
    let all = reasons(&graph);
    assert_eq!(all.len(), 11);
    assert!(
        all.iter()
            .all(|(_, _, r)| r == "Cargo.toml could not be read")
    );
    assert_eq!(
        counts(&graph),
        json!({"total": 11, "resolved": 0, "external": 0, "unresolved": 11})
    );
    let inputs = graph.common_details()["resolution_inputs"].clone();
    assert_eq!(inputs.as_array().unwrap().len(), 1);
    assert_eq!(inputs[0]["file"], "Cargo.toml");
    assert_eq!(inputs[0]["kind"], "Cargo.toml");
    assert!(inputs[0]["error"].is_string());
}

#[test]
fn a_removed_test_module_still_builds_the_tree() {
    let ws = tempfile::tempdir().unwrap();
    rs(ws.path());
    write(
        ws.path(),
        "src/lib.rs",
        "mod net;\nmod util;\nmod gone;\npub use crate::net::http::Client;\nuse serde::Serialize;\nuse std::fmt;\nuse mystery::Thing;\n#[cfg(test)] mod tests;\n",
    );
    write(ws.path(), "src/tests.rs", "use super::*;\n");

    let graph = graph(ws.path());
    let edges = pairs(&graph);
    assert!(!edges.contains(&edge("src/lib.rs", "src/tests.rs")));
    assert!(edges.contains(&edge("src/tests.rs", "src/lib.rs")));
    assert_eq!(kinds_of(&graph, "src/tests.rs", "src/lib.rs"), ["use"]);
    assert!(reasons(&graph).iter().all(|(f, _, _)| f != "src/tests.rs"));
    assert_eq!(reasons(&graph).len(), 3);

    let graph = build(ws.path(), with_tests());
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/tests.rs"), ["module"]);
    assert!(pairs(&graph).contains(&edge("src/tests.rs", "src/lib.rs")));
    assert!(reasons(&graph).iter().all(|(f, _, _)| f != "src/tests.rs"));
}

#[test]
fn the_language_specific_forms() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"forms-rs\"\nversion = \"0.1.0\"\n\n[lib]\nname = \"frs\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "mod net;\n#[path = \"other/place.rs\"]\nmod moved;\ncfg_feature! { pub mod gated; }\npub use net::{self, Conn as C};\nextern crate alloc;\nstatic S: &str = include_str!(\"data.txt\");\n",
    );
    write(root, "src/net.rs", "pub mod http;\npub struct Conn;\n");
    write(root, "src/net/http.rs", "");
    write(root, "src/other/place.rs", "mod inner;\n");
    write(root, "src/other/inner.rs", "");
    write(root, "src/gated.rs", "");
    write(root, "src/data.txt", "");
    write(root, "src/main.rs", "use frs::net::Conn;\n");
    let graph = graph(root);
    assert_eq!(
        pairs(&graph),
        [
            edge("src/lib.rs", "src/data.txt"),
            edge("src/lib.rs", "src/gated.rs"),
            edge("src/lib.rs", "src/net.rs"),
            edge("src/lib.rs", "src/other/place.rs"),
            edge("src/main.rs", "src/net.rs"),
            edge("src/net.rs", "src/net/http.rs"),
            edge("src/other/place.rs", "src/other/inner.rs"),
        ]
    );
    let lib_net = graph
        .edges()
        .iter()
        .find(|e| e.file == "src/lib.rs" && e.to == "src/net.rs")
        .unwrap();
    assert_eq!(lib_net.evidence_total, 3);
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/net.rs"),
        ["module", "use"]
    );
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/gated.rs"), ["module"]);
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/data.txt"), ["include"]);
    let data = graph
        .edges()
        .iter()
        .find(|e| e.to == "src/data.txt")
        .unwrap();
    assert!(!data.to_analyzed);
    assert_eq!(outcome(&graph, "src/lib.rs", "alloc"), Outcome::External);
    assert!(reasons(&graph).is_empty());
}

#[test]
fn a_use_through_a_module_that_a_macro_declares() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"rt\"\nversion = \"0.1.0\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "cfg_rt! {\n    pub mod a;\n}\nuse crate::a::X;\nuse crate::a::Y;\n",
    );
    write(root, "src/a.rs", "pub struct X;\n");
    let graph = graph(root);
    assert_eq!(pairs(&graph), [edge("src/lib.rs", "src/a.rs")]);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "crate::a::X"),
        resolved(&["src/a.rs"])
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "crate::a::Y"),
        unresolved("Y is neither a module nor an item of src/a.rs")
    );
}

#[test]
fn a_path_attribute_inside_a_macro_replaces_the_probe() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "cfg_x! {\n    #[path = \"other.rs\"]\n    mod x;\n}\nuse crate::x::Thing;\n",
    );
    write(root, "src/other.rs", "pub struct Thing;\n");
    write(root, "src/x.rs", "pub struct Decoy;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "x"),
        resolved(&["src/other.rs"])
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "crate::x::Thing"),
        resolved(&["src/other.rs"])
    );
    assert_eq!(pairs(&graph), [edge("src/lib.rs", "src/other.rs")]);
}

#[test]
fn a_comment_inside_a_macro_keeps_the_path_attribute() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "wrapper! { #[path = \"other.rs\"] /* note */ mod x; }\n",
    );
    write(root, "src/other.rs", "");
    write(root, "src/x.rs", "");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "x"),
        resolved(&["src/other.rs"])
    );
    assert_eq!(pairs(&graph), [edge("src/lib.rs", "src/other.rs")]);
}

#[test]
fn a_use_through_a_missing_module_file() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"io\"\nversion = \"0.1.0\"\n",
    );
    write(root, "src/lib.rs", "mod sys;\nuse crate::sys::Fd;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "crate::sys::Fd"),
        unresolved("module sys was not found")
    );
}

#[test]
fn a_local_module_wins_and_a_renamed_path_dependency_resolves() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nbar = { package = \"foo\", path = \"../foo\" }\n",
    );
    write(
        root,
        "app/src/lib.rs",
        "mod config;\nuse config::Settings;\nuse bar::Thing;\nextern crate bar;\n",
    );
    write(root, "app/src/config.rs", "pub struct Settings;\n");
    write(
        root,
        "config/Cargo.toml",
        "[package]\nname = \"config\"\nversion = \"0.1.0\"\n",
    );
    write(root, "config/src/lib.rs", "pub struct Settings;\n");
    write(
        root,
        "foo/Cargo.toml",
        "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n",
    );
    write(root, "foo/src/lib.rs", "pub struct Thing;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "config::Settings"),
        resolved(&["app/src/config.rs"])
    );
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "bar::Thing"),
        resolved(&["foo/src/lib.rs"])
    );
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "bar"),
        resolved(&["foo/src/lib.rs"])
    );
}

#[test]
fn a_fixture_crate_under_a_test_directory_does_not_capture_a_dependency() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    rs(root);
    write(
        root,
        "tests/fixtures/serde/Cargo.toml",
        "[package]\nname = \"serde\"\nversion = \"1.0.0\"\n",
    );
    write(
        root,
        "tests/fixtures/serde/src/lib.rs",
        "pub trait Serialize {}\n",
    );
    for includes in [Includes::default(), with_tests()] {
        let graph = build(root, includes);
        assert_eq!(
            outcome(&graph, "src/lib.rs", "serde::Serialize"),
            Outcome::External
        );
    }
}

#[test]
fn a_workspace_dependency_with_a_path_resolves_into_the_member() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\nmy-core = { path = \"crates/core\" }\n",
    );
    write(
        root,
        "crates/core/Cargo.toml",
        "[package]\nname = \"my-core\"\nversion = \"0.1.0\"\n",
    );
    write(root, "crates/core/src/lib.rs", "pub fn run() {}\n");
    write(
        root,
        "crates/cli/Cargo.toml",
        "[package]\nname = \"cli\"\nversion = \"0.1.0\"\n\n[dependencies]\nmy-core.workspace = true\n",
    );
    write(root, "crates/cli/src/main.rs", "use my_core::run;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "crates/cli/src/main.rs", "my_core::run"),
        resolved(&["crates/core/src/lib.rs"])
    );
}

#[test]
fn a_registry_dependency_named_as_a_workspace_crate_resolves_into_it() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nshared = \"1\"\n\n[patch.crates-io]\nshared = { path = \"../shared\" }\n",
    );
    write(root, "app/src/lib.rs", "use shared::Item;\n");
    write(
        root,
        "shared/Cargo.toml",
        "[package]\nname = \"shared\"\nversion = \"1.0.0\"\n",
    );
    write(root, "shared/src/lib.rs", "pub struct Item;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "shared::Item"),
        resolved(&["shared/src/lib.rs"])
    );

    write(
        root,
        "other/Cargo.toml",
        "[package]\nname = \"shared\"\nversion = \"1.0.0\"\n",
    );
    write(root, "other/src/lib.rs", "pub struct Item;\n");
    let graph = build(root, Includes::default());
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "shared::Item"),
        unresolved("lib name shared is declared by more than one crate")
    );
}

#[cfg(unix)]
#[test]
fn a_hidden_workspace_entry_leaves_a_dependency_name_unresolved() {
    use std::os::unix::fs::PermissionsExt;
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    rs(root);
    write(root, "locked/Cargo.toml", "[package]\nname = \"serde\"\n");
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let graph = graph(root);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        outcome(&graph, "src/lib.rs", "serde::Serialize"),
        unresolved("locked/ could not be read")
    );
    assert_eq!(outcome(&graph, "src/lib.rs", "std::fmt"), Outcome::External);
}

fn package(root: &Path, dir: &str, name: &str, rest: &str) {
    write(
        root,
        &format!("{dir}/Cargo.toml"),
        &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n{rest}"),
    );
}

#[test]
fn a_vendored_crate_is_outside_the_project() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(
        root,
        ".",
        "app",
        "\n[dependencies]\nfoo = { path = \"vendor/foo\" }\nserde = \"1\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "use foo::X;\nuse std::fmt;\nuse serde::Serialize;\n",
    );
    package(root, "vendor/foo", "foo", "");
    write(root, "vendor/foo/src/lib.rs", "pub struct X;\n");
    package(root, "vendor/serde", "serde", "");
    write(root, "vendor/serde/src/lib.rs", "pub trait Serialize {}\n");
    let graph = graph(root);
    assert_eq!(outcome(&graph, "src/lib.rs", "foo::X"), Outcome::External);
    assert_eq!(outcome(&graph, "src/lib.rs", "std::fmt"), Outcome::External);
    assert_eq!(
        graph.common_details()["resolution_inputs"],
        json!([{"file": "Cargo.toml", "kind": "Cargo.toml"}])
    );
    let includes = Includes {
        vendored: true,
        ..Includes::default()
    };
    let graph = build(root, includes);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "serde::Serialize"),
        Outcome::External
    );
    assert_eq!(outcome(&graph, "src/lib.rs", "foo::X"), Outcome::External);
}

#[test]
fn a_removed_test_module_keeps_its_path_attribute() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "#[cfg(test)]\n#[path = \"unit/checks.rs\"]\nmod checks;\npub struct A;\n",
    );
    write(root, "src/unit/checks.rs", "use super::A;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/unit/checks.rs", "super::A"),
        resolved(&["src/lib.rs"])
    );
}

#[test]
fn a_removed_inline_test_module_keeps_its_module_declarations() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "pub struct A;\n#[cfg(test)]\nmod checks {\n    mod helper;\n}\n",
    );
    write(root, "src/checks/helper.rs", "use crate::A;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/checks/helper.rs", "crate::A"),
        resolved(&["src/lib.rs"])
    );
    assert!(graph.relationships_of("src/lib.rs").is_empty());
    let graph = build(root, with_tests());
    assert_eq!(
        outcome(&graph, "src/checks/helper.rs", "crate::A"),
        resolved(&["src/lib.rs"])
    );
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/checks/helper.rs"),
        ["module"]
    );
}

#[test]
fn removed_inline_modules_nest_and_keep_path_attributes() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "pub struct A;\n",
            "mod outer {\n",
            "    #[cfg(test)]\n",
            "    mod t {\n",
            "        mod h;\n",
            "    }\n",
            "}\n",
            "#[cfg(test)]\n",
            "#[path = \"unit\"]\n",
            "mod checks {\n",
            "    mod inner {\n",
            "        #[path = \"deep/p.rs\"]\n",
            "        mod p;\n",
            "        mod q;\n",
            "    }\n",
            "    wrapper! { mod m; }\n",
            "}\n",
            "#[path = \"before.rs\"]\n",
            "#[cfg(test)]\n",
            "mod b;\n",
        ),
    );
    let files = [
        "src/outer/t/h.rs",
        "src/unit/inner/deep/p.rs",
        "src/unit/inner/q.rs",
        "src/unit/m.rs",
        "src/before.rs",
    ];
    for file in files {
        write(root, file, "use crate::A;\n");
    }
    for includes in [Includes::default(), with_tests()] {
        let graph = build(root, includes);
        for file in files {
            assert_eq!(
                outcome(&graph, file, "crate::A"),
                resolved(&["src/lib.rs"]),
                "{file}, tests {}",
                includes.tests
            );
        }
        assert_eq!(
            graph.relationships_of("src/lib.rs").is_empty(),
            !includes.tests
        );
    }
}

#[test]
fn a_dependency_in_several_target_tables_resolves_to_every_crate() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(
        root,
        "app",
        "app",
        "\n[target.'cfg(unix)'.dependencies]\nsys = { package = \"sys-unix\", path = \"../sys-unix\" }\n\n[target.'cfg(windows)'.dependencies]\nsys = { package = \"sys-win\", path = \"../sys-win\" }\n",
    );
    write(root, "app/src/lib.rs", "use sys::X;\n");
    package(root, "sys-unix", "sys-unix", "");
    write(root, "sys-unix/src/lib.rs", "pub struct X;\n");
    package(root, "sys-win", "sys-win", "");
    write(root, "sys-win/src/lib.rs", "pub struct X;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "sys::X"),
        resolved(&["sys-unix/src/lib.rs", "sys-win/src/lib.rs"])
    );
}

#[test]
fn a_use_through_module_variants_resolves_to_every_file() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "#[cfg(unix)]\n#[path = \"sys/unix.rs\"]\nmod sys;\n#[cfg(windows)]\n#[path = \"sys/windows.rs\"]\nmod sys;\nuse sys::Fd;\n",
    );
    write(root, "src/sys/unix.rs", "pub struct Fd;\n");
    write(root, "src/sys/windows.rs", "pub struct Fd;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "sys::Fd"),
        resolved(&["src/sys/unix.rs", "src/sys/windows.rs"])
    );
}

#[test]
fn a_module_file_excluded_as_tests_is_not_unreadable() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(root, "src/lib.rs", "mod tests;\nmod test_util;\n");
    write(
        root,
        "src/test_util.rs",
        "#![cfg(test)]\npub struct Helper;\n",
    );
    write(root, "src/tests.rs", "use crate::test_util::Helper;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/tests.rs", "crate::test_util::Helper"),
        unresolved("src/test_util.rs: excluded as tests")
    );
    let graph = build(root, with_tests());
    assert_eq!(
        outcome(&graph, "src/tests.rs", "crate::test_util::Helper"),
        resolved(&["src/test_util.rs"])
    );
}

/// The Rust resolver, recording which of `watched` were read with `source`
/// while `src/lib.rs` was resolved.
struct Watching {
    watched: Vec<&'static str>,
    read: Mutex<BTreeSet<String>>,
}

impl Resolver for Watching {
    fn family(&self) -> Family {
        Family::Rust
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        let outcome = RustResolver.resolve(file, rel, cx);
        if file == "src/lib.rs" {
            for watched in &self.watched {
                if !cx.removed(watched).is_empty() {
                    self.read.lock().unwrap().insert(watched.to_string());
                }
            }
        }
        outcome
    }
}

#[test]
fn only_the_module_files_of_the_workspace_trees_are_read() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    let marker = "#[cfg(test)]\nmod t {}\n";
    write(
        root,
        "src/lib.rs",
        "mod a;\nmod b;\nuse crate::a::X;\nuse crate::gone::Y;\n",
    );
    write(root, "src/a.rs", &format!("pub struct X;\n{marker}"));
    write(root, "src/b.rs", marker);
    write(root, "tests/t.rs", &format!("mod helper;\n{marker}"));
    write(root, "tests/helper.rs", marker);
    // An analyzed file that no tree reaches is read for its own tree; the
    // lib of a crate that `app` does not depend on is read, because its
    // tree could load a file of `app` through `#[path]`.
    write(root, "src/c.rs", marker);
    package(root, "other", "other", "");
    write(root, "other/src/lib.rs", marker);
    let watching = Watching {
        watched: vec![
            "src/a.rs",
            "src/b.rs",
            "src/c.rs",
            "tests/t.rs",
            "tests/helper.rs",
            "other/src/lib.rs",
        ],
        read: Mutex::new(BTreeSet::new()),
    };
    build_with(root, Includes::default(), &watching);
    let read: Vec<String> = watching.read.into_inner().unwrap().into_iter().collect();
    assert_eq!(
        read,
        [
            "other/src/lib.rs",
            "src/a.rs",
            "src/b.rs",
            "src/c.rs",
            "tests/helper.rs",
            "tests/t.rs"
        ]
    );
}

#[test]
fn an_absolute_path_is_not_resolved() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(
        root,
        ".",
        "app",
        "\n[dependencies]\nfar = { path = \"/opt/far\" }\n",
    );
    write(
        root,
        "src/lib.rs",
        "#[path = \"/abs/x.rs\"]\nmod x;\nstatic S: &str = include_str!(\"/etc/hosts\");\nuse far::Y;\n",
    );
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "x"),
        unresolved("absolute path is not resolved: /abs/x.rs")
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "/etc/hosts"),
        unresolved("absolute path is not resolved: /etc/hosts")
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "far::Y"),
        unresolved("absolute path is not resolved: /opt/far")
    );
}

#[test]
fn a_use_through_a_module_outside_the_workspace_is_external() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "#[path = \"../../outside.rs\"]\nmod o;\nuse o::X;\n",
    );
    let graph = graph(root);
    assert_eq!(outcome(&graph, "src/lib.rs", "o"), Outcome::External);
    assert_eq!(outcome(&graph, "src/lib.rs", "o::X"), Outcome::External);
}

#[test]
fn duplicates_are_decided_on_lib_names() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(
        root,
        "app",
        "app",
        "\n[dependencies]\nshared = \"1\"\nfoo-bar = \"1\"\n\n[dev-dependencies]\nfoo_bar = { path = \"../foo_bar\" }\n",
    );
    write(
        root,
        "app/src/lib.rs",
        "use one::Item;\nuse two::Item as Second;\nuse shared::Item as Third;\nuse foo_bar::Z;\n",
    );
    package(root, "one", "shared", "\n[lib]\nname = \"one\"\n");
    write(root, "one/src/lib.rs", "pub struct Item;\n");
    package(root, "two", "shared", "\n[lib]\nname = \"two\"\n");
    write(root, "two/src/lib.rs", "pub struct Item;\n");
    package(root, "foo-bar", "foo-bar", "");
    write(root, "foo-bar/src/lib.rs", "pub struct Z;\n");
    package(root, "foo_bar", "foo_bar", "");
    write(root, "foo_bar/src/lib.rs", "pub struct Z;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "one::Item"),
        resolved(&["one/src/lib.rs"])
    );
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "two::Item"),
        resolved(&["two/src/lib.rs"])
    );
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "shared::Item"),
        unresolved("first path segment is not a module, crate or dependency: shared"),
        "a dependency is named by the lib name of its crate"
    );
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "foo_bar::Z"),
        unresolved("lib name foo_bar is declared by more than one crate")
    );
}

#[test]
fn toml_strings_are_unescaped() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"esc\\u0061pe\"\nversion = \"0.1.0\"\n\n[lib]\npath = \"\"\"\nsrc/root.rs\"\"\"\n",
    );
    write(root, "src/root.rs", "pub struct X;\n");
    write(root, "src/main.rs", "use escape::X;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/main.rs", "escape::X"),
        resolved(&["src/root.rs"])
    );
}

/// The graph of `paths` only.
fn graph_of(root: &Path, paths: &[&str]) -> DepGraph {
    let paths: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
    let cancel = CancellationToken::new();
    let includes = Includes::default();
    let request = Request::new(root, &paths, &[], &[], includes, 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &includes, &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    DepGraph::build_with(&mut reader, &ctx, &[&RustResolver], &cancel).unwrap()
}

#[test]
fn a_path_attribute_outside_the_parent_directory_is_reached_in_any_order() {
    for shared in ["shared.rs", "tools/shared.rs"] {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        package(root, ".", "app", "");
        write(
            root,
            "src/lib.rs",
            &format!("#[path = \"../{shared}\"]\nmod shared;\nmod a;\npub struct X;\n"),
        );
        write(root, "src/a.rs", "use crate::X;\n");
        write(root, shared, "use crate::X;\n");
        for scope in [&[][..], &[shared][..], &["src/a.rs", shared][..]] {
            let graph = graph_of(root, scope);
            assert_eq!(
                outcome(&graph, shared, "crate::X"),
                resolved(&["src/lib.rs"]),
                "{shared} in {scope:?}"
            );
            assert_eq!(kinds_of(&graph, shared, "src/lib.rs"), ["use"]);
        }
    }
}

/// A file in the lib tree and the bin tree, where each tree names its own
/// `imp` module.
fn shared_in_two_trees(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "#[path = \"../shared.rs\"]\nmod shared;\n#[path = \"imp_lib.rs\"]\nmod imp;\n",
    );
    write(
        root,
        "src/main.rs",
        "#[path = \"../shared.rs\"]\nmod shared;\n#[path = \"imp_bin.rs\"]\nmod imp;\n",
    );
    write(root, "src/imp_lib.rs", "pub struct Thing;\n");
    write(root, "src/imp_bin.rs", "pub struct Thing;\n");
    write(root, "shared.rs", "use crate::imp::Thing;\n");
}

#[test]
fn a_use_in_several_trees_resolves_in_every_tree() {
    let ws = tempfile::tempdir().unwrap();
    shared_in_two_trees(ws.path());
    let g = graph(ws.path());
    assert_eq!(
        outcome(&g, "shared.rs", "crate::imp::Thing"),
        resolved(&["src/imp_bin.rs", "src/imp_lib.rs"])
    );
}

/// A lib, an example and a test root that each load `src/shared.rs` through
/// `#[path]` and name their own `imp` module.
fn shared_in_scoped_roots(root: &Path) {
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "#[path = \"imp_lib.rs\"]\nmod imp;\nmod shared;\n",
    );
    for (root_file, imp) in [
        ("examples/demo.rs", "imp_ex.rs"),
        ("tests/it.rs", "imp_t.rs"),
    ] {
        write(
            root,
            root_file,
            &format!(
                "#[path = \"../src/shared.rs\"]\nmod shared;\n#[path = \"../src/{imp}\"]\nmod imp;\n"
            ),
        );
        write(root, &format!("src/{imp}"), "pub struct Thing;\n");
    }
    write(root, "src/imp_lib.rs", "pub struct Thing;\n");
    write(root, "src/shared.rs", "use crate::imp::Thing;\n");
}

#[test]
fn a_root_outside_the_file_directory_resolves_the_file_in_its_tree() {
    let ws = tempfile::tempdir().unwrap();
    shared_in_scoped_roots(ws.path());
    // A test root is a crate root whether or not test files are analyzed.
    for includes in [Includes::default(), with_tests()] {
        assert_eq!(
            outcome(
                &build(ws.path(), includes),
                "src/shared.rs",
                "crate::imp::Thing"
            ),
            resolved(&["src/imp_ex.rs", "src/imp_lib.rs", "src/imp_t.rs"]),
            "{includes:?}"
        );
    }
}

#[test]
fn a_file_loaded_as_two_modules_of_one_tree_resolves_in_each() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(root, "src/lib.rs", "mod a;\nmod b;\n");
    for m in ["a", "b"] {
        write(
            root,
            &format!("src/{m}.rs"),
            &format!("#[path = \"shared.rs\"]\nmod shared;\n#[path = \"imp_{m}.rs\"]\nmod imp;\n"),
        );
        write(root, &format!("src/imp_{m}.rs"), "pub struct Thing;\n");
    }
    write(root, "src/shared.rs", "use super::imp::Thing;\n");
    assert_eq!(
        outcome(&graph(root), "src/shared.rs", "super::imp::Thing"),
        resolved(&["src/imp_a.rs", "src/imp_b.rs"])
    );
}

#[test]
fn a_file_that_loads_itself_through_path_attributes_terminates() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(root, "src/lib.rs", "mod a;\nuse crate::a::b::again::X;\n");
    write(
        root,
        "src/a.rs",
        "#[path = \"b.rs\"]\nmod b;\npub struct X;\n",
    );
    write(
        root,
        "src/b.rs",
        "#[path = \"a.rs\"]\nmod again;\npub struct Y;\n",
    );
    let g = graph(root);
    assert_eq!(
        outcome(&g, "src/lib.rs", "crate::a::b::again::X"),
        resolved(&["src/a.rs"])
    );
    assert_eq!(outcome(&g, "src/b.rs", "again"), resolved(&["src/a.rs"]));
}

#[test]
fn a_file_outside_its_package_is_resolved_in_the_tree_that_loads_it() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, "app", "app", "");
    write(
        root,
        "app/src/lib.rs",
        "#[path = \"../../shared/common.rs\"]\nmod common;\npub struct X;\n",
    );
    write(root, "shared/common.rs", "use crate::X;\n");
    for scope in [&[][..], &["shared/common.rs"][..]] {
        let graph = graph_of(root, scope);
        assert_eq!(
            outcome(&graph, "shared/common.rs", "crate::X"),
            resolved(&["app/src/lib.rs"]),
            "{scope:?}"
        );
        assert_eq!(
            kinds_of(&graph, "shared/common.rs", "app/src/lib.rs"),
            ["use"]
        );
    }
}

#[test]
fn a_file_of_another_package_resolves_with_the_package_of_its_tree() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(
        root,
        "app",
        "app",
        "\n[dependencies]\ndep = { path = \"../dep\" }\n",
    );
    write(
        root,
        "app/src/lib.rs",
        "#[path = \"../../shared/common.rs\"]\nmod common;\npub struct X;\n",
    );
    package(root, "dep", "dep", "");
    write(root, "dep/src/lib.rs", "pub struct Y;\n");
    package(root, "shared", "shared", "");
    write(root, "shared/src/lib.rs", "pub struct S;\n");
    write(
        root,
        "shared/common.rs",
        "use crate::X;\nuse dep::Y;\nuse app::X as Z;\n",
    );
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "shared/common.rs", "crate::X"),
        resolved(&["app/src/lib.rs"])
    );
    assert_eq!(
        outcome(&graph, "shared/common.rs", "dep::Y"),
        resolved(&["dep/src/lib.rs"])
    );
    // The lib tree of `app` does not name its own lib.
    assert_eq!(
        outcome(&graph, "shared/common.rs", "app::X"),
        unresolved("first path segment is not a module, crate or dependency: app")
    );
}

#[test]
fn raw_identifiers_resolve_as_their_names() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(
        root,
        ".",
        "app",
        "\n[dependencies]\ndep = { path = \"dep\" }\n",
    );
    write(
        root,
        "src/lib.rs",
        "mod r#type;\nmod r#match;\nw! { mod r#gen; }\nmod r#inl {\n    mod r#x;\n}\nextern crate r#dep;\nuse crate::r#type::r#Foo;\nuse crate::r#type::r#run;\nuse r#type::{Bar};\nuse crate::r#gen::Z;\nuse r#dep::Y;\nuse crate::r#inl::r#x::W;\n",
    );
    write(
        root,
        "src/type.rs",
        "pub struct r#Foo;\npub struct Bar;\npub fn r#run() {}\n",
    );
    write(root, "src/match/mod.rs", "");
    write(root, "src/gen.rs", "pub struct Z;\n");
    write(root, "src/inl/x.rs", "pub struct W;\n");
    package(root, "dep", "dep", "");
    write(root, "dep/src/lib.rs", "pub struct Y;\n");
    let graph = graph(root);
    for (target, files) in [
        ("r#type", "src/type.rs"),
        ("r#match", "src/match/mod.rs"),
        ("r#gen", "src/gen.rs"),
        ("r#x", "src/inl/x.rs"),
        ("r#dep", "dep/src/lib.rs"),
        ("crate::r#type::r#Foo", "src/type.rs"),
        ("crate::r#type::r#run", "src/type.rs"),
        ("r#type::Bar", "src/type.rs"),
        ("crate::r#gen::Z", "src/gen.rs"),
        ("r#dep::Y", "dep/src/lib.rs"),
        ("crate::r#inl::r#x::W", "src/inl/x.rs"),
    ] {
        assert_eq!(
            outcome(&graph, "src/lib.rs", target),
            resolved(&[files]),
            "{target}"
        );
    }
    assert!(reasons(&graph).is_empty(), "{:?}", reasons(&graph));
}

#[test]
fn a_raw_path_attribute_replaces_the_probe() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "#[r#path = \"other.rs\"]\nmod x;\nw! { #[r#path = \"other2.rs\"] mod y; }\n",
    );
    for file in ["src/other.rs", "src/other2.rs", "src/x.rs", "src/y.rs"] {
        write(root, file, "");
    }
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "x"),
        resolved(&["src/other.rs"])
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "y"),
        resolved(&["src/other2.rs"])
    );
}

#[test]
fn a_comment_inside_a_path_attribute_keeps_it() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "#[/* a */ path /* c */ = \"other.rs\"]\nmod x;\nwrapper! { #[path /* c */ = \"other2.rs\"] mod y; }\n",
    );
    for file in ["src/other.rs", "src/other2.rs", "src/x.rs", "src/y.rs"] {
        write(root, file, "");
    }
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "x"),
        resolved(&["src/other.rs"])
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "y"),
        resolved(&["src/other2.rs"])
    );
    assert_eq!(
        pairs(&graph),
        [
            edge("src/lib.rs", "src/other.rs"),
            edge("src/lib.rs", "src/other2.rs")
        ]
    );
}

#[test]
fn comments_inside_include_and_use_tokens_are_skipped() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "mod a;\nconst D: &str = include_str!(/* c */ \"data.txt\");\nconst E: &str = r#include_str!(\"data.txt\");\nuse crate::a::/* c */ B;\n",
    );
    write(root, "src/a.rs", "pub struct B;\n");
    write(root, "src/data.txt", "");
    let graph = graph(root);
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/data.txt"), ["include"]);
    assert_eq!(
        graph
            .edges()
            .iter()
            .find(|e| e.to == "src/data.txt")
            .unwrap()
            .evidence_total,
        2
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "crate::a::B"),
        resolved(&["src/a.rs"])
    );
    assert!(reasons(&graph).is_empty(), "{:?}", reasons(&graph));
}

/// The Rust resolver, cancelling `cancel` before its second resolution and
/// recording each outcome by specifier.
struct Cancelling {
    cancel: CancellationToken,
    outcomes: Mutex<Vec<(String, Outcome)>>,
}

impl Resolver for Cancelling {
    fn family(&self) -> Family {
        Family::Rust
    }

    fn resolve(&self, file: &str, rel: &Relationship, cx: &mut ResolveCx<'_>) -> Outcome {
        if !self.outcomes.lock().unwrap().is_empty() {
            self.cancel.cancel();
        }
        let outcome = RustResolver.resolve(file, rel, cx);
        let entry = (rel.specifier.clone(), outcome.clone());
        self.outcomes.lock().unwrap().push(entry);
        outcome
    }
}

#[test]
fn a_cancelled_token_stops_the_module_expansion_from_read_files() {
    // No tree reaches `src/x.rs` or `src/y.rs`, so both are read before the
    // first relationship of `src/x.rs` resolves; the token is cancelled
    // before the second, whose `use` needs the module `y` expanded.
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(root, "src/lib.rs", "pub struct L;\n");
    write(root, "src/x.rs", "#[path = \"y.rs\"]\nmod y;\nuse y::Y;\n");
    write(root, "src/y.rs", "pub struct Y;\n");
    let cancel = CancellationToken::new();
    let resolver = Cancelling {
        cancel: cancel.clone(),
        outcomes: Mutex::new(Vec::new()),
    };
    let request = Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    let result = DepGraph::build_with(&mut reader, &ctx, &[&resolver], &cancel);
    assert!(matches!(
        result,
        Err(act_lite_cli::analyze::envelope::AnalyzeError::Cancelled)
    ));
    let outcomes = resolver.outcomes.lock().unwrap();
    assert_eq!(outcomes[0], ("y".to_string(), resolved(&["src/y.rs"])));
    assert_eq!(outcomes[1].0, "y::Y");
    assert!(
        !matches!(outcomes[1].1, Outcome::Resolved(_)),
        "the module y was expanded after the cancel: {:?}",
        outcomes[1].1
    );
}

#[test]
fn a_string_literal_is_resolved_by_its_decoded_value() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"lit\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        concat!(
            "#[path = \"\\x6f.rs\"]\nmod x;\n",
            "w! { #[path = \"\\u{70}.rs\"] mod y; }\n",
            "#[path = \"\\x69n\"]\nmod inl { mod k; }\n",
            "#[path = \"\\u{d800}.rs\"]\nmod bad;\n",
            "#[path = \"\\q\"]\nmod inl2 { mod m; }\n",
            "v! { #[path = \"\\q.rs\"] mod z; }\n",
            "const A: &str = include_str!(\"d\\u{61}ta.txt\");\n",
            "const B: &str = include_str!(\"da\\\n    ta.txt\");\n",
            "const C: &str = include_str!(r\"d\\u{61}ta.txt\");\n",
            "const D: &str = include_str!(\"d\\qta.txt\");\n",
        ),
    );
    write(root, "src/o.rs", "");
    write(root, "src/p.rs", "");
    write(root, "src/in/k.rs", "");
    write(root, "src/data.txt", "");
    let graph = graph(root);
    assert_eq!(outcome(&graph, "src/lib.rs", "x"), resolved(&["src/o.rs"]));
    assert_eq!(outcome(&graph, "src/lib.rs", "y"), resolved(&["src/p.rs"]));
    assert_eq!(
        outcome(&graph, "src/lib.rs", "k"),
        resolved(&["src/in/k.rs"])
    );
    for name in ["bad", "m", "z"] {
        assert_eq!(
            outcome(&graph, "src/lib.rs", name),
            Outcome::Unresolved("invalid string literal".to_string()),
            "{name}"
        );
    }
    assert_eq!(
        outcome(&graph, "src/lib.rs", "d\\u{61}ta.txt"),
        resolved(&["src/data.txt"])
    );
    assert_eq!(
        outcome(&graph, "src/lib.rs", "da\\\n    ta.txt"),
        resolved(&["src/data.txt"])
    );
    assert!(reasons(&graph).contains(&reason(
        "src/lib.rs",
        "d\\u{61}ta.txt",
        "src/d\\u{61}ta.txt does not exist"
    )));
    assert!(reasons(&graph).contains(&reason(
        "src/lib.rs",
        "d\\qta.txt",
        "invalid string literal"
    )));
}

#[test]
fn an_inline_module_with_a_rejected_path_has_no_directory() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "#[path = \"/outside\"]\n",
            "mod inline {\n",
            "    mod x;\n",
            "    #[path = \"z.rs\"]\n",
            "    mod z;\n",
            "    mod deeper {\n",
            "        mod w;\n",
            "    }\n",
            "}\n",
            "#[path = \"../../..\"]\n",
            "mod up {\n",
            "    mod y;\n",
            "}\n",
        ),
    );
    for decoy in [
        "src/outside/x.rs",
        "src/outside/z.rs",
        "src/outside/deeper/w.rs",
        "y.rs",
    ] {
        write(root, decoy, "pub struct T;\n");
    }
    let graph = graph(root);
    for name in ["x", "z", "w"] {
        assert_eq!(
            outcome(&graph, "src/lib.rs", name),
            unresolved("absolute path is not resolved: /outside"),
            "{name}"
        );
    }
    assert_eq!(
        outcome(&graph, "src/lib.rs", "y"),
        Outcome::External,
        "a path that leaves the workspace is outside, as for a direct module"
    );
    assert!(pairs(&graph).is_empty());
}

#[test]
fn a_use_in_a_block_local_module_resolves_inside_it() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "mod a;\nfn f() {\n    mod inner {\n        mod a {\n            pub struct A;\n        }\n        use self::a::A;\n    }\n}\n",
    );
    write(root, "src/a.rs", "use crate::f;\npub struct A;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "self::a::A"),
        resolved(&["src/lib.rs"]),
        "the block-local module `a`, in the file itself"
    );
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/a.rs"),
        ["module"],
        "no `use` from the block-local module to src/a.rs"
    );
    assert_eq!(kinds_of(&graph, "src/a.rs", "src/lib.rs"), ["use"]);
}

#[test]
fn a_use_in_a_body_finds_the_items_of_the_body_first() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "mod x;\n",
            "pub struct Top;\n",
            "pub fn f() {\n",
            "    mod x {\n",
            "        pub struct Inner;\n",
            "    }\n",
            "    struct Local;\n",
            "    use self::x::Outer;\n",
            "    use x::Inner;\n",
            "    use self::x::Inner as Wrong;\n",
            "    use self::Top;\n",
            "    use self::Local as Missing;\n",
            "}\n",
            "pub struct S;\n",
            "impl S {\n",
            "    fn m(&self) {\n",
            "        mod q {\n",
            "            pub struct Q;\n",
            "        }\n",
            "        use q::Q;\n",
            "    }\n",
            "}\n",
        ),
    );
    write(root, "src/x.rs", "pub struct Outer;\n");
    write(root, "src/q.rs", "pub struct Q;\n");
    let graph = graph(root);
    let of = |spec| outcome(&graph, "src/lib.rs", spec);
    assert_eq!(
        of("self::x::Outer"),
        resolved(&["src/x.rs"]),
        "`self` in a body is the module that holds the body"
    );
    assert_eq!(of("x::Inner"), resolved(&["src/lib.rs"]));
    assert_eq!(
        of("self::x::Inner"),
        unresolved("Inner is neither a module nor an item of src/x.rs")
    );
    assert_eq!(of("self::Top"), resolved(&["src/lib.rs"]));
    assert_eq!(
        of("self::Local"),
        unresolved("Local is neither a module nor an item of src/lib.rs"),
        "an item of the body is not an item of the module"
    );
    assert_eq!(of("q::Q"), resolved(&["src/lib.rs"]), "a method body");
    assert_eq!(pairs(&graph), [edge("src/lib.rs", "src/x.rs")]);
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/x.rs"),
        ["module", "use"]
    );
}

#[test]
fn super_and_nested_blocks_follow_the_block_local_scopes() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "mod a;\n",
            "mod deep;\n",
            "pub struct Top;\n",
            "pub fn f() {\n",
            "    pub struct Local;\n",
            "    mod deep {\n",
            "        pub struct D;\n",
            "    }\n",
            "    mod inner {\n",
            "        use super::Top;\n",
            "        use super::a::A;\n",
            "        use super::Local;\n",
            "        use deep::Hidden;\n",
            "        mod helper {\n",
            "            pub struct H;\n",
            "        }\n",
            "        pub fn g() {\n",
            "            struct G;\n",
            "            use self::helper::H;\n",
            "            mod nested {\n",
            "                use super::super::Top as Again;\n",
            "            }\n",
            "        }\n",
            "    }\n",
            "    {\n",
            "        {\n",
            "            use deep::D;\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    );
    write(root, "src/a.rs", "pub struct A;\n");
    write(root, "src/deep.rs", "pub struct D;\n");
    let graph = graph(root);
    let of = |spec| outcome(&graph, "src/lib.rs", spec);
    assert_eq!(
        of("super::Top"),
        resolved(&["src/lib.rs"]),
        "`super` of a block-local module is the module that holds the body"
    );
    assert_eq!(of("super::a::A"), resolved(&["src/a.rs"]));
    assert_eq!(
        of("super::Local"),
        unresolved("Local is neither a module nor an item of src/lib.rs")
    );
    assert_eq!(
        of("deep::Hidden"),
        unresolved("first path segment is not a module, crate or dependency: deep"),
        "a block-local module does not see the body around it"
    );
    assert_eq!(of("self::helper::H"), resolved(&["src/lib.rs"]));
    assert_eq!(
        of("super::super::Top"),
        resolved(&["src/lib.rs"]),
        "`super` goes out one block-local module, then to the module"
    );
    assert_eq!(
        of("deep::D"),
        resolved(&["src/lib.rs"]),
        "a nested block sees the items of the blocks around it"
    );
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/a.rs"),
        ["module", "use"]
    );
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/deep.rs"),
        ["module"],
        "no `use` reaches the module file `deep`"
    );
}

#[test]
fn a_use_in_a_body_follows_a_use_binding_of_the_body() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "mod a;\nmod b;\nfn f() {\n    use crate::b as a;\n    use a::Thing;\n}\n",
    );
    write(root, "src/a.rs", "use crate::f;\npub struct Thing;\n");
    write(root, "src/b.rs", "pub struct Thing;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "a::Thing"),
        resolved(&["src/b.rs"]),
        "`a` is the binding of `crate::b` in the body"
    );
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/a.rs"), ["module"]);
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/b.rs"),
        ["module", "use"]
    );
}

#[test]
fn use_bindings_of_a_body_reach_its_whole_block_nested_blocks_and_inline_modules() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "mod a;\n",
            "mod b;\n",
            "mod c;\n",
            "fn f() {\n",
            "    use a::Early;\n",
            "    use crate::b as a;\n",
            "    {\n",
            "        use a::Nested;\n",
            "    }\n",
            "    mod inner {\n",
            "        use crate::c as a;\n",
            "        use a::Inside;\n",
            "        fn g() {\n",
            "            use a::Deep;\n",
            "        }\n",
            "    }\n",
            "}\n",
            "fn h() {\n",
            "    use crate::c::*;\n",
            "    use sub::G;\n",
            "    use a::Thing;\n",
            "}\n",
            "fn k() {\n",
            "    use b;\n",
            "    use b::Again;\n",
            "}\n",
            "fn l() {\n",
            "    use p as q;\n",
            "    use q as p;\n",
            "}\n",
        ),
    );
    write(root, "src/a.rs", "pub struct Thing;\n");
    write(
        root,
        "src/b.rs",
        "pub struct Early;\npub struct Nested;\npub struct Again;\n",
    );
    write(
        root,
        "src/c.rs",
        "pub mod sub {\n    pub struct G;\n}\npub struct Inside;\npub struct Deep;\n",
    );
    let graph = graph(root);
    let of = |spec| outcome(&graph, "src/lib.rs", spec);
    assert_eq!(
        of("a::Early"),
        resolved(&["src/b.rs"]),
        "a binding is seen in its whole block, also before it"
    );
    assert_eq!(
        of("a::Nested"),
        resolved(&["src/b.rs"]),
        "a nested block sees the bindings of the blocks around it"
    );
    assert_eq!(
        of("a::Inside"),
        resolved(&["src/c.rs"]),
        "a binding of an inline module in a body"
    );
    assert_eq!(
        of("a::Deep"),
        resolved(&["src/c.rs"]),
        "a body inside the inline module sees its bindings"
    );
    assert_eq!(
        of("sub::G"),
        resolved(&["src/c.rs"]),
        "through the glob of the body"
    );
    assert_eq!(of("a::Thing"), resolved(&["src/a.rs"]));
    assert_eq!(
        of("b::Again"),
        resolved(&["src/b.rs"]),
        "`use b;` binds `b` to the module `b` around the body"
    );
    assert_eq!(
        of("q"),
        unresolved("first path segment is not a module, crate or dependency: q"),
        "a binding is not followed through itself"
    );
    assert_eq!(
        kinds_of(&graph, "src/lib.rs", "src/a.rs"),
        ["module", "use"]
    );
}

#[test]
fn a_dependency_is_named_by_its_lib_name_or_its_rename() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, "foo", "foo", "\n[lib]\nname = \"bar\"\n");
    write(root, "foo/src/lib.rs", "pub struct Thing;\n");
    package(
        root,
        "app",
        "app",
        "\n[dependencies]\nfoo = { path = \"../foo\" }\n",
    );
    write(
        root,
        "app/src/lib.rs",
        "use bar::Thing;\nuse foo::Thing as Other;\nextern crate bar;\nextern crate foo;\n",
    );
    package(
        root,
        "renamed",
        "renamed",
        "\n[dependencies]\nbaz = { package = \"foo\", path = \"../foo\" }\n",
    );
    write(
        root,
        "renamed/src/lib.rs",
        "use baz::Thing;\nuse bar::Thing as Other;\n",
    );
    package(
        root,
        "byname",
        "byname",
        "\n[dependencies]\nfoo = \"0.1\"\n",
    );
    write(
        root,
        "byname/src/lib.rs",
        "use bar::Thing;\nuse foo::Thing as Other;\n",
    );
    let graph = graph(root);
    let lib = resolved(&["foo/src/lib.rs"]);
    let missing = |name: &str| {
        unresolved(&format!(
            "first path segment is not a module, crate or dependency: {name}"
        ))
    };
    assert_eq!(outcome(&graph, "app/src/lib.rs", "bar::Thing"), lib);
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "foo::Thing"),
        missing("foo")
    );
    assert_eq!(outcome(&graph, "app/src/lib.rs", "bar"), lib);
    assert_eq!(
        outcome(&graph, "app/src/lib.rs", "foo"),
        unresolved("foo is not a crate or dependency")
    );
    assert_eq!(outcome(&graph, "renamed/src/lib.rs", "baz::Thing"), lib);
    assert_eq!(
        outcome(&graph, "renamed/src/lib.rs", "bar::Thing"),
        missing("bar")
    );
    assert_eq!(
        outcome(&graph, "byname/src/lib.rs", "bar::Thing"),
        lib,
        "a workspace crate named by its package name"
    );
    assert_eq!(
        outcome(&graph, "byname/src/lib.rs", "foo::Thing"),
        missing("foo")
    );
}

/// The `module` outcomes of the declarations of `name` in `file`, in
/// location order.
fn module_outcomes(graph: &DepGraph, file: &str, name: &str) -> Vec<Outcome> {
    graph
        .relationships_of(file)
        .iter()
        .filter(|r| r.relationship.specifier == name && r.relationship.kind.as_str() == "module")
        .map(|r| r.outcome.clone())
        .collect()
}

#[test]
fn a_file_module_in_a_body_is_not_a_child_of_the_module_around_the_body() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        "mod a;\nmod c;\nfn f() {\n    #[path = \"b.rs\"]\n    mod a;\n}\n",
    );
    write(root, "src/a.rs", "pub struct Thing;\n");
    write(
        root,
        "src/b.rs",
        "use crate::c::Other;\npub struct Thing;\n",
    );
    write(
        root,
        "src/c.rs",
        "use crate::a::Thing;\npub struct Other;\n",
    );
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/c.rs", "crate::a::Thing"),
        resolved(&["src/a.rs"]),
        "the `mod a;` of the body is not the child `a` of the crate root"
    );
    assert_eq!(
        module_outcomes(&graph, "src/lib.rs", "a"),
        [resolved(&["src/a.rs"]), resolved(&["src/b.rs"])],
        "the declaration in the body is a `module` relationship to its file"
    );
    assert_eq!(
        outcome(&graph, "src/b.rs", "crate::c::Other"),
        resolved(&["src/c.rs"]),
        "the file of the body module is in the tree"
    );
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/b.rs"), ["module"]);
    assert_eq!(kinds_of(&graph, "src/c.rs", "src/a.rs"), ["use"]);
}

#[test]
fn a_path_in_a_body_finds_a_file_module_of_the_body_first() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "mod a;\n",
            "mod c;\n",
            "pub struct Root;\n",
            "fn f() {\n",
            "    #[path = \"b.rs\"]\n",
            "    mod a;\n",
            "    use a::Thing;\n",
            "    {\n",
            "        use a::sub::Deep;\n",
            "    }\n",
            "}\n",
            "fn g() {\n",
            "    mod inner {\n",
            "        pub struct In;\n",
            "        #[path = \"e.rs\"]\n",
            "        pub mod e;\n",
            "        use self::e::Item;\n",
            "    }\n",
            "    use inner::e::Item as Again;\n",
            "}\n",
            "fn h() {\n",
            "    mod x;\n",
            "}\n",
        ),
    );
    write(root, "src/a.rs", "pub struct Thing;\n");
    write(
        root,
        "src/b.rs",
        concat!(
            "use crate::c::Other;\n",
            "use super::Root;\n",
            "use self::sub::Deep;\n",
            "pub struct Thing;\n",
            "pub mod sub;\n",
        ),
    );
    write(root, "src/sub.rs", "pub struct Deep;\n");
    write(root, "src/c.rs", "pub struct Other;\n");
    write(
        root,
        "src/inner/e.rs",
        "use super::In;\nuse super::super::Root;\npub struct Item;\n",
    );
    write(root, "src/x.rs", "use crate::Root;\npub struct X;\n");
    let graph = graph(root);
    let of = |file, spec| outcome(&graph, file, spec);
    assert_eq!(
        of("src/lib.rs", "a::Thing"),
        resolved(&["src/b.rs"]),
        "the body module `a` hides the child `a` of the crate root"
    );
    assert_eq!(
        of("src/lib.rs", "a::sub::Deep"),
        resolved(&["src/sub.rs"]),
        "a nested block walks on through the file of the body module"
    );
    assert_eq!(of("src/b.rs", "crate::c::Other"), resolved(&["src/c.rs"]));
    assert_eq!(
        of("src/b.rs", "super::Root"),
        resolved(&["src/lib.rs"]),
        "super is the module that holds the body"
    );
    assert_eq!(
        of("src/b.rs", "self::sub::Deep"),
        resolved(&["src/sub.rs"]),
        "a `#[path]` file owns its directory"
    );
    assert_eq!(
        module_outcomes(&graph, "src/lib.rs", "e"),
        [resolved(&["src/inner/e.rs"])],
        "an inline module of the body adds its directory"
    );
    assert_eq!(
        of("src/lib.rs", "self::e::Item"),
        resolved(&["src/inner/e.rs"])
    );
    assert_eq!(
        of("src/lib.rs", "inner::e::Item"),
        resolved(&["src/inner/e.rs"])
    );
    assert_eq!(
        of("src/inner/e.rs", "super::In"),
        resolved(&["src/lib.rs"]),
        "super is the inline module of the body"
    );
    assert_eq!(
        of("src/inner/e.rs", "super::super::Root"),
        resolved(&["src/lib.rs"])
    );
    assert_eq!(
        module_outcomes(&graph, "src/lib.rs", "x"),
        [unresolved(
            "a file module inside a block needs a #[path] attribute"
        )],
        "rustc loads no file for `mod x;` in a block without `#[path]`"
    );
    assert_eq!(
        of("src/x.rs", "crate::Root"),
        unresolved("file is not reached from a crate root"),
        "src/x.rs is not in the tree"
    );
}

#[test]
fn a_macro_in_a_body_declares_in_the_body_scope() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "mod a;\n",
            "mod c;\n",
            "macro_rules! items { ($($i:item)*) => { $($i)* } }\n",
            "fn f() {\n",
            "    items! {\n",
            "        #[path = \"b.rs\"]\n",
            "        pub mod a;\n",
            "        #[path = \"b.rs\"]\n",
            "        mod bm;\n",
            "        struct Hidden;\n",
            "    }\n",
            "    use bm::Thing;\n",
            "}\n",
            "fn g() {\n",
            "    mod inner {\n",
            "        items! {\n",
            "            #[path = \"e.rs\"]\n",
            "            pub(crate) mod e;\n",
            "            use crate::c as k;\n",
            "        }\n",
            "        use self::e::Item;\n",
            "        use k::Other;\n",
            "    }\n",
            "}\n",
            "fn h() {\n",
            "    items! { mod x; }\n",
            "}\n",
            "fn u() {\n",
            "    items! { use crate::c as ca; }\n",
            "    use ca::Other;\n",
            "}\n",
        ),
    );
    write(root, "src/a.rs", "pub struct Thing;\n");
    write(
        root,
        "src/b.rs",
        "use crate::c::Other;\nuse super::f;\npub struct Thing;\n",
    );
    write(
        root,
        "src/c.rs",
        "use crate::a::Thing;\nuse crate::Hidden;\npub struct Other;\n",
    );
    write(root, "src/inner/e.rs", "pub struct Item;\n");
    write(root, "src/x.rs", "use crate::f;\npub struct X;\n");
    let graph = graph(root);
    let of = |file, spec| outcome(&graph, file, spec);
    assert_eq!(
        of("src/c.rs", "crate::a::Thing"),
        resolved(&["src/a.rs"]),
        "a `mod a;` of a macro in a body is not the child `a` of the crate root"
    );
    assert_eq!(
        of("src/c.rs", "crate::Hidden"),
        unresolved("Hidden is neither a module nor an item of src/lib.rs"),
        "an item of a macro in a body is not an item of the module"
    );
    assert_eq!(
        module_outcomes(&graph, "src/lib.rs", "a"),
        [resolved(&["src/a.rs"]), resolved(&["src/b.rs"])]
    );
    assert_eq!(
        of("src/lib.rs", "bm::Thing"),
        resolved(&["src/b.rs"]),
        "a path in the body finds the macro's `mod` first"
    );
    assert_eq!(of("src/b.rs", "crate::c::Other"), resolved(&["src/c.rs"]));
    assert_eq!(of("src/b.rs", "super::f"), resolved(&["src/lib.rs"]));
    assert_eq!(
        module_outcomes(&graph, "src/lib.rs", "e"),
        [resolved(&["src/inner/e.rs"])],
        "a macro in an inline module of a body"
    );
    assert_eq!(
        of("src/lib.rs", "self::e::Item"),
        resolved(&["src/inner/e.rs"])
    );
    assert_eq!(
        of("src/lib.rs", "k::Other"),
        resolved(&["src/c.rs"]),
        "a `use` of a macro binds in the inline module of the body"
    );
    assert_eq!(
        of("src/lib.rs", "ca::Other"),
        resolved(&["src/c.rs"]),
        "a `use` of a macro binds in the body"
    );
    assert_eq!(
        module_outcomes(&graph, "src/lib.rs", "x"),
        [unresolved(
            "a file module inside a block needs a #[path] attribute"
        )]
    );
    assert_eq!(
        of("src/x.rs", "crate::f"),
        unresolved("file is not reached from a crate root"),
        "src/x.rs is not in the tree"
    );
}

#[test]
fn a_leading_path_separator_before_a_use_list_names_a_crate() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "edition = \"2021\"\n");
    write(
        root,
        "src/lib.rs",
        "pub struct Root;\nmod std;\nuse ::{std::fmt};\nw! { use ::{std::fmt as f}; }\n",
    );
    write(root, "src/std.rs", "pub mod fmt {}\nuse crate::Root;\n");
    let graph = graph(root);
    let outcomes: Vec<Outcome> = graph
        .relationships_of("src/lib.rs")
        .iter()
        .filter(|r| r.relationship.specifier == "::std::fmt")
        .map(|r| r.outcome.clone())
        .collect();
    assert_eq!(outcomes, [Outcome::External, Outcome::External]);
    assert_eq!(kinds_of(&graph, "src/lib.rs", "src/std.rs"), ["module"]);
    assert_eq!(kinds_of(&graph, "src/std.rs", "src/lib.rs"), ["use"]);
}

/// A body of `src/lib.rs` with `globs` copies of `use std::collections::*;`
/// and a `use` whose first segment only a glob names.
fn many_globs(root: &Path, globs: usize) {
    package(root, ".", "app", "");
    let mut body = String::from("fn f() {\n");
    for _ in 0..globs {
        body.push_str("    use std::collections::*;\n");
    }
    body.push_str("    use hash_map::Entry;\n}\n");
    write(root, "src/lib.rs", &body);
}

#[test]
fn many_globs_of_a_body_are_each_tried_once() {
    for globs in [12, 16] {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path().to_path_buf();
        many_globs(&root, globs);
        let (send, receive) = std::sync::mpsc::channel();
        let worker = root.clone();
        std::thread::spawn(move || {
            let graph = graph(&worker);
            let _ = send.send((
                outcome(&graph, "src/lib.rs", "hash_map::Entry"),
                outcome(&graph, "src/lib.rs", "std::collections"),
            ));
        });
        let (entry, glob) = receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{globs} globs: no result within 5 s"));
        assert_eq!(entry, Outcome::External, "{globs} globs: through a glob");
        assert_eq!(glob, Outcome::External, "{globs} globs: the glob itself");
    }
}

#[test]
fn a_cancelled_token_stops_the_lookup_through_globs() {
    // The token is cancelled before the second relationship, the `use`
    // that only a glob of the body resolves.
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    let mut body = String::from("mod y;\nfn f() {\n    use hash_map::Entry;\n");
    for _ in 0..16 {
        body.push_str("    use std::collections::*;\n");
    }
    body.push_str("}\n");
    write(root, "src/lib.rs", &body);
    write(root, "src/y.rs", "pub struct Y;\n");
    let cancel = CancellationToken::new();
    let resolver = Cancelling {
        cancel: cancel.clone(),
        outcomes: Mutex::new(Vec::new()),
    };
    let request = Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    let result = DepGraph::build_with(&mut reader, &ctx, &[&resolver], &cancel);
    assert!(matches!(
        result,
        Err(act_lite_cli::analyze::envelope::AnalyzeError::Cancelled)
    ));
    let outcomes = resolver.outcomes.lock().unwrap();
    assert_eq!(outcomes[0].0, "y");
    assert_eq!(outcomes[1].0, "hash_map::Entry");
    assert!(
        !matches!(outcomes[1].1, Outcome::External),
        "a glob was tried after the cancel: {:?}",
        outcomes[1].1
    );
}

#[test]
fn a_chain_of_bindings_with_two_declarations_each_stops_at_the_lookup_limit() {
    // Each `yN` has two declarations, so the paths through the chain
    // double at each step: 2^25 lookups in full. The lookup stops at the
    // limit with the targets found so far. Each `use` of the chain is a
    // relationship too; those up to `y12` pass the limit as well.
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().to_path_buf();
    package(&root, ".", "app", "");
    let depth = 24;
    let mut body = String::from("mod m;\nfn f() {\n    use y0::Thing;\n");
    for i in 0..depth {
        let next = i + 1;
        body.push_str(&format!(
            "    #[cfg(feature = \"a\")]\n    use y{next}::a as y{i};\n"
        ));
        body.push_str(&format!(
            "    #[cfg(not(feature = \"a\"))]\n    use y{next}::b as y{i};\n"
        ));
    }
    body.push_str(&format!("    use crate::m as y{depth};\n}}\n"));
    write(&root, "src/lib.rs", &body);
    write(&root, "src/m.rs", "pub use std::collections::*;\n");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let graph = graph(&root);
        let _ = send.send((
            outcome(&graph, "src/lib.rs", "y0::Thing"),
            graph.limits_hit().to_vec(),
        ));
    });
    let started = std::time::Instant::now();
    let (thing, limits) = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("no result within 5 s");
    eprintln!("the chain took {:?}", started.elapsed());
    assert_eq!(thing, resolved(&["src/m.rs"]));
    let mut uses = vec!["y0::Thing".to_string()];
    for i in 1..=12 {
        uses.push(format!("y{i}::a"));
        uses.push(format!("y{i}::b"));
    }
    let expected: Vec<Value> = uses
        .iter()
        .map(|spec| {
            json!({
                "limit": "rust_use_lookups",
                "value": 10000,
                "detail": format!("the use {spec} in src/lib.rs reached 10000 lookups through block-local bindings and globs; its other lookups were not followed")
            })
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&limits).unwrap(),
        Value::from(expected)
    );
}

#[test]
fn a_binding_followed_with_different_suffixes_gives_each_target() {
    // `y0` has two declarations; both go on through the one binding `y1`,
    // with different suffixes, so each reaches its own `sub` module.
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    package(root, ".", "app", "");
    write(
        root,
        "src/lib.rs",
        concat!(
            "mod m;\n",
            "fn f() {\n",
            "    #[cfg(feature = \"a\")]\n",
            "    use y1::a as y0;\n",
            "    #[cfg(not(feature = \"a\"))]\n",
            "    use y1::b as y0;\n",
            "    use crate::m as y1;\n",
            "    use y0::sub::Thing;\n",
            "}\n",
        ),
    );
    write(root, "src/m.rs", "pub mod a;\npub mod b;\n");
    write(root, "src/m/a.rs", "pub mod sub;\n");
    write(root, "src/m/b.rs", "pub mod sub;\n");
    write(root, "src/m/a/sub.rs", "pub struct Thing;\n");
    write(root, "src/m/b/sub.rs", "pub struct Thing;\n");
    let graph = graph(root);
    assert_eq!(
        outcome(&graph, "src/lib.rs", "y0::sub::Thing"),
        resolved(&["src/m/a/sub.rs", "src/m/b/sub.rs"])
    );
    assert_eq!(graph.limits_hit(), []);
}
