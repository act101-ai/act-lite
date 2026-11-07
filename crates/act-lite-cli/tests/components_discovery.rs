//! Discovery of Vue, Svelte and Astro component files through the built
//! binary: they are recognized by their extension only, `repo-outline` lists
//! them under their language, near-miss names are refused, and the directory
//! rule makes a component file a test file.

mod langs_added;

use langs_added::{Mcp, cli, cli_json};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// A path under `tests/fixtures/components/`.
fn component(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/components")
        .join(rel)
}

/// A temporary workspace holding each `(path, content)` of `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

/// The `(path, language)` pairs `query repo-outline` lists, paths with `/`.
fn outline(ws: &Path) -> Vec<(String, String)> {
    let outline = cli_json(ws, &["query", "repo-outline"]);
    outline["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no files in {outline}"))
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().replace('\\', "/"),
                f["language"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn pair(path: &str, language: &str) -> (String, String) {
    (path.to_string(), language.to_string())
}

fn fixture_workspace() -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    for name in ["a.vue", "a.svelte", "a.astro"] {
        std::fs::copy(component(name), ws.path().join(name)).unwrap();
    }
    ws
}

#[test]
fn skeleton_answers_each_component_file() {
    let ws = fixture_workspace();
    let mut mcp = Mcp::start(ws.path());
    for name in ["a.vue", "a.svelte", "a.astro"] {
        let output = cli(ws.path(), &["query", "skeleton", name]);
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let skeleton: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(skeleton["declarations"].is_array(), "{name}: {skeleton}");
        let result = mcp
            .call("skeleton", json!({ "file": name }))
            .unwrap_or_else(|e| panic!("{name}: MCP skeleton failed: {e}"));
        assert_ne!(result["isError"], Value::Bool(true), "{name}: {result}");
    }
}

#[test]
fn repo_outline_lists_component_files_under_their_language() {
    let ws = fixture_workspace();
    let mut listed = outline(ws.path());
    listed.sort();
    assert_eq!(
        listed,
        [
            pair("a.astro", "Astro"),
            pair("a.svelte", "Svelte"),
            pair("a.vue", "Vue"),
        ]
    );
}

#[test]
fn sveltekit_and_astro_conventional_names_are_found_by_their_extension() {
    let ws = workspace(&[
        ("src/routes/+page.svelte", "<p>page</p>\n"),
        ("src/routes/+layout.svelte", "<slot />\n"),
        ("src/pages/index.astro", "---\n---\n<p>index</p>\n"),
        ("src/lib/state.svelte.ts", "export const a = 1;\n"),
        ("src/lib/state.svelte.js", "export const b = 2;\n"),
    ]);
    let mut listed = outline(ws.path());
    listed.sort();
    assert_eq!(
        listed,
        [
            pair("src/lib/state.svelte.js", "JavaScript"),
            pair("src/lib/state.svelte.ts", "TypeScript"),
            pair("src/pages/index.astro", "Astro"),
            pair("src/routes/+layout.svelte", "Svelte"),
            pair("src/routes/+page.svelte", "Svelte"),
        ]
    );
}

#[test]
fn near_miss_component_names_are_refused() {
    for name in ["Counter.vue.bak", "x.vuex", "a.mdx"] {
        let ws = workspace(&[(name, "<template><p/></template>\n")]);
        let output = cli(ws.path(), &["query", "skeleton", name]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{name} must be refused: {stderr}");
        assert!(
            stderr.contains("unsupported file type") && stderr.contains(name),
            "{name}: {stderr}"
        );
        assert!(outline(ws.path()).is_empty(), "{name} listed");
    }
}

#[test]
fn the_directory_rule_makes_a_component_file_a_test_file() {
    let content = "<script setup lang=\"ts\">\nconst a = 1;\n</script>\n";
    let ws = workspace(&[("src/__tests__/A.vue", content), ("src/A.vue", content)]);
    let patterns = cli_json(ws.path(), &["analyze", "patterns"]);
    assert_eq!(
        patterns["excluded"]["tests"],
        json!([{ "path": "src/__tests__/A.vue" }]),
        "{patterns}"
    );
    assert_eq!(
        patterns["coverage"]["languages"],
        json!([{ "language": "Vue", "files": 1, "supported": true }]),
        "{patterns}"
    );
}
