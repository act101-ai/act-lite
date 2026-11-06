//! End-to-end tests of `act-lite analyze graph|cycles|coupling|impact`, run
//! against the built binary with `--workspace` and an empty configuration
//! directory. Every workspace is synthetic and built in a temporary
//! directory; expected edges, counts and texts come from the fixture sources
//! and the documented rules, not from the binary's output.
#![cfg(unix)]

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};
use tempfile::TempDir;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A temporary workspace with an empty `.git/`.
fn workspace() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

// ---- running the binary ----

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn base(ws: &Path, config: &Path, name: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_act-lite"));
    cmd.env("ACT_CONFIG_DIR", config)
        .env_remove("ACT_LITE_DISABLE_LSP")
        .arg("--workspace")
        .arg(ws)
        .args(["analyze", name]);
    cmd
}

/// `act-lite --workspace <ws> analyze <name> <args>`.
fn exec(ws: &Path, name: &str, args: &[&str]) -> Run {
    let config = TempDir::new().unwrap();
    let out = base(ws, config.path(), name).args(args).output().unwrap();
    Run {
        code: out.status.code(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// The parsed stdout of a run that must exit 0.
fn analyze(ws: &Path, name: &str, args: &[&str]) -> Value {
    let run = exec(ws, name, args);
    assert_eq!(run.code, Some(0), "{name} {args:?}: {}", run.stderr);
    serde_json::from_str(&run.stdout).expect("stdout is one JSON value")
}

fn graph(ws: &Path, args: &[&str]) -> Value {
    analyze(ws, "graph", args)
}

// ---- reading results ----

/// The edges of a `graph` result as (file, to), in result order.
fn edges(v: &Value) -> Vec<(String, String)> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["file"].as_str().unwrap().to_string(),
                f["to"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

/// The edge from `file` to `to`.
fn edge<'a>(v: &'a Value, file: &str, to: &str) -> &'a Value {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file"] == file && f["to"] == to)
        .unwrap_or_else(|| panic!("no edge {file} -> {to} in {}", edges_text(v)))
}

fn edges_text(v: &Value) -> String {
    format!("{:?}", edges(v))
}

fn kinds(edge: &Value) -> Vec<&str> {
    edge["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect()
}

/// The unresolved entries as (file, target, reason), in result order.
fn unresolved(v: &Value) -> Vec<(String, String, String)> {
    v["incomplete"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["file"].as_str().unwrap().to_string(),
                u["target"].as_str().unwrap().to_string(),
                u["reason"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn triples(list: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    list.iter()
        .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
        .collect()
}

/// The unresolved entry of `file` with the given `reason`.
fn reason_of(v: &Value, file: &str) -> Vec<String> {
    unresolved(v)
        .into_iter()
        .filter(|(f, _, _)| f == file)
        .map(|(_, _, reason)| reason)
        .collect()
}

fn relationships(v: &Value) -> Value {
    v["details"]["relationships"].clone()
}

fn count_of(total: u64, resolved: u64, external: u64, unresolved: u64) -> Value {
    json!({"total": total, "resolved": resolved, "external": external, "unresolved": unresolved})
}

// ---- fixtures ----

const TS_PACKAGE: &str =
    r#"{"name":"ts-fixture","private":true,"dependencies":{"react":"18.3.1"}}"#;

/// The `ts` fixture: seven analyzed files, a CSS file and a test file.
fn ts(root: &Path) {
    write(root, "package.json", TS_PACKAGE);
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

fn ts_workspace() -> TempDir {
    let ws = workspace();
    ts(ws.path());
    ws
}

/// The `acyclic` fixture: a diamond of imports and one external.
fn acyclic(root: &Path) {
    write(
        root,
        "src/a.ts",
        "import { b } from './b';\nimport fs from 'node:fs';\n",
    );
    write(root, "src/b.ts", "import { c } from './c';\n");
    write(root, "src/c.ts", "export const c = 1;\n");
    write(root, "src/d.ts", "import './b';\nimport './c';\n");
}

/// The `unres` fixture: `src/u.ts` imports a file that does not exist.
fn unres(root: &Path, with_importer: bool) {
    write(root, "src/t.ts", "export const t = 1;\n");
    if with_importer {
        write(root, "src/u.ts", "import { t } from './t-old';\n");
    }
}

/// The `py` fixture.
fn py(root: &Path) {
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

/// The `go` fixture; `libs_mod` tells whether `libs/db/go.mod` exists.
fn go(root: &Path, libs_mod: bool) {
    write(root, "go.mod", "module example.com/shop\n");
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
    if libs_mod {
        write(root, "libs/db/go.mod", "module example.com/shop/store/db\n");
    }
    write(root, "libs/db/db.go", "package db\n");
    write(root, "tools/go.mod", "module example.com/tools\n");
    write(
        root,
        "tools/gen.go",
        "package tools\n\nimport \"example.com/shop/store\"\n",
    );
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

// ---- model and extraction ----

#[test]
fn the_ts_fixture_gives_eight_edges_and_the_documented_details() {
    let ws = ts_workspace();
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert_eq!(v["status"], "findings");
    assert_eq!(v["complete"], false);
    assert_eq!(v["total"], 8);
    // (file, to, to_analyzed, kinds, evidence line, specifier)
    let table: [(&str, &str, bool, &str, u64, &str); 8] = [
        ("src/a.ts", "src/main.ts", true, "import", 1, "./main"),
        ("src/main.ts", "src/a.ts", true, "import", 1, "./a"),
        ("src/main.ts", "src/c.ts", true, "require", 5, "./c.js"),
        ("src/main.ts", "src/d/index.ts", true, "reexport", 6, "./d"),
        ("src/main.ts", "src/lib/b.ts", true, "import", 2, "@lib/b"),
        (
            "src/main.ts",
            "src/types.d.ts",
            true,
            "type_import",
            7,
            "./types",
        ),
        ("src/view.tsx", "src/a.ts", true, "import", 2, "./a.js"),
        (
            "src/view.tsx",
            "src/style.css",
            false,
            "import",
            1,
            "./style.css",
        ),
    ];
    let found = v["findings"].as_array().unwrap();
    assert_eq!(found.len(), 8);
    for (f, (file, to, analyzed, kind, line, specifier)) in found.iter().zip(table) {
        assert_eq!(
            (f["file"].as_str().unwrap(), f["to"].as_str().unwrap()),
            (file, to)
        );
        assert_eq!(f["to_analyzed"], analyzed, "{file} -> {to}");
        assert_eq!(f["kinds"], json!([kind]), "{file} -> {to}");
        assert_eq!(f["depth"], Value::Null, "{file} -> {to}");
        assert_eq!(f["evidence_total"], 1);
        let evidence = &f["evidence"][0];
        assert_eq!(evidence["specifier"], specifier);
        assert_eq!(evidence["kind"], kind);
        let range = evidence["range"].as_str().unwrap();
        assert_eq!(range.split(':').next().unwrap(), line.to_string());
    }
    // The ranges of two evidence entries.
    assert_eq!(
        found[2]["evidence"][0],
        json!({"range": "5:11-5:28", "kind": "require", "specifier": "./c.js"})
    );
    assert_eq!(
        found[1]["evidence"][0],
        json!({"range": "1:1-1:25", "kind": "import", "specifier": "./a"})
    );
    assert_eq!(v["incomplete"]["counts"]["unresolved"], 2);
    let entries = v["incomplete"]["unresolved"].as_array().unwrap();
    assert_eq!(entries[0]["file"], "src/main.ts");
    assert_eq!(entries[0]["kind"], "dynamic_import");
    assert_eq!(entries[0]["target"], "./missing");
    assert_eq!(entries[0]["reason"], "no file matches ./missing");
    assert!(entries[0]["range"].as_str().unwrap().starts_with("8:"));
    assert_eq!(entries[1]["file"], "src/main.ts");
    assert_eq!(entries[1]["kind"], "require");
    assert_eq!(entries[1]["target"], "name");
    assert_eq!(entries[1]["reason"], "specifier is not a string literal");
    assert!(entries[1]["range"].as_str().unwrap().starts_with("9:"));
    let d = &v["details"];
    assert_eq!(relationships(&v), count_of(12, 8, 2, 2));
    assert_eq!(
        d["kinds"],
        json!([
            "dynamic_import",
            "import",
            "reexport",
            "require",
            "type_import"
        ])
    );
    assert_eq!(
        d["resolution_inputs"],
        json!([
            {"file": "package.json", "kind": "package.json"},
            {"file": "tsconfig.json", "kind": "tsconfig"}
        ])
    );
}

#[test]
fn the_python_fixture_gives_six_edges_and_one_unresolved_entry() {
    let ws = workspace();
    py(ws.path());
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("app/helpers.py", "app/util.py"),
            ("app/main.py", "app/helpers.py"),
            ("app/main.py", "app/models/__init__.py"),
            ("app/main.py", "app/util.py"),
            ("app/uses_lib.py", "src/lib/__init__.py"),
            ("scripts/run.py", "scripts/tool.py"),
        ])
    );
    assert_eq!(
        unresolved(&v),
        triples(&[(
            "app/main.py",
            "app.missing",
            "app.missing not found under ."
        )])
    );
    assert_eq!(v["incomplete"]["unresolved"][0]["range"], "5:1-5:19");
    assert_eq!(relationships(&v), count_of(9, 6, 2, 1));
    assert_eq!(
        v["details"]["resolution_inputs"],
        json!([{"file": "pyproject.toml", "kind": "pyproject.toml"}])
    );
}

#[test]
fn a_python_module_file_before_the_last_segment_gives_no_edge_and_no_cycle() {
    let ws = workspace();
    write(ws.path(), "a.py", "");
    write(ws.path(), "a/b.py", "import main\n");
    write(ws.path(), "main.py", "import a.b\nimport a . b\n");
    let v = graph(ws.path(), &[]);
    assert_eq!(edges(&v), pairs(&[("a/b.py", "main.py")]));
    assert_eq!(
        unresolved(&v),
        triples(&[
            ("main.py", "a.b", "a is not a package"),
            ("main.py", "a . b", "a is not a package"),
        ])
    );
    let cycles = analyze(ws.path(), "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
    write(ws.path(), "a/__init__.py", "");
    let v = graph(ws.path(), &[]);
    assert_eq!(
        edges(&v),
        pairs(&[("a/b.py", "main.py"), ("main.py", "a/b.py")])
    );
    let cycles = analyze(ws.path(), "cycles", &[]);
    assert_eq!(cycles["findings"][0]["members_total"], 2);
}

#[test]
fn a_python_name_bound_with_a_kelvin_sign_beats_a_submodule_and_gives_no_cycle() {
    let ws = workspace();
    // U+212A KELVIN SIGN is `K` after NFKC normalization.
    write(ws.path(), "pkg/__init__.py", "\u{212A} = 1\n");
    write(ws.path(), "pkg/K.py", "import consumer\n");
    write(ws.path(), "consumer.py", "from pkg import K\n");
    let v = graph(ws.path(), &[]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("consumer.py", "pkg/__init__.py"),
            ("pkg/K.py", "consumer.py")
        ])
    );
    let cycles = analyze(ws.path(), "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
}

#[test]
fn a_python_package_with_a_symlinked_initializer_finds_its_submodule_in_its_own_directory() {
    let ws = workspace();
    write(ws.path(), "other/__init__.py", "");
    write(ws.path(), "other/n.py", "");
    write(ws.path(), "pkg/n.py", "import main\n");
    write(ws.path(), "main.py", "from pkg import n\n");
    std::os::unix::fs::symlink("../other/__init__.py", ws.path().join("pkg/__init__.py")).unwrap();
    let v = graph(ws.path(), &[]);
    assert_eq!(
        edges(&v),
        pairs(&[("main.py", "pkg/n.py"), ("pkg/n.py", "main.py")])
    );
    let cycles = analyze(ws.path(), "cycles", &[]);
    assert_eq!(cycles["findings"][0]["members_total"], 2);
    assert_eq!(cycles["findings"][0]["members"][0]["file"], "main.py");
    assert_eq!(cycles["findings"][0]["members"][1]["file"], "pkg/n.py");
}

#[test]
fn the_go_fixture_gives_five_edges_and_its_variants_follow_the_module_rules() {
    let ws = workspace();
    go(ws.path(), true);
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("main.go", "store/cart.go"),
            ("main.go", "store/store.go"),
            ("store/store.go", "libs/db/db.go"),
            ("tools/gen.go", "store/cart.go"),
            ("tools/gen.go", "store/store.go"),
        ])
    );
    for to in ["store/cart.go", "store/store.go"] {
        let e = edge(&v, "main.go", to);
        assert_eq!(e["evidence"][0]["range"], "5:2-5:26");
        assert_eq!(e["evidence"][0]["specifier"], "example.com/shop/store");
    }
    assert_eq!(
        unresolved(&v),
        triples(&[(
            "main.go",
            "example.com/shop/gone",
            "no Go package files for example.com/shop/gone in the workspace"
        )])
    );
    assert_eq!(v["incomplete"]["unresolved"][0]["range"], "6:2-6:25");
    assert_eq!(
        v["details"]["resolution_inputs"],
        json!([
            {"file": "go.mod", "kind": "go.mod"},
            {"file": "libs/db/go.mod", "kind": "go.mod"},
            {"file": "tools/go.mod", "kind": "go.mod"}
        ])
    );
    // No edge targets a test file, with or without --include-tests.
    for args in [
        &["--limit", "1000"][..],
        &["--limit", "1000", "--include-tests"],
    ] {
        let v = graph(ws.path(), args);
        assert!(
            edges(&v).iter().all(|(_, to)| to != "store/store_test.go"),
            "{args:?}: {}",
            edges_text(&v)
        );
    }

    // Without libs/db/go.mod the import path names no package.
    let ws = workspace();
    go(ws.path(), false);
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert!(edges(&v).iter().all(|(from, _)| from != "store/store.go"));
    assert!(unresolved(&v).contains(&(
        "store/store.go".to_string(),
        "example.com/shop/store/db".to_string(),
        "no Go package files for example.com/shop/store/db in the workspace".to_string()
    )));

    // A second directory for the same import path is ambiguous.
    let ws = workspace();
    go(ws.path(), true);
    write(ws.path(), "store/db/x.go", "package db\n");
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert!(edges(&v).iter().all(|(from, _)| from != "store/store.go"));
    assert_eq!(
        reason_of(&v, "store/store.go"),
        ["ambiguous import: example.com/shop, example.com/shop/store/db"]
    );

    // A package of another module is not reached through this module's path.
    let ws = workspace();
    go(ws.path(), true);
    write(
        ws.path(),
        "main.go",
        "package main\n\nimport \"example.com/shop/tools\"\n",
    );
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert!(edges(&v).iter().all(|(from, _)| from != "main.go"));
    assert!(
        unresolved(&v)
            .iter()
            .any(|(f, t, _)| f == "main.go" && t == "example.com/shop/tools"),
        "{:?}",
        unresolved(&v)
    );
}

#[test]
fn the_rust_fixture_gives_six_edges_and_three_unresolved_entries() {
    let ws = workspace();
    rs(ws.path());
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("src/lib.rs", "src/net/http.rs"),
            ("src/lib.rs", "src/net/mod.rs"),
            ("src/lib.rs", "src/util.rs"),
            ("src/main.rs", "src/util.rs"),
            ("src/net/http.rs", "src/util.rs"),
            ("src/net/mod.rs", "src/net/http.rs"),
        ])
    );
    let use_edge = edge(&v, "src/lib.rs", "src/net/http.rs");
    assert_eq!(kinds(use_edge), ["use"]);
    assert!(
        use_edge["evidence"][0]["range"]
            .as_str()
            .unwrap()
            .starts_with("4:")
    );
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/net/mod.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/util.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/main.rs", "src/util.rs")), ["use"]);
    assert_eq!(kinds(edge(&v, "src/net/http.rs", "src/util.rs")), ["use"]);
    assert_eq!(
        kinds(edge(&v, "src/net/mod.rs", "src/net/http.rs")),
        ["module"]
    );
    assert_eq!(
        unresolved(&v),
        triples(&[
            (
                "src/extra.rs",
                "crate::util::helper",
                "file is not reached from a crate root"
            ),
            (
                "src/lib.rs",
                "gone",
                "neither src/gone.rs nor src/gone/mod.rs exists"
            ),
            (
                "src/lib.rs",
                "mystery::Thing",
                "first path segment is not a module, crate or dependency: mystery"
            ),
        ])
    );
    let lines: Vec<&str> = v["incomplete"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["range"].as_str().unwrap().split(':').next().unwrap())
        .collect();
    assert_eq!(lines, ["1", "3", "7"]);
    assert_eq!(relationships(&v), count_of(11, 6, 2, 3));
}

