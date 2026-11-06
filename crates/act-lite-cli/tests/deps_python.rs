//! Python resolution: the roots, packages before modules, namespace
//! packages, names bound in a module, relative imports, the external test
//! and the workspace modules outside the roots.

use act_lite_cli::analyze::deps::python::{PythonResolver, STDLIB_MODULES};
use act_lite_cli::analyze::deps::{DepGraph, Outcome, supports};
use act_lite_cli::analyze::discovery::{Includes, discover};
use act_lite_cli::analyze::read::{Reader, SupportReader};
use act_lite_cli::analyze::run::{Request, RunContext};
use serde_json::json;
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Builds the graph of `root` with the Python resolver, `paths` as the scope.
fn build(root: &Path, paths: &[&str]) -> DepGraph {
    let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
    let cancel = CancellationToken::new();
    let request = Request::new(root, &paths, &[], &[], Includes::default(), 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &Includes::default(), &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    DepGraph::build_with(&mut reader, &ctx, &[&PythonResolver], &cancel).unwrap()
}

fn edges(graph: &DepGraph) -> Vec<(String, String)> {
    graph
        .edges()
        .iter()
        .map(|e| (e.file.clone(), e.to.clone()))
        .collect()
}

/// The outcome of the relationship of `file` with `specifier`.
fn outcome(graph: &DepGraph, file: &str, specifier: &str) -> Outcome {
    let found: Vec<&Outcome> = graph
        .relationships_of(file)
        .iter()
        .filter(|r| r.relationship.specifier == specifier)
        .map(|r| &r.outcome)
        .collect();
    assert_eq!(found.len(), 1, "{file}: {specifier}");
    found[0].clone()
}

fn resolved(targets: &[&str]) -> Outcome {
    Outcome::Resolved(targets.iter().map(|t| t.to_string()).collect())
}

fn unresolved(reason: &str) -> Outcome {
    Outcome::Unresolved(reason.to_string())
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn fixture_py(root: &Path) {
    write(
        root,
        "pyproject.toml",
        "[project]\nname = \"py-fixture\"\ndependencies = [\"requests>=2\"]\n",
    );
    write(root, "app/__init__.py", "");
    write(
        root,
        "app/main.py",
        "import os\nfrom app import util\nfrom .models import User\nfrom . import helpers\nimport app.missing\nimport requests\n",
    );
    write(root, "app/util.py", "x = 1\n");
    write(root, "app/models/__init__.py", "class User: pass\n");
    write(root, "app/helpers.py", "from .util import x\n");
    write(root, "app/uses_lib.py", "import lib\n");
    write(root, "scripts/run.py", "import tool\n");
    write(root, "scripts/tool.py", "");
    write(root, "src/lib/__init__.py", "");
}

#[test]
fn the_py_fixture() {
    let ws = tempfile::tempdir().unwrap();
    fixture_py(ws.path());
    let graph = build(ws.path(), &[]);
    assert_eq!(
        edges(&graph),
        pairs(&[
            ("app/helpers.py", "app/util.py"),
            ("app/main.py", "app/helpers.py"),
            ("app/main.py", "app/models/__init__.py"),
            ("app/main.py", "app/util.py"),
            ("app/uses_lib.py", "src/lib/__init__.py"),
            ("scripts/run.py", "scripts/tool.py"),
        ])
    );
    assert_eq!(outcome(&graph, "app/main.py", "os"), Outcome::External);
    assert_eq!(
        outcome(&graph, "app/main.py", "requests"),
        Outcome::External
    );
    let entries = graph.unresolved(&|_| true);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file, "app/main.py");
    assert_eq!(entries[0].target, "app.missing");
    assert_eq!(entries[0].reason, "app.missing not found under .");
    let details = graph.common_details();
    assert_eq!(
        details["relationships"],
        json!({"total": 9, "resolved": 6, "external": 2, "unresolved": 1})
    );
    assert_eq!(
        details["resolution_inputs"],
        json!([{"file": "pyproject.toml", "kind": "pyproject.toml"}])
    );
}

#[test]
fn the_language_specific_forms() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(root, "pkg/__init__.py", "");
    write(root, "pkg/a.py", "from .. import x\n");
    write(root, "top.py", "from . import y\n");
    write(root, "pkg/b.py", "import os, pkg.a\n");
    write(root, "pkg/c.py", "from pkg import *\n");
    write(root, "ns/mod.py", "");
    write(root, "pkg/d.py", "import ns.mod\n");
    write(root, "pkg/e.py", "from pkg import gone\n");
    write(root, "pkg/f.py", "import yaml\n");
    write(root, "lib/data.txt", "data\n");
    write(root, "src/lib/__init__.py", "");
    write(root, "src/lib/x.py", "");
    write(root, "pkg/g.py", "import lib.x\n");
    let graph = build(root, &[]);
    assert_eq!(
        outcome(&graph, "pkg/a.py", "..x"),
        unresolved("relative import beyond the top-level package")
    );
    assert_eq!(
        outcome(&graph, "top.py", ".y"),
        unresolved("relative import outside a package")
    );
    let b = graph.relationships_of("pkg/b.py");
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].relationship.range, b[1].relationship.range);
    assert_eq!(outcome(&graph, "pkg/b.py", "os"), Outcome::External);
    assert_eq!(
        outcome(&graph, "pkg/b.py", "pkg.a"),
        resolved(&["pkg/a.py"])
    );
    assert_eq!(
        outcome(&graph, "pkg/c.py", "pkg.*"),
        resolved(&["pkg/__init__.py"])
    );
    assert_eq!(
        outcome(&graph, "pkg/d.py", "ns.mod"),
        resolved(&["ns/mod.py"])
    );
    assert_eq!(
        outcome(&graph, "pkg/e.py", "pkg.gone"),
        unresolved("gone is neither bound in pkg/__init__.py nor a submodule of pkg")
    );
    assert_eq!(
        outcome(&graph, "pkg/f.py", "yaml"),
        unresolved(
            "yaml is not a standard-library module, a declared dependency or a module under the roots"
        )
    );
    assert_eq!(
        outcome(&graph, "pkg/g.py", "lib.x"),
        resolved(&["src/lib/x.py"])
    );
}

