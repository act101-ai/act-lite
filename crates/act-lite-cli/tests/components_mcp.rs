//! Vue, Svelte and Astro component files through `act-lite mcp serve`,
//! beside the built binary: the structure of the committed fixtures, the
//! first content block against the command-line output for navigation,
//! malformed regions, the analyzers and references, and the output limits.

#[allow(dead_code)] // not every helper of the shared module is used here
mod langs_added;

use langs_added::Mcp;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const DISABLED: &[(&str, &str)] = &[("ACT_LITE_DISABLE_LSP", "1")];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/components")
        .join(name)
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

fn serve(ws: &Path) -> Mcp {
    Mcp::start_with_env(ws, DISABLED)
}

/// The parsed stdout of the built `act-lite --workspace <ws> <args>`, run
/// with an empty configuration directory and `ACT_LITE_DISABLE_LSP=1`.
fn cli(ws: &Path, args: &[&str]) -> Value {
    let config = tempfile::tempdir().unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .current_dir(ws)
        .env("ACT_CONFIG_DIR", config.path())
        .envs(DISABLED.iter().copied())
        .arg("--workspace")
        .arg(ws)
        .args(args)
        .output()
        .expect("the act-lite binary must start");
    assert!(
        output.status.success(),
        "act-lite {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout is one JSON value")
}

/// The text of the first content block of a successful call.
fn first_text(mcp: &mut Mcp, tool: &str, args: Value) -> String {
    let result = mcp
        .call(tool, args.clone())
        .unwrap_or_else(|e| panic!("{tool} {args} failed: {e}"));
    assert_ne!(result["isError"], true, "{tool} {args}: {result}");
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool}: no text block in {result}"))
        .to_string()
}

/// The JSON value at the start of the first content block, and the text
/// after it.
fn first_block(mcp: &mut Mcp, tool: &str, args: Value) -> (Value, String) {
    let text = first_text(mcp, tool, args);
    let mut values = serde_json::Deserializer::from_str(&text).into_iter::<Value>();
    let value = values
        .next()
        .and_then(Result::ok)
        .unwrap_or_else(|| panic!("{tool}: not JSON: {text}"));
    let rest = text[values.byte_offset()..].to_string();
    (value, rest)
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// `(kind, name, start line, end line)`.
type Lines = (String, String, u64, u64);

/// A row of a table: `(kind, name, start line, end line)`.
type Expected = (&'static str, &'static str, u64, u64);

const COUNTER_VUE: &[Expected] = &[
    ("module", "<script lang=\"ts\">", 2, 4),
    ("export", "export default { name: 'Counter' }", 3, 3),
    ("module", "<script setup lang=\"ts\">", 6, 15),
    ("import", "import { ref } from 'vue'", 7, 7),
    ("variable", "props", 8, 8),
    ("variable", "count", 9, 9),
    ("function", "increment", 10, 14),
    ("module", "<template>", 17, 22),
    ("element", "div", 18, 21),
    ("field", "app", 18, 18),
    ("field", "counter", 18, 18),
    ("field", "big", 18, 18),
    ("element", "button", 19, 19),
    ("element", "span", 20, 20),
    ("module", "<style scoped>", 24, 26),
    ("class", ".counter", 25, 25),
    ("field", "color", 25, 25),
];

const COUNTER_SVELTE: &[Expected] = &[
    ("module", "<script context=\"module\" lang=\"ts\">", 1, 3),
    ("export", "export const prerender = true;", 2, 2),
    ("variable", "prerender", 2, 2),
    ("element", "p", 5, 5),
    ("module", "<script lang=\"ts\">", 5, 13),
    ("export", "export let start: number = 0;", 5, 5),
    ("variable", "start", 5, 5),
    ("variable", "count", 6, 6),
    ("function", "increment", 8, 12),
    ("element", "div", 15, 21),
    ("field", "counter", 15, 15),
    ("element", "button", 16, 16),
    ("element", "span", 18, 18),
    ("function", "row", 20, 20),
    ("element", "i", 20, 20),
    ("module", "<style>", 23, 25),
    ("class", ".counter", 24, 24),
    ("field", "color", 24, 24),
];

const CARD_ASTRO: &[Expected] = &[
    ("module", "---", 1, 8),
    (
        "import",
        "import Layout from '../layouts/Layout.astro';",
        2,
        2,
    ),
    ("interface", "Props", 3, 3),
    ("function", "shout", 5, 7),
    ("element", "Layout", 9, 11),
    ("element", "h1", 10, 10),
    ("field", "top", 10, 10),
    ("field", "t", 10, 10),
    ("module", "<script>", 12, 14),
    ("function", "hello", 13, 13),
    ("module", "<script is:inline>", 15, 15),
    ("variable", "inline", 15, 15),
    ("module", "<script type=\"application/ld+json\">", 16, 16),
    ("module", "<style>", 17, 19),
    ("class", "h1", 18, 18),
    ("field", "color", 18, 18),
];

const FIXTURES: [(&str, &[Expected]); 3] = [
    ("Counter.vue", COUNTER_VUE),
    ("Counter.svelte", COUNTER_SVELTE),
    ("Card.astro", CARD_ASTRO),
];

fn fixture_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, _) in FIXTURES {
        std::fs::copy(fixture(name), dir.path().join(name)).unwrap();
    }
    dir
}