#[test]
fn a_file_with_a_syntax_error_still_gives_its_edge_and_is_listed() {
    let ws = workspace();
    write(
        ws.path(),
        "src/p.ts",
        "import { x } from './q';\nconst = ;\n",
    );
    write(ws.path(), "src/q.ts", "export const x = 1;\n");
    let v = graph(ws.path(), &[]);
    assert_eq!(edges(&v), pairs(&[("src/p.ts", "src/q.ts")]));
    let failures = v["incomplete"]["parse_failures"].as_array().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0]["file"], "src/p.ts");
}

#[test]
fn a_resolution_input_that_does_not_parse_leaves_every_relationship_unresolved() {
    let ws = workspace();
    rs(ws.path());
    write(ws.path(), "Cargo.toml", "[package");
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert_eq!(edges(&v), []);
    let inputs = v["details"]["resolution_inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0]["file"], "Cargo.toml");
    assert_eq!(inputs[0]["kind"], "Cargo.toml");
    assert!(inputs[0]["error"].is_string(), "{}", inputs[0]);
    assert_eq!(v["incomplete"]["skipped_files"], json!([]));
    let entries = unresolved(&v);
    assert_eq!(entries.len(), 11);
    assert!(
        entries
            .iter()
            .all(|(_, _, r)| r == "Cargo.toml could not be read")
    );
    assert_eq!(relationships(&v), count_of(11, 0, 0, 11));
    assert_eq!(v["status"], "incomplete");
}

#[test]
fn an_out_of_line_test_module_follows_include_tests() {
    let ws = workspace();
    rs(ws.path());
    write(
        ws.path(),
        "src/lib.rs",
        "mod net;\nmod util;\nmod gone;\npub use crate::net::http::Client;\nuse serde::Serialize;\nuse std::fmt;\nuse mystery::Thing;\n#[cfg(test)] mod tests;\n",
    );
    write(ws.path(), "src/tests.rs", "use super::*;\n");
    let v = graph(ws.path(), &["--limit", "1000"]);
    let found = edges(&v);
    assert!(!found.contains(&("src/lib.rs".to_string(), "src/tests.rs".to_string())));
    assert_eq!(kinds(edge(&v, "src/tests.rs", "src/lib.rs")), ["use"]);
    assert!(
        reason_of(&v, "src/tests.rs").is_empty(),
        "{:?}",
        unresolved(&v)
    );
    let v = graph(ws.path(), &["--limit", "1000", "--include-tests"]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/tests.rs")), ["module"]);
}

#[test]
fn an_inline_test_module_keeps_its_module_files_in_the_tree() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "pub struct A;\n#[cfg(test)]\nmod checks {\n    mod helper;\n}\n",
    );
    write(root, "src/checks/helper.rs", "use crate::A;\n");
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(edges(&v), pairs(&[("src/checks/helper.rs", "src/lib.rs")]));
    assert_eq!(unresolved(&v), []);
    let v = graph(root, &["--limit", "1000", "--include-tests"]);
    assert_eq!(
        kinds(edge(&v, "src/lib.rs", "src/checks/helper.rs")),
        ["module"]
    );
    assert_eq!(
        kinds(edge(&v, "src/checks/helper.rs", "src/lib.rs")),
        ["use"]
    );
}