#[test]
fn a_wildcard_import_adds_the_package_file() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "pkg/__init__.py", "from .base import *\n");
    write(ws.path(), "pkg/base.py", "");
    write(ws.path(), "pkg/sub.py", "");
    write(ws.path(), "app.py", "from pkg import sub\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(
        outcome(&graph, "app.py", "pkg.sub"),
        resolved(&["pkg/sub.py", "pkg/__init__.py"])
    );
}

#[test]
fn a_loop_target_or_a_with_alias_in_the_package_file_beats_a_submodule() {
    for init in ["for n in [1]: pass\n", "with c() as n: pass\n"] {
        let ws = tempfile::tempdir().unwrap();
        write(ws.path(), "pkg/__init__.py", init);
        write(ws.path(), "pkg/n.py", "");
        write(ws.path(), "a.py", "from pkg import n\n");
        let graph = build(ws.path(), &[]);
        assert_eq!(
            outcome(&graph, "a.py", "pkg.n"),
            resolved(&["pkg/__init__.py"]),
            "{init}"
        );
    }
}

#[test]
fn an_annotation_without_a_value_does_not_beat_a_submodule() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "pkg/__init__.py", "n: int\n");
    write(ws.path(), "pkg/n.py", "");
    write(ws.path(), "a.py", "from pkg import n\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "a.py", "pkg.n"), resolved(&["pkg/n.py"]));
}

#[test]
fn a_global_statement_does_not_beat_a_submodule() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "pkg/__init__.py", "global n\n");
    write(ws.path(), "pkg/n.py", "");
    write(ws.path(), "a.py", "from pkg import n\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "a.py", "pkg.n"), resolved(&["pkg/n.py"]));
}

#[test]
fn a_workspace_module_outside_the_roots_is_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "pyproject.toml",
        "[project]\nname = \"mono\"\ndependencies = [\"core\"]\n",
    );
    write(ws.path(), "libs/core/core/__init__.py", "");
    write(ws.path(), "libs/core/core/util.py", "");
    write(
        ws.path(),
        "apps/web/web/views.py",
        "from core import util\n",
    );
    let graph = build(ws.path(), &[]);
    assert_eq!(
        outcome(&graph, "apps/web/web/views.py", "core.util"),
        unresolved("core is a module of the workspace outside the roots: libs/core/core/")
    );
}

#[test]
fn a_subpackage_is_no_top_level_candidate() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/flask/__init__.py", "");
    write(ws.path(), "src/flask/json/__init__.py", "");
    write(ws.path(), "src/flask/app.py", "import json\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(
        outcome(&graph, "src/flask/app.py", "json"),
        Outcome::External
    );
}

#[test]
fn a_package_beats_a_module_of_the_same_name() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "a/__init__.py", "");
    write(ws.path(), "a.py", "");
    write(ws.path(), "b.py", "import a\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "b.py", "a"), resolved(&["a/__init__.py"]));
}

