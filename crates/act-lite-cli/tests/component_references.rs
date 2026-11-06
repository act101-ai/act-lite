//! `references` and `definition` in Vue, Svelte and Astro component files,
//! through the built binary and `act-lite mcp serve`: a position in a script
//! region searches the regions of that language in the component files of
//! the same framework and the plain files of that language; a position in
//! the outer document searches the outer structure of those component
//! files; a position in a template expression is not a symbol.

#[allow(dead_code)] // not every helper of the shared module is used here
mod langs_added;

use langs_added::Mcp;
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

const DISABLED: &[(&str, &str)] = &[("ACT_LITE_DISABLE_LSP", "1")];

/// A temporary workspace with each committed component fixture of `names`.
fn fixtures(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/components");
    for name in names {
        std::fs::copy(from.join(name), dir.path().join(name)).unwrap();
    }
    dir
}

/// A temporary workspace holding each `(path, content)` of `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        std::fs::write(dir.path().join(rel), content).unwrap();
    }
    dir
}

/// The reference workspace: one TypeScript file and two Vue components that
/// import from it.
fn reference_workspace() -> tempfile::TempDir {
    workspace(&[
        (
            "fmt.ts",
            "export function fmt(n: number): string {\n  return String(n)\n}\n",
        ),
        (
            "Uses.vue",
            "<script setup lang=\"ts\">\nimport { fmt } from './fmt'\nconst label = fmt(1)\n</script>\n<template><p>{{ label }}</p></template>\n",
        ),
        (
            "Other.vue",
            "<script setup lang=\"ts\">\nimport { fmt } from './fmt'\nconst x = fmt(2)\n</script>\n",
        ),
    ])
}

/// Runs the built `act-lite --workspace <ws> <args>`, with an empty
/// configuration directory and, when `disable_lsp`, `ACT_LITE_DISABLE_LSP=1`.
fn run(ws: &Path, args: &[&str], disable_lsp: bool) -> Output {
    let config = tempfile::tempdir().unwrap();
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("act-lite"));
    command
        .current_dir(ws)
        .env("ACT_CONFIG_DIR", config.path())
        .env_remove("ACT_LITE_DISABLE_LSP")
        .arg("--workspace")
        .arg(ws)
        .args(args);
    if disable_lsp {
        command.envs(DISABLED.iter().copied());
    }
    command.output().expect("the act-lite binary must start")
}

/// [`run`] with language servers disabled; asserts exit 0 and parses stdout.
fn json_of(ws: &Path, args: &[&str]) -> Value {
    let output = run(ws, args, true);
    assert!(
        output.status.success(),
        "act-lite {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Asserts exit 1 and returns stdout and stderr together.
fn failure(output: &Output) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(1), "{text}");
    text
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn references(ws: &Path, name: &str, file: &str, line: u32, column: u32) -> Value {
    let (line, column) = (line.to_string(), column.to_string());
    json_of(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            file,
            "--line",
            &line,
            "--column",
            &column,
        ],
    )
}

fn references_output(ws: &Path, name: &str, file: &str, line: u32, column: u32) -> Output {
    let (line, column) = (line.to_string(), column.to_string());
    run(
        ws,
        &[
            "query",
            "references",
            name,
            "--file",
            file,
            "--line",
            &line,
            "--column",
            &column,
        ],
        true,
    )
}

fn definition_output(ws: &Path, name: &str, file: &str, line: u32, column: u32) -> Output {
    let (line, column) = (line.to_string(), column.to_string());
    run(
        ws,
        &[
            "query",
            "definition",
            name,
            "--file",
            file,
            "--line",
            &line,
            "--column",
            &column,
        ],
        true,
    )
}