#[test]
fn an_undeclared_bare_specifier_is_unresolved_until_it_is_declared() {
    let ws = ts_workspace();
    write(ws.path(), "src/c.ts", "import lodash from 'lodash';\n");
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert_eq!(
        reason_of(&v, "src/c.ts"),
        ["bare specifier is not a declared dependency: lodash"]
    );
    let entry = v["incomplete"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["file"] == "src/c.ts")
        .unwrap();
    assert_eq!(entry["target"], "lodash");
    assert_eq!(relationships(&v)["external"], 2);

    write(
        ws.path(),
        "package.json",
        r#"{"name":"ts-fixture","private":true,"dependencies":{"react":"18.3.1","lodash":"4.17.21"}}"#,
    );
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert!(reason_of(&v, "src/c.ts").is_empty());
    assert_eq!(relationships(&v)["external"], 3);

    write(ws.path(), "src/c.ts", "import fs from 'fs';\n");
    let v = graph(ws.path(), &["--limit", "1000"]);
    assert!(reason_of(&v, "src/c.ts").is_empty());
    assert_eq!(relationships(&v)["external"], 3);
}

#[test]
fn javascript_and_typescript_forms_resolve_as_documented() {
    let ws = workspace();
    let root = ws.path();
    for (rel, content) in [
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
    ] {
        write(root, rel, content);
    }
    let v = graph(root, &["--limit", "1000"]);
    let want = [
        ("cfg/m.ts", "cfg/t/a.ts", "import"),
        ("ext/m.ts", "ext/b/k.ts", "import"),
        ("src/q.ts", "packages/p/lib.js", "import"),
        ("src/t.ts", "src/y.js", "require"),
        ("src/u.ts", "src/y.js", "dynamic_import"),
        ("src/x.js", "src/y.js", "require"),
        ("src/x.jsx", "src/y.js", "reexport"),
    ];
    assert_eq!(
        edges(&v),
        pairs(&want.map(|(a, b, _)| (a, b))),
        "{:?}",
        unresolved(&v)
    );
    for (file, to, kind) in want {
        assert_eq!(kinds(edge(&v, file, to)), [kind], "{file}");
    }
    assert_eq!(
        unresolved(&v),
        triples(&[
            (
                "src/q2.ts",
                "e",
                "package.json \"exports\" of e is not applied"
            ),
            (
                "src/r.ts",
                "/src/y.js",
                "root-relative specifier is not applied"
            ),
            (
                "src/w.ts",
                "#internal",
                "package.json \"imports\" is not applied"
            ),
        ])
    );
    // `../../outside` leaves the workspace root: an external, not an entry.
    assert_eq!(relationships(&v), count_of(11, 7, 1, 3));
    let inputs: Vec<(&str, &str)> = v["details"]["resolution_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["file"].as_str().unwrap(), i["kind"].as_str().unwrap()))
        .collect();
    assert!(
        inputs.contains(&("ext/base.json", "tsconfig")),
        "{inputs:?}"
    );
}

#[test]
fn python_forms_resolve_as_documented() {
    let ws = workspace();
    let root = ws.path();
    for (rel, content) in [
        ("pkg/__init__.py", ""),
        ("pkg/a.py", "from .. import x\n"),
        ("top.py", "from . import y\n"),
        ("pkg/b.py", "import os, pkg.a\n"),
        ("pkg/c.py", "from pkg import *\n"),
        ("ns/mod.py", ""),
        ("pkg/d.py", "import ns.mod\n"),
        ("pkg/e.py", "from pkg import gone\n"),
        ("pkg/f.py", "import yaml\n"),
        ("lib/data.txt", ""),
        ("src/lib/__init__.py", ""),
        ("src/lib/x.py", ""),
        ("pkg/g.py", "import lib.x\n"),
    ] {
        write(root, rel, content);
    }
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("pkg/b.py", "pkg/a.py"),
            ("pkg/c.py", "pkg/__init__.py"),
            ("pkg/d.py", "ns/mod.py"),
            ("pkg/g.py", "src/lib/x.py"),
        ])
    );
    assert_eq!(
        edge(&v, "pkg/c.py", "pkg/__init__.py")["evidence"][0]["specifier"],
        "pkg.*"
    );
    assert_eq!(
        reason_of(&v, "pkg/a.py"),
        ["relative import beyond the top-level package"]
    );
    assert_eq!(
        reason_of(&v, "top.py"),
        ["relative import outside a package"]
    );
    assert_eq!(
        reason_of(&v, "pkg/e.py"),
        ["gone is neither bound in pkg/__init__.py nor a submodule of pkg"]
    );
    assert_eq!(
        reason_of(&v, "pkg/f.py"),
        [
            "yaml is not a standard-library module, a declared dependency or a module under the roots"
        ]
    );
    // `import os, pkg.a` is two relationships with one range.
    let b = edge(&v, "pkg/b.py", "pkg/a.py");
    assert_eq!(b["evidence"][0]["range"], "1:1-1:17");
    assert_eq!(relationships(&v), count_of(9, 4, 1, 4));
}