#[test]
fn a_dependency_file_that_does_not_parse_makes_externals_unresolved() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "pyproject.toml",
        "[project\ndependencies = [\"requests\"]\n",
    );
    write(
        ws.path(),
        "main.py",
        "import os\nimport requests\nimport m\n",
    );
    write(ws.path(), "m.py", "");
    let graph = build(ws.path(), &[]);
    let reason = unresolved("pyproject.toml could not be read");
    assert_eq!(outcome(&graph, "main.py", "requests"), reason);
    assert_eq!(outcome(&graph, "main.py", "os"), reason);
    assert_eq!(outcome(&graph, "main.py", "m"), resolved(&["m.py"]));
    let inputs = &graph.common_details()["resolution_inputs"];
    assert_eq!(inputs[0]["file"], "pyproject.toml");
    assert!(inputs[0]["error"].is_string());
}

#[test]
fn a_module_outside_paths_is_read_for_its_names_only() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "lib/__init__.py", "");
    write(ws.path(), "lib/conf.py", "def setting(): pass\n");
    write(ws.path(), "app/main.py", "from lib.conf import setting\n");
    let graph = build(ws.path(), &["app"]);
    assert_eq!(
        outcome(&graph, "app/main.py", "lib.conf.setting"),
        resolved(&["lib/conf.py"])
    );
    assert!(!graph.analyzed().contains("lib/conf.py"));
    assert!(graph.relationships_of("lib/conf.py").is_empty());
    assert!(!graph.edges()[0].to_analyzed);
}

#[test]
fn declared_distributions_from_each_dependency_file() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "pyproject.toml",
        concat!(
            "[project]\nname = \"p\"\n",
            "[project.optional-dependencies]\ndocs = [\"Sphinx>=7\"]\n",
            "[dependency-groups]\ndev = [\"pytest\", {include-group = \"docs\"}]\n",
            "[tool.poetry.dependencies]\npython = \"^3.11\"\nclick = \"^8\"\n",
            "[tool.poetry.group.lint.dependencies]\nruff = \"*\"\n",
        ),
    );
    write(
        ws.path(),
        "svc/requirements-dev.txt",
        "# tools\n-r base.txt\nMy.Package[extra]>=1 ; python_version > '3'\n",
    );
    write(
        ws.path(),
        "svc/setup.cfg",
        "[metadata]\nname = svc\n\n[options]\ninstall_requires =\n    attrs>=22\n    typing-extensions\n",
    );
    write(
        ws.path(),
        "svc/main.py",
        "import sphinx\nimport pytest\nimport click\nimport ruff\nimport my_package\nimport attrs\nimport typing_extensions\nimport python\n",
    );
    let graph = build(ws.path(), &[]);
    for name in [
        "sphinx",
        "pytest",
        "click",
        "ruff",
        "my_package",
        "attrs",
        "typing_extensions",
    ] {
        assert_eq!(
            outcome(&graph, "svc/main.py", name),
            Outcome::External,
            "{name}"
        );
    }
    assert_eq!(
        outcome(&graph, "svc/main.py", "python"),
        unresolved(
            "python is not a standard-library module, a declared dependency or a module under the roots"
        )
    );
    assert_eq!(
        graph.common_details()["resolution_inputs"],
        json!([
            {"file": "pyproject.toml", "kind": "pyproject.toml"},
            {"file": "svc/requirements-dev.txt", "kind": "requirements"},
            {"file": "svc/setup.cfg", "kind": "setup.cfg"}
        ])
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_workspace_directory_leaves_externals_unresolved() {
    use std::os::unix::fs::PermissionsExt;
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "locked/os.py", "");
    write(ws.path(), "app/main.py", "import os\n");
    let locked = ws.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let graph = build(ws.path(), &["app"]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        outcome(&graph, "app/main.py", "os"),
        unresolved("locked/ could not be read")
    );
}

#[test]
fn the_standard_library_list() {
    assert_eq!(STDLIB_MODULES.len(), 290);
    assert!(STDLIB_MODULES.windows(2).all(|w| w[0] < w[1]));
    for name in ["os", "json", "__future__", "asyncio", "zoneinfo"] {
        assert!(STDLIB_MODULES.contains(&name), "{name}");
    }
    for name in ["distutils", "imp", "requests"] {
        assert!(!STDLIB_MODULES.contains(&name), "{name}");
    }
}

#[test]
fn a_broken_dependency_file_is_read_for_a_standard_library_name() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "pyproject.toml", "[project\n");
    write(ws.path(), "main.py", "import os\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(
        outcome(&graph, "main.py", "os"),
        unresolved("pyproject.toml could not be read")
    );
    let inputs = &graph.common_details()["resolution_inputs"];
    assert_eq!(inputs[0]["file"], "pyproject.toml");
    assert!(inputs[0]["error"].is_string());
}

