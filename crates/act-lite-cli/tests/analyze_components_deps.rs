//! Component files (Vue, Svelte, Astro) through the dependency tools of the
//! built binary (`act-lite --workspace <dir> analyze <tool> ...`): component
//! files are listed as unsupported languages, their script imports are not
//! relationships, and an import of a component file is an edge to a file
//! outside the analyzed set.
//! Every workspace is synthetic and built in a temporary directory.

use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Runs `act-lite --workspace <root> analyze <name> <args>` with an empty
/// configuration directory and returns the parsed stdout of a zero exit.
fn analyze(root: &Path, name: &str, args: &[&str]) -> Value {
    let config = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_act-lite"))
        .env("ACT_CONFIG_DIR", config.path())
        .arg("--workspace")
        .arg(root)
        .args(["analyze", name])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("stdout is one JSON value")
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    dir
}

const COMPONENTS: [(&str, &str, &str); 3] = [
    (
        "Card.vue",
        "Vue",
        "<script setup>\nimport { u } from './util';\n</script>\n<template><p/></template>\n",
    ),
    (
        "Card.svelte",
        "Svelte",
        "<script>\nimport { u } from './util';\n</script>\n<p></p>\n",
    ),
    (
        "Card.astro",
        "Astro",
        "---\nimport { u } from './util';\n---\n<p></p>\n",
    ),
];

fn language(v: &Value, name: &str) -> Value {
    v["coverage"]["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["language"] == name)
        .unwrap_or_else(|| panic!("{name} not listed: {v}"))
        .clone()
}

#[test]
fn each_component_entry_alone_is_unsupported_for_each_dependency_tool() {
    for (file, name, source) in COMPONENTS {
        let dir = workspace();
        write(dir.path(), file, source);
        for (tool, args) in [
            ("graph", vec![]),
            ("cycles", vec![]),
            ("coupling", vec![]),
            ("impact", vec!["--focus", file]),
        ] {
            let v = analyze(dir.path(), tool, &args);
            assert_eq!(v["status"], "unsupported", "{tool} on {file}: {v}");
            assert_eq!(
                v["coverage"]["languages"],
                json!([{"language": name, "files": 1, "supported": false}]),
                "{tool} on {file}"
            );
            assert_eq!(v["findings"], json!([]), "{tool} on {file}");
        }
    }
}

#[test]
fn coupling_lists_the_three_component_languages_as_unsupported() {
    let dir = workspace();
    write(dir.path(), "util.ts", "export const u = 1;\n");
    for (file, _, source) in COMPONENTS {
        write(dir.path(), file, source);
    }
    let v = analyze(dir.path(), "coupling", &[]);
    for (_, name, _) in COMPONENTS {
        assert_eq!(
            language(&v, name),
            json!({"language": name, "files": 1, "supported": false})
        );
    }
    assert_eq!(v["details"]["relationships"]["total"], 0, "{v}");
}

#[test]
fn an_import_of_a_component_is_an_edge_and_one_without_extension_is_unresolved() {
    let dir = workspace();
    write(
        dir.path(),
        "main.ts",
        "import Card from './Card.vue';\nimport Other from './Other';\n",
    );
    write(dir.path(), "Card.vue", COMPONENTS[0].2);
    write(dir.path(), "Other.vue", "<template><p/></template>\n");
    let v = analyze(
        dir.path(),
        "graph",
        &["--focus", "main.ts", "--direction", "out"],
    );
    let findings = v["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{v}");
    assert_eq!(findings[0]["file"], "main.ts");
    assert_eq!(findings[0]["to"], "Card.vue");
    assert_eq!(findings[0]["to_analyzed"], false);
    let unresolved = v["incomplete"]["unresolved"].as_array().unwrap();
    assert_eq!(unresolved.len(), 1, "{v}");
    assert_eq!(unresolved[0]["file"], "main.ts");
    assert_eq!(unresolved[0]["target"], "./Other");
    assert_eq!(unresolved[0]["reason"], "no file matches ./Other");
}

#[test]
fn a_component_importing_a_file_is_not_its_dependent() {
    let dir = workspace();
    write(dir.path(), "util.ts", "export const u = 1;\n");
    write(dir.path(), "Card.vue", COMPONENTS[0].2);
    let v = analyze(dir.path(), "impact", &["--focus", "util.ts"]);
    assert_eq!(v["findings"], json!([]), "{v}");
    assert_eq!(v["details"]["direct"], 0);
    assert_eq!(v["details"]["transitive"], 0);
    assert_eq!(
        language(&v, "Vue"),
        json!({"language": "Vue", "files": 1, "supported": false})
    );
}