#[test]
fn go_forms_resolve_as_documented() {
    let ws = workspace();
    let root = ws.path();
    for (rel, content) in [
        (
            "go.mod",
            "module example.com/f\n\nreplace example.com/ext => ./third/ext\n",
        ),
        ("third/ext/go.mod", "module example.com/ext\n"),
        ("third/ext/e.go", "package ext\n"),
        ("p/p.go", "package p\n"),
        ("q/q.go", "package q\n"),
        ("r/r.go", "package r\n"),
        ("s/s.go", "package s\n"),
        (
            "main.go",
            "package main\n\nimport (\n\t\"C\"\n\t`example.com/f/p`\n\tq \"example.com/f/q\"\n\t. \"example.com/f/r\"\n\t_ \"example.com/f/s\"\n\t\"example.com/ext\"\n)\n",
        ),
    ] {
        write(root, rel, content);
    }
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("main.go", "p/p.go"),
            ("main.go", "q/q.go"),
            ("main.go", "r/r.go"),
            ("main.go", "s/s.go"),
            ("main.go", "third/ext/e.go"),
        ])
    );
    assert_eq!(unresolved(&v), []);
    assert_eq!(relationships(&v), count_of(6, 5, 1, 0));
}

#[test]
fn rust_forms_resolve_as_documented() {
    let ws = workspace();
    let root = ws.path();
    for (rel, content) in [
        (
            "Cargo.toml",
            "[package]\nname = \"forms-rs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"frs\"\n",
        ),
        (
            "src/lib.rs",
            "mod net;\n#[path = \"other/place.rs\"]\nmod moved;\ncfg_feature! { pub mod gated; }\npub use net::{self, Conn as C};\nextern crate alloc;\nstatic S: &str = include_str!(\"data.txt\");\n",
        ),
        ("src/net.rs", "pub mod http;\npub struct Conn;\n"),
        ("src/net/http.rs", ""),
        ("src/other/place.rs", "mod inner;\n"),
        ("src/other/inner.rs", ""),
        ("src/gated.rs", ""),
        ("src/data.txt", ""),
        ("src/main.rs", "use frs::net::Conn;\n"),
    ] {
        write(root, rel, content);
    }
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(
        edges(&v),
        pairs(&[
            ("src/lib.rs", "src/data.txt"),
            ("src/lib.rs", "src/gated.rs"),
            ("src/lib.rs", "src/net.rs"),
            ("src/lib.rs", "src/other/place.rs"),
            ("src/main.rs", "src/net.rs"),
            ("src/net.rs", "src/net/http.rs"),
            ("src/other/place.rs", "src/other/inner.rs"),
        ]),
        "{:?}",
        unresolved(&v)
    );
    let net = edge(&v, "src/lib.rs", "src/net.rs");
    assert_eq!(kinds(net), ["module", "use"]);
    assert_eq!(net["evidence_total"], 3);
    let data = edge(&v, "src/lib.rs", "src/data.txt");
    assert_eq!(kinds(data), ["include"]);
    assert_eq!(data["to_analyzed"], false);
    assert_eq!(
        kinds(edge(&v, "src/lib.rs", "src/other/place.rs")),
        ["module"]
    );
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/gated.rs")), ["module"]);
    assert_eq!(unresolved(&v), []);
}