/// `(file name, line, column)` of each reference, in output order.
fn sites(answer: &Value) -> Vec<(String, u64, u64)> {
    answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| {
            (
                file_name(r["file"].as_str().unwrap()),
                r["line"].as_u64().unwrap(),
                r["column"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn at(file: &str, line: u64, column: u64) -> (String, u64, u64) {
    (file.to_string(), line, column)
}

/// `file start_line:start_column-end_line:end_column` of a definition answer.
fn definition_span(ws: &Path, name: &str, file: &str, line: u32, column: u32) -> String {
    let output = definition_output(ws, name, file, line, column);
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "{text}{}", {
        String::from_utf8_lossy(&output.stderr)
    });
    let answer: Value = serde_json::from_str(&text).unwrap();
    let d = &answer["definition"];
    format!(
        "{} {}:{}-{}:{}",
        file_name(d["file"].as_str().unwrap()),
        d["line"],
        d["column"],
        d["end_line"],
        d["end_column"]
    )
}

/// The MCP payload: the first content block, parsed.
fn payload(result: &Value) -> Value {
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

/// `(file name, line)` of each site of an MCP `references` payload.
fn mcp_sites(answer: &Value) -> Vec<(String, u64)> {
    answer["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no references in {answer}"))
        .iter()
        .map(|r| {
            (
                file_name(r["file"].as_str().unwrap()),
                r["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

const NAME_LEVEL: &str =
    "tree-sitter workspace baseline (name-level; language servers are disabled)";

#[test]
fn a_position_in_a_template_expression_is_not_a_symbol_on_the_cli() {
    for (file, line, column) in [("Counter.svelte", 18, 12), ("Counter.vue", 20, 31)] {
        let ws = fixtures(&[file]);
        let text = failure(&references_output(ws.path(), "count", file, line, column));
        assert!(text.contains("symbol not found: count in "), "{text}");
        let text = failure(&definition_output(ws.path(), "count", file, line, column));
        assert!(text.contains("No symbol found at "), "{text}");
        assert!(text.contains(&format!("{file}:{line}:{column}")), "{text}");
    }
}

#[test]
fn a_name_used_only_in_a_template_expression_is_not_found_on_mcp() {
    let ws = workspace(&[("Expr.svelte", "<p>{onlyhere}</p>\n")]);
    let message = Mcp::start_with_env(ws.path(), DISABLED)
        .call(
            "references",
            json!({ "symbol": "onlyhere", "file": "Expr.svelte" }),
        )
        .unwrap_err();
    assert!(
        message.contains("Symbol 'onlyhere' not found in Expr.svelte"),
        "{message}"
    );
}

#[test]
fn an_outer_document_id_is_its_own_reference_and_definition() {
    let ws = fixtures(&["Counter.vue"]);
    let answer = references(ws.path(), "app", "Counter.vue", 18, 12);
    assert_eq!(sites(&answer), vec![at("Counter.vue", 18, 12)], "{answer}");
    assert_eq!(
        definition_span(ws.path(), "app", "Counter.vue", 18, 12),
        "Counter.vue 18:12-18:15"
    );
    let ws = fixtures(&["Card.astro"]);
    let answer = references(ws.path(), "top", "Card.astro", 10, 11);
    assert_eq!(sites(&answer), vec![at("Card.astro", 10, 11)], "{answer}");
}

#[test]
fn script_references_do_not_list_template_uses() {
    let ws = fixtures(&["Counter.vue"]);
    let answer = references(ws.path(), "count", "Counter.vue", 9, 7);
    assert_eq!(
        sites(&answer),
        vec![
            at("Counter.vue", 9, 7),
            at("Counter.vue", 11, 19),
            at("Counter.vue", 12, 5)
        ],
        "{answer}"
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let result = Mcp::start_with_env(ws.path(), DISABLED)
        .call(
            "references",
            json!({ "symbol": "count", "file": "Counter.vue" }),
        )
        .unwrap();
    let mcp = payload(&result);
    assert_eq!(mcp["resolved_by"], NAME_LEVEL, "{mcp}");
    let lines: Vec<(String, u64)> = sites(&answer).into_iter().map(|(f, l, _)| (f, l)).collect();
    assert_eq!(mcp_sites(&mcp), lines, "{mcp}");

    let ws = fixtures(&["Counter.svelte"]);
    let answer = references(ws.path(), "count", "Counter.svelte", 6, 7);
    assert_eq!(
        sites(&answer),
        vec![
            at("Counter.svelte", 6, 7),
            at("Counter.svelte", 7, 16),
            at("Counter.svelte", 9, 21),
            at("Counter.svelte", 10, 7)
        ],
        "{answer}"
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let result = Mcp::start_with_env(ws.path(), DISABLED)
        .call(
            "references",
            json!({ "symbol": "count", "file": "Counter.svelte" }),
        )
        .unwrap();
    let mcp = payload(&result);
    assert_eq!(mcp["resolved_by"], NAME_LEVEL, "{mcp}");
    let lines: Vec<(String, u64)> = sites(&answer).into_iter().map(|(f, l, _)| (f, l)).collect();
    assert_eq!(mcp_sites(&mcp), lines, "{mcp}");
}

#[test]
fn a_region_name_matches_regions_of_its_framework_and_plain_files_of_its_language() {
    let ws = reference_workspace();
    let answer = references(ws.path(), "fmt", "Uses.vue", 3, 15);
    assert_eq!(
        sites(&answer),
        vec![
            at("Other.vue", 2, 10),
            at("Other.vue", 3, 11),
            at("Uses.vue", 2, 10),
            at("Uses.vue", 3, 15),
            at("fmt.ts", 1, 17)
        ],
        "{answer}"
    );
    assert_eq!(
        definition_span(ws.path(), "fmt", "Uses.vue", 3, 15),
        "fmt.ts 1:17-1:20"
    );
    let answer = references(ws.path(), "fmt", "fmt.ts", 1, 17);
    assert_eq!(sites(&answer), vec![at("fmt.ts", 1, 17)], "{answer}");
}

#[test]
fn callers_callees_and_implementations_need_a_language_server() {
    let ws = reference_workspace();
    for query in ["callers", "callees", "implementations"] {
        let text = failure(&run(
            ws.path(),
            &["query", query, "fmt", "--file", "Uses.vue"],
            false,
        ));
        assert!(
            text.contains(&format!(
                "query type '{query}' needs a language server, and act-lite has none configured for Vue files"
            )),
            "{text}"
        );
    }
    for (file, content, entry) in [
        (
            "Counter.svelte",
            "<script lang=\"ts\">let count = 1;</script>\n",
            "Svelte",
        ),
        ("Card.astro", "---\nconst count = 1;\n---\n", "Astro"),
    ] {
        let ws = workspace(&[(file, content)]);
        for query in ["callees", "implementations"] {
            let text = failure(&run(
                ws.path(),
                &["query", query, "count", "--file", file],
                false,
            ));
            assert!(
                text.contains(&format!("none configured for {entry} files")),
                "{text}"
            );
        }
    }
    let message = Mcp::start(ws.path())
        .call("callers", json!({ "symbol": "fmt", "file": "Uses.vue" }))
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for Vue files"
        ),
        "{message}"
    );
}

#[test]
fn the_mcp_anchor_counts_columns_in_bytes_after_a_multibyte_character() {
    let ws = workspace(&[(
        "Wide.vue",
        "<script setup lang=\"ts\">\nconst s = 'ä'; use(s)\n</script>\n",
    )]);
    let result = Mcp::start_with_env(ws.path(), DISABLED)
        .call("references", json!({ "symbol": "use", "file": "Wide.vue" }))
        .unwrap_or_else(|e| panic!("{e}"));
    let mcp = payload(&result);
    assert_eq!(mcp_sites(&mcp), vec![("Wide.vue".to_string(), 2)], "{mcp}");
}

#[test]
fn symbols_batch_ids_find_region_symbols_with_the_merged_parse_error() {
    let ws = fixtures(&["Counter.vue"]);
    let bad = "<template><p/></template>\n<script lang=\"ts\">\nfunction f(a: number {\n  return a\n}\nfunction g(b: number) { return b }\n</script>\n";
    std::fs::write(ws.path().join("BadTs.vue"), bad).unwrap();
    let id_of = |file: &str, name: &str| -> String {
        let listed = json_of(ws.path(), &["query", "symbols-batch", "--files", file]);
        listed["results"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == name)
            .unwrap_or_else(|| panic!("no {name} in {listed}"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for (file, name, parse_error) in [
        ("Counter.vue", "increment", false),
        ("BadTs.vue", "g", true),
    ] {
        let id = id_of(file, name);
        let found = json_of(ws.path(), &["query", "symbols-batch", "--ids", &id]);
        let group = &found["results"][0];
        assert_eq!(group["symbols"][0]["name"], name, "{found}");
        assert_eq!(group["symbols"][0]["kind"], "function", "{found}");
        assert_eq!(group.get("parse_error").is_some(), parse_error, "{found}");
    }
}

#[test]
fn outer_references_skip_custom_blocks_other_templates_and_error_content() {
    let ws = workspace(&[
        (
            "A.vue",
            "<template>\n  <p id=\"x\">a</p>\n</template>\n<i18n><p id=\"x\">b</p></i18n>\n<template lang=\"pug\"><p id=\"x\">c</p></template>\n",
        ),
        (
            "B.vue",
            "<docs><p id=\"x\">d</p></docs>\n<template><i id=\"x\"/></template>\n",
        ),
    ]);
    let answer = references(ws.path(), "x", "A.vue", 2, 10);
    assert_eq!(
        sites(&answer),
        vec![at("A.vue", 2, 10), at("B.vue", 2, 18)],
        "{answer}"
    );
    let text = failure(&references_output(ws.path(), "x", "A.vue", 4, 14));
    assert!(text.contains("symbol not found: x in "), "{text}");

    let ws = workspace(&[(
        "U.astro",
        "<p id=\"x\">a</p>\n<div><p id=\"x\">b</p>{</div>\n",
    )]);
    let answer = references(ws.path(), "x", "U.astro", 1, 8);
    assert_eq!(sites(&answer), vec![at("U.astro", 1, 8)], "{answer}");
}

/// An Astro file with `n` script regions and `n` template expressions that
/// use `zz`, a name no region declares.
fn many_expressions(n: usize) -> String {
    let mut text = String::new();
    for i in 0..n {
        text.push_str(&format!("<script>let v{i} = 1;</script>\n<p>{{zz}}</p>\n"));
    }
    text
}

#[test]
fn mcp_references_to_a_template_only_name_say_not_found() {
    let ws = workspace(&[("Many.astro", &many_expressions(50))]);
    let message = Mcp::start_with_env(ws.path(), DISABLED)
        .call(
            "references",
            json!({ "symbol": "zz", "file": "Many.astro" }),
        )
        .unwrap_err();
    assert!(
        message.contains("Symbol 'zz' not found in Many.astro"),
        "{message}"
    );
}

#[test]
fn mcp_references_to_a_template_only_name_stay_linear_in_the_regions() {
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: set ACT_LITE_CHECK_PERF=1 to time 8,000 regions and expressions");
        return;
    }
    let ws = workspace(&[("Many.astro", &many_expressions(8_000))]);
    let mut mcp = Mcp::start_with_env(ws.path(), DISABLED);
    let args = json!({ "symbol": "zz", "file": "Many.astro" });
    let _ = mcp.call("references", args.clone());
    let started = std::time::Instant::now();
    let message = mcp.call("references", args).unwrap_err();
    let elapsed = started.elapsed();
    assert!(message.contains("not found"), "{message}");
    assert!(elapsed.as_secs_f64() < 2.0, "{elapsed:?}");
}

/// The lines of the references MCP gives for `symbol` in `file`.
fn mcp_lines(ws: &Path, symbol: &str, file: &str) -> Vec<u64> {
    let result = Mcp::start_with_env(ws, DISABLED)
        .call("references", json!({ "symbol": symbol, "file": file }))
        .unwrap();
    let text = result.to_string();
    let mut lines: Vec<u64> =
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap_or(&text))
            .ok()
            .and_then(|v| v["references"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|r| r["line"].as_u64().or(r["range"]["start"]["line"].as_u64()))
            .collect();
    lines.sort_unstable();
    lines
}

#[test]
fn mcp_prefers_a_script_use_to_template_text_or_a_custom_block() {
    for (name, text) in [
        (
            "Text.vue",
            "<template><p>foo</p></template>\n<script>foo()</script>\n",
        ),
        ("Docs.vue", "<docs>foo</docs>\n<script>foo()</script>\n"),
    ] {
        let ws = workspace(&[(name, text)]);
        let cli = references(ws.path(), "foo", name, 2, 9);
        let cli_lines: Vec<u64> = cli["references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["line"].as_u64().unwrap())
            .collect();
        assert_eq!(cli_lines, vec![2], "{name}: {cli}");
        assert_eq!(mcp_lines(ws.path(), "foo", name), vec![2], "{name}");
    }
}