fn set<T: Ord + Clone>(items: &[T]) -> BTreeSet<T> {
    items.iter().cloned().collect()
}

fn blocks(rows: &[Lines]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.0 == "module")
        .map(|r| r.1.clone())
        .collect()
}

/// The MCP `skeleton` rows.
fn mcp_rows(declarations: &Value) -> Vec<Lines> {
    declarations
        .as_array()
        .unwrap_or_else(|| panic!("no declarations: {declarations}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                d["line"].as_u64().unwrap(),
                d["end_line"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// The command-line `skeleton` or `symbols` rows, by lines.
fn cli_rows(items: &Value) -> Vec<Lines> {
    items
        .as_array()
        .unwrap_or_else(|| panic!("no items: {items}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                d["range"]["start"]["line"].as_u64().unwrap(),
                d["range"]["end"]["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// MCP `symbols` rows, with the kind in lower case as the command line
/// writes it.
fn mcp_symbol_rows(symbols: &Value) -> Vec<Lines> {
    mcp_rows(symbols)
        .into_iter()
        .map(|(k, n, s, e)| (k.to_lowercase(), n, s, e))
        .collect()
}

fn without_imports(rows: &[Lines]) -> Vec<Lines> {
    rows.iter().filter(|r| r.0 != "import").cloned().collect()
}

// ---- structure over MCP ----

#[test]
fn skeleton_of_each_fixture_is_its_table_without_imports() {
    let ws = fixture_workspace();
    let mut mcp = serve(ws.path());
    for (name, expected) in FIXTURES {
        let (result, _) = first_block(&mut mcp, "skeleton", json!({"file": name}));
        let actual = mcp_rows(&result["declarations"]);
        let expected: Vec<Lines> = expected
            .iter()
            .filter(|(k, ..)| *k != "import")
            .map(|(k, n, s, e)| (k.to_string(), n.to_string(), *s, *e))
            .collect();
        assert_eq!(set(&actual), set(&expected), "{name}: {actual:#?}");
        assert_eq!(actual.len(), expected.len(), "{name}");
        assert_eq!(blocks(&actual), blocks(&expected), "{name}");
        assert!(result.get("parse_error").is_none(), "{name}");

        // The first block against the command line.
        let from_cli = cli(ws.path(), &["query", "skeleton", name]);
        assert_eq!(
            actual,
            without_imports(&cli_rows(&from_cli["declarations"])),
            "{name}"
        );
    }
}

#[test]
fn symbols_symbols_batch_and_repo_outline_match_the_command_line() {
    let ws = fixture_workspace();
    let mut mcp = serve(ws.path());
    for (name, _) in FIXTURES {
        let (result, _) = first_block(&mut mcp, "symbols", json!({"file": name}));
        assert!(result.get("parse_error").is_none(), "{name}");
        let from_cli = cli(ws.path(), &["query", "symbols", name]);
        assert_eq!(
            mcp_symbol_rows(&result["symbols"]),
            cli_rows(&from_cli["symbols"]),
            "{name}"
        );

        let (batch, _) = first_block(&mut mcp, "symbols_batch", json!({"files": [name]}));
        assert_eq!(
            batch,
            cli(ws.path(), &["query", "symbols-batch", "--files", name]),
            "{name}"
        );
    }
    let (outline, _) = first_block(&mut mcp, "repo_outline", json!({"symbols": true}));
    let from_cli = cli(ws.path(), &["query", "repo-outline", "--symbols"]);
    assert_eq!(outline["files"], from_cli["files"]);
    assert_eq!(outline["languages"], from_cli["languages"]);
    let listed: Vec<(&str, &str)> = outline["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap(), f["language"].as_str().unwrap()))
        .collect();
    assert_eq!(
        set(&listed),
        set(&[
            ("Card.astro", "Astro"),
            ("Counter.svelte", "Svelte"),
            ("Counter.vue", "Vue")
        ])
    );
}

// ---- malformed regions ----

fn malformed_workspace() -> tempfile::TempDir {
    workspace(&[
        (
            "Unclosed.vue",
            "<script setup lang=\"ts\">\nconst a = 1\n\n<template>\n  <p>{{ a }}</p>\n</template>\n",
        ),
        ("Unclosed.astro", "---\nconst a = 1;\n<p>{a}</p>\n"),
        (
            "BadTs.vue",
            "<template><p/></template>\n<script lang=\"ts\">\nfunction f(a: number {\n  return a\n}\nfunction g(b: number) { return b }\n</script>\n",
        ),
        (
            "Regex.astro",
            "---\nfunction e(v) { return v.replace(/\"/g, \"&quot;\"); }\n---\n<p/>\n",
        ),
        (
            "Amp.svelte",
            "<p>Set & go</p>\n<script>function f() {}</script>\n",
        ),
    ])
}

#[test]
fn malformed_regions_give_the_command_line_answer() {
    let ws = malformed_workspace();
    let mut mcp = serve(ws.path());
    for name in [
        "Unclosed.vue",
        "Unclosed.astro",
        "BadTs.vue",
        "Regex.astro",
        "Amp.svelte",
    ] {
        let (result, _) = first_block(&mut mcp, "symbols", json!({"file": name}));
        let from_cli = cli(ws.path(), &["query", "symbols", name]);
        assert_eq!(
            mcp_symbol_rows(&result["symbols"]),
            cli_rows(&from_cli["symbols"]),
            "{name}"
        );
        assert_eq!(result["parse_error"], true, "{name}");
        assert_eq!(from_cli["parse_error"], true, "{name}");
    }

    let (patterns, _) = first_block(&mut mcp, "analyze_patterns", json!({}));
    assert_eq!(patterns, cli(ws.path(), &["analyze", "patterns"]));
    let listed: Vec<(&str, &str, &str)> = patterns["details"]["regions_not_analyzed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["file"].as_str().unwrap(),
                e["range"].as_str().unwrap(),
                e["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(
        listed.contains(&("Unclosed.vue", "1:1-1:25", "malformed region")),
        "{listed:?}"
    );
    assert!(
        listed.contains(&("Unclosed.astro", "1:1-1:4", "malformed region")),
        "{listed:?}"
    );
    assert!(listed.iter().all(|e| e.0 != "BadTs.vue"), "{listed:?}");
}

// ---- analyzers ----

const CHECK: [&str; 5] = [
    "function check(a: number, b: number, c: number, d: number, e: number, f: number): number {",
    "  const s = a * b - c * d",
    "  const t = s * 2",
    "  return t + e + f",
    "}",
];
const LEGACY: &str = "<script lang=\"coffee\">\nx = 1\n</script>\n";

fn lines(lines: &[&str]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// The analyzer workspace: `check` in a TypeScript file and in a script
/// region of each framework, and a Vue file whose script is CoffeeScript.
fn analyzer_workspace() -> tempfile::TempDir {
    let util = format!("export {}", lines(&CHECK));
    let indented: Vec<String> = CHECK.iter().map(|l| format!("  {l}")).collect();
    let indented: Vec<&str> = indented.iter().map(String::as_str).collect();
    let mut vue = vec![
        "<template>",
        "  <p>{{ a && b && c && d && e && f && g && h && i && j && k && l }}</p>",
        "</template>",
        "<script setup lang=\"ts\">",
        "const a = 1",
        "if (a) {} else if (a) {} else if (a) {} else if (a) {} else if (a) {} else if (a) {}",
    ];
    vue.extend(CHECK);
    vue.push("</script>");
    let mut svelte = vec!["<script lang=\"ts\">"];
    svelte.extend(indented);
    svelte.extend(["</script>", "<p>{check(1, 2, 3, 4, 5, 6)}</p>"]);
    let mut astro = vec!["---"];
    astro.extend(CHECK);
    astro.extend(["---", "<p>{check(1, 2, 3, 4, 5, 6)}</p>"]);
    workspace(&[
        ("util.ts", &util),
        ("Big.vue", &lines(&vue)),
        ("Big.svelte", &lines(&svelte)),
        ("Big.astro", &lines(&astro)),
        ("Legacy.vue", LEGACY),
    ])
}

#[test]
fn each_analyzer_gives_the_command_line_result() {
    let ws = analyzer_workspace();
    let mut mcp = serve(ws.path());
    for (tool, analyzer) in [
        ("analyze_patterns", "patterns"),
        ("analyze_hotspots", "hotspots"),
        ("analyze_clones", "clones"),
    ] {
        let (result, _) = first_block(&mut mcp, tool, json!({}));
        assert_eq!(result, cli(ws.path(), &["analyze", analyzer]), "{tool}");
        assert_ne!(result["total"], 0, "{tool}: {result}");
    }
    let (patterns, _) = first_block(&mut mcp, "analyze_patterns", json!({}));
    assert_eq!(patterns["total"], 4);
    assert_eq!(
        patterns["details"]["regions_not_analyzed"],
        json!([{"file": "Legacy.vue", "range": "1:1-1:23", "reason": "unsupported script language: coffee"}])
    );
}

#[test]
fn analyze_patterns_limit_one_keeps_the_total() {
    let ws = analyzer_workspace();
    let mut mcp = serve(ws.path());
    let (result, _) = first_block(&mut mcp, "analyze_patterns", json!({"limit": 1}));
    assert_eq!(result["findings"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(result["total"], 4);
    assert_eq!(result["truncated"], true);
    assert_eq!(
        result,
        cli(ws.path(), &["analyze", "patterns", "--limit", "1"])
    );
}

#[test]
fn regions_not_analyzed_lists_30_of_31() {
    let files: Vec<(String, &str)> = (0..31).map(|i| (format!("C{i:02}.vue"), LEGACY)).collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), *c)).collect();
    let ws = workspace(&files);
    let mut mcp = serve(ws.path());
    let (result, _) = first_block(&mut mcp, "analyze_patterns", json!({}));
    assert_eq!(
        result["details"]["regions_not_analyzed"]
            .as_array()
            .unwrap()
            .len(),
        30,
        "{result}"
    );
    assert_eq!(result["details"]["regions_not_analyzed_total"], 31);
    assert_eq!(result, cli(ws.path(), &["analyze", "patterns"]));
}

// ---- references ----

#[test]
fn references_from_a_script_region_give_the_command_line_sites() {
    let ws = fixture_workspace();
    let mut mcp = serve(ws.path());
    for (name, line, column, expected) in [
        ("Counter.vue", 9, 7, vec![9, 11, 12]),
        ("Counter.svelte", 6, 7, vec![6, 7, 9, 10]),
    ] {
        let from_cli = cli(
            ws.path(),
            &[
                "query",
                "references",
                "count",
                "--file",
                name,
                "--line",
                &line.to_string(),
                "--column",
                &column.to_string(),
            ],
        );
        assert_eq!(from_cli["source"], "tree-sitter");
        let cli_sites: Vec<(String, u64)> = from_cli["references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    file_name(r["file"].as_str().unwrap()),
                    r["line"].as_u64().unwrap(),
                )
            })
            .collect();
        let (result, _) = first_block(
            &mut mcp,
            "references",
            json!({"symbol": "count", "file": name}),
        );
        let mcp_sites: Vec<(String, u64)> = result["references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    file_name(r["file"].as_str().unwrap()),
                    r["line"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(mcp_sites, cli_sites, "{name}");
        let expected: Vec<(String, u64)> =
            expected.iter().map(|l| (name.to_string(), *l)).collect();
        assert_eq!(mcp_sites, expected, "{name}");
        assert_eq!(
            result["resolved_by"],
            "tree-sitter workspace baseline (name-level; language servers are disabled)"
        );
    }
}

// ---- output limits ----

#[test]
fn symbols_of_many_vue_stop_at_50_with_the_note() {
    let mut text = String::from("<script setup lang=\"ts\">\n");
    for i in 0..60 {
        text.push_str(&format!("function f{i}() {{}}\n"));
    }
    text.push_str("</script>\n");
    let ws = workspace(&[("Many.vue", &text)]);
    let mut mcp = serve(ws.path());

    let (result, rest) = first_block(&mut mcp, "symbols", json!({"file": "Many.vue"}));
    assert_eq!(result["symbols"].as_array().unwrap().len(), 50);
    assert_eq!(
        rest,
        "\nNote: Returning 50 of 61 symbols. Use `pattern` (name regex) or `kind` (e.g. [\"function\",\"class\"]) to filter."
    );

    let (result, rest) = first_block(
        &mut mcp,
        "symbols",
        json!({"file": "Many.vue", "limit": 61}),
    );
    assert_eq!(result["symbols"].as_array().unwrap().len(), 61);
    assert_eq!(rest, "");
}