#[test]
fn a_path_attribute_outside_the_parent_directory_gives_its_use_edge() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "#[path = \"../shared.rs\"]\nmod shared;\npub struct X;\n",
    );
    write(root, "shared.rs", "use crate::X;\n");
    for args in [
        &["--limit", "1000"][..],
        &["--limit", "1000", "shared.rs"][..],
    ] {
        let v = graph(root, args);
        assert_eq!(
            kinds(edge(&v, "shared.rs", "src/lib.rs")),
            ["use"],
            "{args:?}"
        );
        assert_eq!(reason_of(&v, "shared.rs"), Vec::<String>::new(), "{args:?}");
    }
}

#[test]
fn a_use_in_several_crate_trees_gives_an_edge_from_each_tree() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    for (root_file, imp) in [("src/lib.rs", "imp_lib.rs"), ("src/main.rs", "imp_bin.rs")] {
        write(
            root,
            root_file,
            &format!("#[path = \"../shared.rs\"]\nmod shared;\n#[path = \"{imp}\"]\nmod imp;\n"),
        );
        write(root, &format!("src/{imp}"), "pub struct Thing;\n");
    }
    write(root, "shared.rs", "use crate::imp::Thing;\n");
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(kinds(edge(&v, "shared.rs", "src/imp_bin.rs")), ["use"]);
    assert_eq!(kinds(edge(&v, "shared.rs", "src/imp_lib.rs")), ["use"]);
    assert_eq!(unresolved(&v), []);
}

#[test]
fn a_use_in_a_block_local_module_gives_no_edge_and_no_cycle() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "mod a;\nfn f() {\n    mod inner {\n        mod a {\n            pub struct A;\n        }\n        use self::a::A;\n    }\n}\n",
    );
    write(root, "src/a.rs", "use crate::f;\npub struct A;\n");
    let v = graph(root, &[]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/a.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/a.rs", "src/lib.rs")), ["use"]);
    assert_eq!(unresolved(&v), []);
    let cycles = analyze(root, "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
    assert_eq!(cycles["status"], "no_findings");
}

#[test]
fn a_leading_path_separator_before_a_use_list_gives_no_edge_and_no_cycle() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "pub struct Root;\nmod std;\nuse ::{std::fmt};\n",
    );
    write(root, "src/std.rs", "pub mod fmt {}\nuse crate::Root;\n");
    let v = graph(root, &[]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/std.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/std.rs", "src/lib.rs")), ["use"]);
    assert_eq!(unresolved(&v), []);
    let cycles = analyze(root, "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
    assert_eq!(cycles["status"], "no_findings");
}

#[test]
fn a_use_through_a_use_binding_in_a_body_gives_no_edge_and_no_cycle() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "mod a;\nmod b;\nfn f() {\n    use crate::b as a;\n    use a::Thing;\n}\n",
    );
    write(root, "src/a.rs", "use crate::f;\npub struct Thing;\n");
    write(root, "src/b.rs", "pub struct Thing;\n");
    let v = graph(root, &[]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/a.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/b.rs")), ["module", "use"]);
    assert_eq!(kinds(edge(&v, "src/a.rs", "src/lib.rs")), ["use"]);
    assert_eq!(unresolved(&v), []);
    let cycles = analyze(root, "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
    assert_eq!(cycles["status"], "no_findings");
}

#[test]
fn a_dependency_with_a_lib_name_is_used_by_that_name() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "foo/Cargo.toml",
        "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n\n[lib]\nname = \"bar\"\n",
    );
    write(root, "foo/src/lib.rs", "pub struct Thing;\n");
    write(
        root,
        "app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nfoo = { path = \"../foo\" }\n",
    );
    write(root, "app/src/lib.rs", "use bar::Thing;\n");
    write(root, "app/src/main.rs", "use foo::Thing;\n");
    let v = graph(root, &[]);
    assert_eq!(edges(&v), pairs(&[("app/src/lib.rs", "foo/src/lib.rs")]));
    assert_eq!(
        unresolved(&v),
        triples(&[(
            "app/src/main.rs",
            "foo::Thing",
            "first path segment is not a module, crate or dependency: foo"
        )])
    );
}

#[test]
fn a_file_module_in_a_body_gives_no_edge_from_elsewhere_and_no_cycle() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "mod a;\nmod c;\nfn f() {\n    #[path = \"b.rs\"]\n    mod a;\n    use a::Thing;\n}\n",
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
    let v = graph(root, &[]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/a.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/b.rs")), ["module", "use"]);
    assert_eq!(kinds(edge(&v, "src/b.rs", "src/c.rs")), ["use"]);
    assert_eq!(kinds(edge(&v, "src/c.rs", "src/a.rs")), ["use"]);
    assert!(
        !edges(&v).contains(&("src/c.rs".to_string(), "src/b.rs".to_string())),
        "{}",
        edges_text(&v)
    );
    assert_eq!(unresolved(&v), []);
    let cycles = analyze(root, "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
    assert_eq!(cycles["status"], "no_findings");
}

#[test]
fn a_file_module_of_a_macro_in_a_body_gives_no_edge_from_elsewhere_and_no_cycle() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
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
            "        mod a;\n",
            "    }\n",
            "}\n",
        ),
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
    let v = graph(root, &[]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/a.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/lib.rs", "src/b.rs")), ["module"]);
    assert_eq!(kinds(edge(&v, "src/b.rs", "src/c.rs")), ["use"]);
    assert_eq!(kinds(edge(&v, "src/c.rs", "src/a.rs")), ["use"]);
    assert!(
        !edges(&v).contains(&("src/c.rs".to_string(), "src/b.rs".to_string())),
        "{}",
        edges_text(&v)
    );
    assert_eq!(unresolved(&v), []);
    let cycles = analyze(root, "cycles", &[]);
    assert_eq!(cycles["findings"], json!([]));
    assert_eq!(cycles["status"], "no_findings");
}

