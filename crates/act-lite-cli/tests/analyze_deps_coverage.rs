//! The coverage of the dependency analyzers over the 40 languages: for one
//! fixture per language, `act-lite analyze coupling` lists the language as
//! supported for the seven that have a file-dependency rule and as unsupported
//! for the other 33, and the README's language table says the same.

use act_lite_parser::Grammar;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The seven languages with a file-dependency rule.
const SUPPORTED: [&str; 7] = [
    "JavaScript",
    "JSX",
    "TypeScript",
    "TSX",
    "Python",
    "Go",
    "Rust",
];

/// Every language with its fixture, relative to `tests/fixtures/`.
const ENTRIES: [(&str, &str); 40] = [
    ("Bash", "langs/bash/fixture.sh"),
    ("C", "langs/c/fixture.c"),
    ("CMake", "langs-added/cmake/CMakeLists.txt"),
    ("C++", "langs/cpp/fixture.cpp"),
    ("C#", "langs/csharp/fixture.cs"),
    ("CSS", "langs/css/fixture.css"),
    ("Dart", "langs/dart/fixture.dart"),
    ("Dockerfile", "langs/dockerfile/Dockerfile"),
    ("Elixir", "langs/elixir/fixture.ex"),
    ("Go", "langs/go/fixture.go"),
    ("GraphQL", "langs-added/graphql/schema.graphql"),
    ("HCL/Terraform", "langs/hcl/fixture.tf"),
    ("HTML", "langs/html/fixture.html"),
    ("Java", "langs/java/Main.java"),
    ("JavaScript", "langs/javascript/fixture.js"),
    ("JSX", "langs/jsx/fixture.jsx"),
    ("JSON", "langs/json/fixture.json"),
    ("JSONC", "langs-added/jsonc/settings.jsonc"),
    ("Julia", "langs-added/julia/geometry.jl"),
    ("Kotlin", "langs/kotlin/fixture.kt"),
    ("Lua", "langs/lua/fixture.lua"),
    ("Make", "langs-added/make/Makefile"),
    ("Markdown", "langs/markdown/fixture.md"),
    ("Nix", "langs-added/nix/default.nix"),
    ("PHP", "langs/php/fixture.php"),
    ("Protocol Buffers", "langs-added/protobuf/shop.proto"),
    ("Python", "langs/python/fixture.py"),
    ("R", "langs-added/r/scale.R"),
    ("Ruby", "langs/ruby/fixture.rb"),
    ("Rust", "langs/rust/fixture.rs"),
    ("Scala", "langs/scala/fixture.scala"),
    ("SQL", "langs/sql/fixture.sql"),
    ("SCSS", "langs/scss/fixture.scss"),
    ("Swift", "langs/swift/fixture.swift"),
    ("TOML", "langs/toml/fixture.toml"),
    ("TSX", "langs/tsx/fixture.tsx"),
    ("TypeScript", "langs/typescript/fixture.ts"),
    ("XML", "langs-added/xml/pom.xml"),
    ("YAML", "langs/yaml/fixture.yaml"),
    ("Zig", "langs-added/zig/point.zig"),
];

fn manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn supported(name: &str) -> bool {
    SUPPORTED.contains(&name)
}

/// Runs `act-lite analyze coupling <file>` in a workspace that holds a copy of
/// the fixture, and returns the parsed result.
fn coupling_of(rel: &str) -> Value {
    let source = manifest().join("tests/fixtures").join(rel);
    let name = source.file_name().unwrap().to_str().unwrap().to_string();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join(".git")).unwrap();
    std::fs::copy(&source, workspace.path().join(&name)).unwrap();
    let config = tempfile::tempdir().unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .current_dir(workspace.path())
        .env("ACT_CONFIG_DIR", config.path())
        .arg("--workspace")
        .arg(workspace.path())
        .args(["analyze", "coupling", &name])
        .output()
        .expect("the act-lite binary must start");
    assert!(
        output.status.success(),
        "{rel}: act-lite failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout is JSON")
}

#[test]
fn the_entry_table_has_one_entry_per_grammar() {
    let grammars: BTreeSet<&str> = Grammar::all().iter().map(|g| g.name()).collect();
    let entries: BTreeSet<&str> = ENTRIES.iter().map(|(name, _)| *name).collect();
    assert_eq!(Grammar::all().len(), 40);
    assert_eq!(ENTRIES.len(), 40);
    assert_eq!(entries.len(), 40, "an entry name repeats");
    assert_eq!(entries, grammars);
    assert_eq!(SUPPORTED.iter().filter(|n| entries.contains(*n)).count(), 7);
}

#[test]
fn every_fixture_exists() {
    for (name, rel) in ENTRIES {
        assert!(
            manifest().join("tests/fixtures").join(rel).is_file(),
            "{name}: no fixture at {rel}"
        );
    }
}

#[test]
fn coupling_lists_each_language_with_the_expected_support() {
    for (name, rel) in ENTRIES {
        let out = coupling_of(rel);
        let languages = out["coverage"]["languages"].as_array().unwrap();
        assert_eq!(languages.len(), 1, "{name}: {out}");
        assert_eq!(languages[0]["language"], name, "{name}: {out}");
        assert_eq!(languages[0]["files"], 1, "{name}: {out}");
        assert_eq!(languages[0]["supported"], supported(name), "{name}: {out}");
        if !supported(name) {
            assert_eq!(out["status"], "unsupported", "{name}: {out}");
        }
    }
}

/// The rows of the README's table of dependency-analyzer languages: the
/// language, its `Supported` cell and its reason cell.
fn readme_rows() -> Vec<(String, String, String)> {
    let text = std::fs::read_to_string(manifest().join("../../README.md"))
        .unwrap()
        .replace("\r\n", "\n");
    let section = text
        .split("\n## Dependency analyzers\n")
        .nth(1)
        .expect("the README has a Dependency analyzers section")
        .split("\n## ")
        .next()
        .unwrap();
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in section.lines() {
        if !line.starts_with('|') {
            in_table = false;
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        let cells: Vec<String> = cells.iter().map(|c| c.trim().to_string()).collect();
        if cells[0] == "Language" {
            assert_eq!(cells.len(), 5, "{line}");
            assert_eq!(cells[3], "Supported");
            in_table = true;
        } else if in_table && !cells[0].starts_with("---") {
            assert_eq!(cells.len(), 5, "{line}");
            rows.push((cells[0].clone(), cells[3].clone(), cells[4].clone()));
        }
    }
    rows
}

#[test]
fn the_readme_table_has_the_same_rows_and_values() {
    let rows = readme_rows();
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    let expected: Vec<&str> = ENTRIES.iter().map(|(name, _)| *name).collect();
    fn sorted<'a>(v: &[&'a str]) -> Vec<&'a str> {
        let mut v = v.to_vec();
        v.sort();
        v
    }
    assert_eq!(rows.len(), 40, "rows: {names:?}");
    assert_eq!(sorted(&names), sorted(&expected));
    for (name, value, reason) in &rows {
        if supported(name) {
            assert_eq!(value, "yes", "{name}");
        } else {
            assert_eq!(value, "no", "{name}");
            assert!(
                !reason.is_empty() && reason != "—",
                "{name}: no reason for `no`"
            );
        }
    }
}