fn not_declared(name: &str) -> Outcome {
    unresolved(&format!(
        "{name} is not a standard-library module, a declared dependency or a module under the roots"
    ))
}

#[test]
fn a_setup_cfg_url_line_declares_no_name() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "setup.cfg",
        "[options]\ninstall_requires =\n    https://example.com/pkg.whl\n    file: reqs.txt\n",
    );
    write(ws.path(), "main.py", "import https\nimport file\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "main.py", "https"), not_declared("https"));
    assert_eq!(outcome(&graph, "main.py", "file"), not_declared("file"));
}

#[test]
fn a_blank_line_does_not_end_install_requires() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "setup.cfg",
        "[options]\ninstall_requires =\n    alpha\n\n    beta\n",
    );
    write(ws.path(), "main.py", "import beta\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "main.py", "beta"), Outcome::External);
}

#[test]
fn a_name_with_a_url_declares_the_name() {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "requirements.txt",
        "alpha@https://x.org/alpha.whl\n",
    );
    write(ws.path(), "main.py", "import alpha\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "main.py", "alpha"), Outcome::External);
}

#[test]
fn a_byte_order_mark_does_not_hide_the_first_name() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "requirements.txt", "\u{feff}alpha\n");
    write(ws.path(), "main.py", "import alpha\n");
    let graph = build(ws.path(), &[]);
    assert_eq!(outcome(&graph, "main.py", "alpha"), Outcome::External);
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_workspace_is_external() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    write(tmp.path(), "outside/ext/__init__.py", "");
    write(tmp.path(), "outside/far.py", "");
    write(&root, "pkg/__init__.py", "");
    write(&root, "main.py", "import ext\nimport pkg.far\n");
    std::os::unix::fs::symlink(tmp.path().join("outside/ext"), root.join("ext")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("outside/far.py"), root.join("pkg/far.py")).unwrap();
    let graph = build(&root, &[]);
    assert_eq!(outcome(&graph, "main.py", "ext"), Outcome::External);
    assert_eq!(outcome(&graph, "main.py", "pkg.far"), Outcome::External);
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_inside_a_root_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "app/__init__.py", "");
    write(ws.path(), "app/sub/x.py", "");
    write(
        ws.path(),
        "app/main.py",
        "import app.sub.x\nimport app.sub\n",
    );
    write(ws.path(), "app/other.py", "from app import sub\n");
    let sub = ws.path().join("app/sub");
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();
    let graph = build(ws.path(), &["app/main.py", "app/other.py"]);
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).unwrap();
    let reason = unresolved("app/sub/ could not be read");
    assert_eq!(outcome(&graph, "app/main.py", "app.sub.x"), reason);
    assert_eq!(outcome(&graph, "app/main.py", "app.sub"), reason);
    assert_eq!(outcome(&graph, "app/other.py", "app.sub"), reason);
}

/// As `build`, with vendored files included.
fn build_with_vendored(root: &Path) -> DepGraph {
    let cancel = CancellationToken::new();
    let includes = Includes {
        vendored: true,
        ..Includes::default()
    };
    let request = Request::new(root, &[], &[], &[], includes, 100).unwrap();
    let ctx = RunContext {
        root: root.to_path_buf(),
        discovered: discover(root, &request.scope, &supports, &cancel).unwrap(),
        support: SupportReader::new(root, &includes, &cancel),
    };
    let mut reader = Reader::new(&ctx.discovered.files, &supports, false, &cancel);
    DepGraph::build_with(&mut reader, &ctx, &[&PythonResolver], &cancel).unwrap()
}

/// A declared `requests` whose only workspace copy is inside the virtual
/// environment `venv`, with vendored files included.
fn requests_in_a_virtual_environment(venv: &str) -> Outcome {
    let ws = tempfile::tempdir().unwrap();
    write(
        ws.path(),
        "pyproject.toml",
        "[project]\nname = \"p\"\ndependencies = [\"requests\"]\n",
    );
    write(
        ws.path(),
        &format!("{venv}/pyvenv.cfg"),
        "home = /usr/bin\n",
    );
    write(
        ws.path(),
        &format!("{venv}/lib/python3.13/site-packages/requests/__init__.py"),
        "",
    );
    write(ws.path(), "main.py", "import requests\n");
    outcome(&build_with_vendored(ws.path()), "main.py", "requests")
}

#[test]
fn a_module_in_a_virtual_environment_is_no_top_level_candidate() {
    assert_eq!(
        requests_in_a_virtual_environment(".venv"),
        Outcome::External
    );
}

#[test]
fn a_pyvenv_cfg_marks_a_directory_of_any_name_as_vendored() {
    assert_eq!(requests_in_a_virtual_environment("env"), Outcome::External);
}