#[test]
fn an_example_root_that_loads_a_source_file_gives_an_edge_from_its_tree() {
    let ws = workspace();
    let root = ws.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "#[path = \"imp_lib.rs\"]\nmod imp;\nmod shared;\n",
    );
    write(
        root,
        "examples/demo.rs",
        "#[path = \"../src/shared.rs\"]\nmod shared;\n#[path = \"../src/imp_ex.rs\"]\nmod imp;\n",
    );
    write(root, "src/imp_lib.rs", "pub struct Thing;\n");
    write(root, "src/imp_ex.rs", "pub struct Thing;\n");
    write(root, "src/shared.rs", "use crate::imp::Thing;\n");
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(kinds(edge(&v, "src/shared.rs", "src/imp_ex.rs")), ["use"]);
    assert_eq!(kinds(edge(&v, "src/shared.rs", "src/imp_lib.rs")), ["use"]);
    assert_eq!(unresolved(&v), []);
}

#[test]
fn a_main_that_is_a_directory_resolves_to_its_index() {
    let ws = workspace();
    let root = ws.path();
    write(root, "pkg/package.json", r#"{"main":"lib"}"#);
    write(root, "pkg/lib/index.js", "");
    write(root, "pkg/index.js", "");
    write(root, "a.js", "import './pkg';\n");
    let v = graph(root, &["--limit", "1000"]);
    assert_eq!(kinds(edge(&v, "a.js", "pkg/lib/index.js")), ["import"]);
    assert_eq!(unresolved(&v), []);
}

// ---- unsupported scopes, shape, determinism, evidence, artifacts ----

/// The four tools with the arguments that make them run on `ts`.
const TOOLS: [(&str, &[&str]); 4] = [
    ("graph", &[]),
    ("cycles", &[]),
    ("coupling", &[]),
    ("impact", &["--focus", "src/c.ts"]),
];

#[test]
fn a_scope_with_only_an_unsupported_language_is_unsupported() {
    let ws = workspace();
    write(ws.path(), "src/Main.java", "class Main {}\n");
    for (name, args) in [
        ("graph", &[][..]),
        ("cycles", &[][..]),
        ("coupling", &[][..]),
        ("impact", &["--focus", "src/Main.java"][..]),
    ] {
        let v = analyze(ws.path(), name, args);
        assert_eq!(v["status"], "unsupported", "{name}: {v}");
        assert_eq!(v["complete"], true, "{name}");
        assert_eq!(v["coverage"]["analyzed_files"], 0, "{name}");
        assert_eq!(
            v["coverage"]["languages"],
            json!([{"language": "Java", "files": 1, "supported": false}]),
            "{name}"
        );
    }
    acyclic(ws.path());
    let v = analyze(ws.path(), "cycles", &[]);
    assert_eq!(v["status"], "no_findings");
    let languages = v["coverage"]["languages"].as_array().unwrap();
    let java = languages.iter().find(|l| l["language"] == "Java").unwrap();
    assert_eq!(java["supported"], false);
}

fn keys(v: &Value) -> Vec<String> {
    let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

fn sorted(list: &[&str]) -> Vec<String> {
    let mut list: Vec<String> = list.iter().map(|s| s.to_string()).collect();
    list.sort();
    list
}

const COMMON_DETAILS: [&str; 5] = [
    "kinds",
    "kinds_total",
    "relationships",
    "resolution_inputs",
    "resolution_inputs_total",
];

const FOCUS_DETAILS: [&str; 8] = [
    "focus",
    "focus_total",
    "focus_files",
    "focus_not_analyzed",
    "focus_not_analyzed_total",
    "focus_files_not_analyzed",
    "focus_files_not_analyzed_total",
    "max_depth",
];

/// The `details` keys of each tool: the common keys and its own.
fn details_keys(name: &str) -> Vec<String> {
    let own: Vec<&str> = match name {
        "graph" => FOCUS_DETAILS
            .into_iter()
            .chain(["direction", "files", "not_followed", "not_followed_total"])
            .collect(),
        "cycles" => vec!["cyclic_files", "not_followed", "not_followed_total"],
        "coupling" => vec!["formulas", "edges"],
        "impact" => FOCUS_DETAILS
            .into_iter()
            .chain(["direct", "transitive"])
            .collect(),
        _ => unreachable!(),
    };
    sorted(&[&COMMON_DETAILS[..], &own[..]].concat())
}

/// The keys of a finding of each tool.
fn finding_keys(name: &str) -> Vec<String> {
    sorted(match name {
        "graph" => &[
            "file",
            "to",
            "to_analyzed",
            "depth",
            "kinds",
            "evidence",
            "evidence_total",
        ][..],
        "cycles" => &[
            "file",
            "members",
            "members_total",
            "edges_total",
            "path",
            "path_total",
        ][..],
        "coupling" => &[
            "file",
            "fan_in",
            "fan_out",
            "degree",
            "instability",
            "external",
            "unresolved",
        ][..],
        "impact" => &["file", "depth", "reaches", "chain", "chain_total"][..],
        _ => unreachable!(),
    })
}

/// The exact key sets of a result, at every level the conventions fix.
fn check_shape(name: &str, v: &Value, context: &str) {
    assert_eq!(
        keys(v),
        sorted(&[
            "analysis",
            "status",
            "complete",
            "findings",
            "total",
            "limit",
            "truncated",
            "coverage",
            "excluded",
            "incomplete",
            "evidence",
            "details",
        ]),
        "{context}"
    );
    assert_eq!(v["analysis"], name, "{context}");
    assert_eq!(
        keys(&v["coverage"]),
        sorted(&["analyzed_files", "languages", "unrecognized_files"]),
        "{context}"
    );
    assert_eq!(
        keys(&v["excluded"]),
        sorted(&[
            "tests",
            "generated",
            "vendored",
            "ignored",
            "counts",
            "truncated"
        ]),
        "{context}"
    );
    assert_eq!(
        keys(&v["excluded"]["counts"]),
        sorted(&["tests", "generated", "vendored", "ignored"]),
        "{context}"
    );
    assert_eq!(
        keys(&v["incomplete"]),
        sorted(&[
            "skipped_files",
            "parse_failures",
            "unresolved",
            "limits_hit",
            "counts",
            "truncated"
        ]),
        "{context}"
    );
    assert_eq!(
        keys(&v["incomplete"]["counts"]),
        sorted(&[
            "skipped_files",
            "parse_failures",
            "unresolved",
            "limits_hit"
        ]),
        "{context}"
    );
    assert_eq!(
        keys(&v["evidence"]),
        sorted(&["sources", "language_servers"]),
        "{context}"
    );
    assert_eq!(keys(&v["details"]), details_keys(name), "{context}");
    for (key, value) in v["details"].as_object().unwrap() {
        if value.is_array() {
            assert!(
                v["details"].get(format!("{key}_total")).is_some(),
                "{context}: details.{key} has no {key}_total"
            );
        }
    }
    for f in v["findings"].as_array().unwrap() {
        assert_eq!(keys(f), finding_keys(name), "{context}: {f}");
    }
}

#[test]
fn every_result_has_exactly_the_documented_keys_also_when_empty() {
    let full = ts_workspace();
    let empty = workspace();
    write(empty.path(), "src/a.ts", "export const a = 1;\n");
    for (name, args) in TOOLS {
        check_shape(
            name,
            &analyze(full.path(), name, args),
            &format!("ts {name}"),
        );
        let args: Vec<&str> = if name == "impact" {
            vec!["--focus", "src/a.ts"]
        } else {
            vec![]
        };
        let v = analyze(empty.path(), name, &args);
        check_shape(name, &v, &format!("empty {name}"));
        assert_eq!(v["findings"], json!([]), "{name}");
    }
    let v = analyze(full.path(), "graph", &["--focus", "src/main.ts"]);
    check_shape("graph", &v, "graph with a focus");
    let v = analyze(full.path(), "cycles", &[]);
    check_shape("cycles", &v, "cycles on ts");
    assert_eq!(v["findings"].as_array().unwrap().len(), 1);
}

#[test]
fn two_runs_print_byte_identical_output() {
    let ws = ts_workspace();
    for (name, args) in TOOLS
        .into_iter()
        .chain([("graph", &["--focus", "src/a.ts", "--max-depth", "2"][..])])
    {
        let first = exec(ws.path(), name, args);
        let second = exec(ws.path(), name, args);
        assert_eq!(first.code, Some(0), "{name}: {}", first.stderr);
        assert!(!first.stdout.is_empty());
        assert_eq!(first.stdout, second.stdout, "{name} {args:?}");
    }
}

/// Whether any object under `v` has the key `source`.
fn has_source_key(v: &Value) -> bool {
    match v {
        Value::Object(map) => map.contains_key("source") || map.values().any(has_source_key),
        Value::Array(list) => list.iter().any(has_source_key),
        _ => false,
    }
}

#[test]
fn evidence_is_tree_sitter_only_with_and_without_a_language_server() {
    let ws = ts_workspace();
    let want = json!({"sources": ["tree-sitter"], "language_servers": []});
    for (name, args) in TOOLS {
        let enabled = analyze(ws.path(), name, args);
        assert_eq!(enabled["evidence"], want, "{name}");
        let config = TempDir::new().unwrap();
        let out = base(ws.path(), config.path(), name)
            .args(args)
            .env("ACT_LITE_DISABLE_LSP", "1")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{name}");
        let disabled: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            disabled["evidence"], want,
            "{name} without a language server"
        );
        assert_eq!(disabled, enabled, "{name}");
        assert!(!has_source_key(&enabled["findings"]), "{name}");
        assert!(!has_source_key(&enabled["incomplete"]), "{name}");
    }
}

type Snapshot = BTreeMap<String, (Vec<u8>, SystemTime)>;

fn snapshot(root: &Path) -> Snapshot {
    fn walk(root: &Path, dir: &Path, out: &mut Snapshot) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let rel = path.strip_prefix(root).unwrap().display().to_string();
            let content = if meta.is_file() {
                fs::read(&path).unwrap()
            } else {
                Vec::new()
            };
            out.insert(rel, (content, meta.modified().unwrap()));
            if meta.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Snapshot::new();
    walk(root, root, &mut out);
    out
}

fn group_members(pgid: u32) -> String {
    let out = Command::new("pgrep")
        .args(["-g", &pgid.to_string()])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// A fixture label, its builder and a path to use as the impact focus.
type Fixture = (&'static str, fn(&Path), &'static str);

#[test]
fn resolution_inputs_are_read_and_never_written() {
    let fixtures: [Fixture; 3] = [
        ("ts", ts, "src/c.ts"),
        ("go", |root| go(root, true), "store"),
        ("rs", rs, "src/util.rs"),
    ];
    for (label, build, focus) in fixtures {
        let ws = workspace();
        build(ws.path());
        let config = TempDir::new().unwrap();
        let before = (snapshot(ws.path()), snapshot(config.path()));
        for (name, args) in [
            ("graph", &[][..]),
            ("cycles", &[][..]),
            ("coupling", &[][..]),
            ("impact", &["--focus", focus][..]),
        ] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_act-lite"));
            cmd.env("ACT_CONFIG_DIR", config.path())
                .arg("--workspace")
                .arg(ws.path())
                .args(["analyze", name])
                .args(args)
                .process_group(0)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut child = cmd.spawn().unwrap();
            let pgid = child.id();
            assert!(child.wait().unwrap().success(), "{label} {name}");
            assert_eq!(
                group_members(pgid).trim(),
                "",
                "{label} {name}: a process remains in the group"
            );
        }
        assert!(
            before.0 == snapshot(ws.path()),
            "{label}: the workspace changed"
        );
        assert!(
            before.1 == snapshot(config.path()),
            "{label}: the configuration directory changed"
        );
    }
}

#[test]
fn results_are_ordered_by_path_bytes() {
    // `a-b/x.ts` sorts before `a/b.ts`: `-` (0x2d) is below `/` (0x2f).
    let ws = workspace();
    let root = ws.path();
    write(root, "t.ts", "export const t = 1;\n");
    write(root, "a-b/x.ts", "import '../t';\n");
    write(root, "a/b.ts", "import '../t';\n");
    let v = graph(root, &[]);
    assert_eq!(
        edges(&v),
        pairs(&[("a-b/x.ts", "t.ts"), ("a/b.ts", "t.ts")])
    );
    let v = analyze(root, "impact", &["--focus", "t.ts"]);
    let files: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["a-b/x.ts", "a/b.ts"]);
    let v = analyze(root, "coupling", &[]);
    let files: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["t.ts", "a-b/x.ts", "a/b.ts"]);

    let ws = workspace();
    let root = ws.path();
    write(root, "a-b/x.ts", "import './y';\n");
    write(root, "a-b/y.ts", "import './x';\n");
    write(root, "a/b.ts", "import './c';\n");
    write(root, "a/c.ts", "import './b';\n");
    let v = analyze(root, "cycles", &[]);
    let files: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["a-b/x.ts", "a/b.ts"]);
}

