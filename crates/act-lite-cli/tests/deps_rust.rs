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
fn only_the_module_files_on_the_path_are_read() {
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
    let watching = Watching {
        watched: vec!["src/a.rs", "src/b.rs", "tests/t.rs", "tests/helper.rs"],
        read: Mutex::new(BTreeSet::new()),
    };
    build_with(root, Includes::default(), &watching);
    let read: Vec<String> = watching.read.into_inner().unwrap().into_iter().collect();
    assert_eq!(read, ["src/a.rs"]);
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
        "use shared::Item;\nuse foo_bar::Z;\n",
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
        outcome(&graph, "app/src/lib.rs", "shared::Item"),
        resolved(&["one/src/lib.rs", "two/src/lib.rs"])
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