// ---- truncation and nested bounds ----

#[test]
fn limit_cuts_the_edges_and_keeps_the_total() {
    let ws = ts_workspace();
    let all = graph(ws.path(), &["--limit", "1000"]);
    let v = graph(ws.path(), &["--limit", "3"]);
    assert_eq!(v["total"], 8);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["limit"], 3);
    assert_eq!(
        v["findings"],
        json!(all["findings"].as_array().unwrap()[..3])
    );
    let v = graph(ws.path(), &["--limit", "0"]);
    assert_eq!(v["findings"], json!([]));
    assert_eq!(v["total"], 8);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["status"], "findings");
}

#[test]
fn an_edge_keeps_five_evidence_entries_and_a_cycle_twenty_members_and_steps() {
    let ws = workspace();
    write(ws.path(), "src/x.ts", &"import './y';\n".repeat(7));
    write(ws.path(), "src/y.ts", "export const y = 1;\n");
    let v = graph(ws.path(), &[]);
    assert_eq!(v["total"], 1);
    let f = &v["findings"][0];
    assert_eq!(f["evidence"].as_array().unwrap().len(), 5);
    assert_eq!(f["evidence_total"], 7);

    let ws = workspace();
    for i in 0..60 {
        write(
            ws.path(),
            &format!("src/g{i:02}.ts"),
            &format!("import './g{:02}';\n", (i + 1) % 60),
        );
    }
    let v = analyze(ws.path(), "cycles", &[]);
    let f = &v["findings"][0];
    assert_eq!(f["members_total"], 60);
    assert_eq!(f["members"].as_array().unwrap().len(), 20);
    assert_eq!(f["path_total"], 60);
    assert_eq!(f["path"].as_array().unwrap().len(), 20);
}

/// A workspace with `src/x.ts` and 31 CSS files in 31 directories, so that a
/// focus of the 31 directories holds files that are not analyzed.
fn css_directories(root: &Path) -> Vec<String> {
    write(root, "src/x.ts", "export const x = 1;\n");
    (0..31)
        .map(|i| {
            write(root, &format!("d{i:02}/s.css"), ".x {}\n");
            format!("d{i:02}")
        })
        .collect()
}

fn focus_args<'a>(focus: &'a [String], extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = Vec::new();
    for f in focus {
        args.extend(["--focus", f.as_str()]);
    }
    args.extend(extra);
    args
}

fn check_focus_caps(v: &Value, context: &str) {
    let d = &v["details"];
    assert_eq!(d["focus"].as_array().unwrap().len(), 30, "{context}");
    assert_eq!(d["focus_total"], 31, "{context}");
    assert_eq!(d["focus"][0], "d00/", "{context}");
    assert_eq!(d["focus_files"], 31, "{context}");
    assert_eq!(d["focus_files_not_analyzed"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_files_not_analyzed_total"], 31, "{context}");
    assert_eq!(d["focus_not_analyzed"].as_array().unwrap().len(), 30);
    assert_eq!(d["focus_not_analyzed_total"], 31, "{context}");
}

#[test]
fn details_lists_are_capped_with_their_totals() {
    let ws = workspace();
    let focus = css_directories(ws.path());
    let v = graph(ws.path(), &focus_args(&focus, &["--direction", "in"]));
    check_focus_caps(&v, "graph");
    let v = analyze(ws.path(), "impact", &focus_args(&focus, &[]));
    check_focus_caps(&v, "impact");

    let ws = workspace();
    let mut source = String::new();
    for i in 0..101 {
        write(ws.path(), &format!("src/c{i:03}.css"), ".x {}\n");
        source.push_str(&format!("import './c{i:03}.css';\n"));
    }
    write(ws.path(), "src/x.ts", &source);
    let v = graph(ws.path(), &["--focus", "src/x.ts", "--direction", "out"]);
    let d = &v["details"];
    assert_eq!(d["not_followed"].as_array().unwrap().len(), 100);
    assert_eq!(d["not_followed_total"], 101);
    assert_eq!(d["not_followed"][0], json!({"file": "src/c000.css"}));

    // `analyze cycles` lists the targets outside its scope the same way.
    let ws = workspace();
    let imports: String = (0..101)
        .map(|i| format!("import '../out/b{i:03}';\n"))
        .collect();
    write(ws.path(), "in/a.ts", &imports);
    for i in 0..101 {
        write(
            ws.path(),
            &format!("out/b{i:03}.ts"),
            "export const b = 1;\n",
        );
    }
    let v = analyze(ws.path(), "cycles", &["in"]);
    assert_eq!(v["details"]["not_followed"].as_array().unwrap().len(), 100);
    assert_eq!(v["details"]["not_followed_total"], 101);
}

// ---- the two counterexamples ----

#[test]
fn a_diamond_of_imports_is_no_cycle_and_stays_complete() {
    let ws = workspace();
    acyclic(ws.path());
    let v = analyze(ws.path(), "cycles", &[]);
    assert_eq!(v["status"], "no_findings");
    assert_eq!(v["complete"], true);
    assert_eq!(v["findings"], json!([]));
}

#[test]
fn an_unresolved_import_is_never_read_as_no_dependents() {
    let ws = workspace();
    unres(ws.path(), true);
    let v = analyze(ws.path(), "impact", &["--focus", "src/t.ts"]);
    assert_eq!(v["status"], "incomplete");
    assert_eq!(v["findings"], json!([]));
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
    let ws = workspace();
    unres(ws.path(), false);
    let v = analyze(ws.path(), "impact", &["--focus", "src/t.ts"]);
    assert_eq!(v["status"], "no_findings");
    assert_eq!(v["complete"], true);
}

// ---- cancellation ----

/// Files of a workspace that `analyze cycles` takes several seconds to read:
/// 15,000 files in 100 directories, each importing 20 files of its directory.
fn slow_workspace(root: &Path) {
    let total = 15_000;
    for i in 0..total {
        let mut source = String::new();
        for k in 0..20 {
            source.push_str(&format!(
                "import {{ v{k} }} from './f{}';\n",
                (i + 100 * (k + 1)) % total
            ));
        }
        write(root, &format!("d{:03}/f{i}.ts", i % 100), &source);
    }
}

#[test]
fn sigint_ends_a_long_cycle_analysis_without_output() {
    // Local only: it needs a workspace that takes over a second to analyze.
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: ACT_LITE_CHECK_PERF not set");
        return;
    }
    let ws = workspace();
    slow_workspace(ws.path());
    let config = TempDir::new().unwrap();
    let start = Instant::now();
    let run = exec(ws.path(), "cycles", &["--limit", "0"]);
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert!(
        start.elapsed() > Duration::from_secs(1),
        "the workspace is too small: {:?}",
        start.elapsed()
    );

    let child = base(ws.path(), config.path(), "cycles")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let sent = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.signal(), Some(2));
    assert!(out.stdout.is_empty());
}
