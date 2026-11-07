//! Vue, Svelte and Astro component files through the built binary: the
//! source offsets of the committed fixtures, the equivalence of every region
//! with its view file, the other navigation tools, the output limits, and a
//! file of many regions.

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/components")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap()
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

/// Runs the built `act-lite --workspace <ws> <args>` from `ws`, with an empty
/// configuration directory and `ACT_LITE_DISABLE_LSP=1`.
fn run(ws: &Path, args: &[&str]) -> Output {
    let config = tempfile::tempdir().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .current_dir(ws)
        .env("ACT_CONFIG_DIR", config.path())
        .env("ACT_LITE_DISABLE_LSP", "1")
        .arg("--workspace")
        .arg(ws)
        .args(args)
        .output()
        .expect("the act-lite binary must start")
}

/// [`run`]: the parsed stdout on exit 0, else the error message on stderr.
fn outcome(ws: &Path, args: &[&str]) -> Result<Value, String> {
    let output = run(ws, args);
    if output.status.success() {
        return Ok(serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout of {args:?} is not JSON ({e}): {}",
                String::from_utf8_lossy(&output.stdout)
            )
        }));
    }
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let message = serde_json::from_str::<Value>(&stderr)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or(stderr);
    Err(message)
}

fn json(ws: &Path, args: &[&str]) -> Value {
    outcome(ws, args).unwrap_or_else(|e| panic!("act-lite {args:?} failed: {e}"))
}

fn span(range: &Value) -> String {
    format!(
        "{}:{}-{}:{}",
        range["start"]["line"],
        range["start"]["column"],
        range["end"]["line"],
        range["end"]["column"]
    )
}

/// `(kind, name, range, name_range)`; `name_range` is `-` when absent.
type Row = (String, String, String, String);

fn rows_of(declarations: &Value) -> Vec<Row> {
    declarations
        .as_array()
        .unwrap_or_else(|| panic!("no declarations: {declarations}"))
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                span(&d["range"]),
                if d["name_range"].is_null() {
                    "-".to_string()
                } else {
                    span(&d["name_range"])
                },
            )
        })
        .collect()
}

fn table(rows: &[Expected]) -> Vec<Row> {
    rows.iter()
        .map(|(k, n, r, nr)| (k.to_string(), n.to_string(), r.to_string(), nr.to_string()))
        .collect()
}

fn set<T: Ord + Clone>(items: &[T]) -> BTreeSet<T> {
    items.iter().cloned().collect()
}

fn blocks(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.0 == "module")
        .map(|r| r.1.clone())
        .collect()
}

/// A row of a table: `(kind, name, range, name_range)`.
type Expected = (&'static str, &'static str, &'static str, &'static str);

const COUNTER_VUE: &[Expected] = &[
    ("module", "<script lang=\"ts\">", "2:1-4:10", "2:1-2:19"),
    (
        "export",
        "export default { name: 'Counter' }",
        "3:1-3:35",
        "-",
    ),
    (
        "module",
        "<script setup lang=\"ts\">",
        "6:1-15:10",
        "6:1-6:25",
    ),
    ("import", "import { ref } from 'vue'", "7:1-7:26", "-"),
    ("variable", "props", "8:1-8:47", "8:7-8:12"),
    ("variable", "count", "9:1-9:31", "9:7-9:12"),
    ("function", "increment", "10:1-14:2", "10:10-10:19"),
    ("module", "<template>", "17:1-22:12", "17:1-17:11"),
    ("element", "div", "18:3-21:9", "18:4-18:7"),
    ("field", "app", "18:8-18:16", "18:12-18:15"),
    ("field", "counter", "18:17-18:36", "18:24-18:31"),
    ("field", "big", "18:17-18:36", "18:32-18:35"),
    ("element", "button", "19:5-19:45", "19:6-19:12"),
    ("element", "span", "20:5-20:46", "20:6-20:10"),
    ("module", "<style scoped>", "24:1-26:9", "24:1-24:15"),
    ("class", ".counter", "25:1-25:25", "25:1-25:9"),
    ("field", "color", "25:12-25:23", "25:12-25:17"),
];

const COUNTER_SVELTE: &[Expected] = &[
    (
        "module",
        "<script context=\"module\" lang=\"ts\">",
        "1:1-3:10",
        "1:1-1:36",
    ),
    ("export", "export const prerender = true;", "2:3-2:33", "-"),
    ("variable", "prerender", "2:10-2:33", "2:16-2:25"),
    ("element", "p", "5:1-5:15", "5:2-5:3"),
    ("module", "<script lang=\"ts\">", "5:15-13:10", "5:15-5:33"),
    ("export", "export let start: number = 0;", "5:33-5:62", "-"),
    ("variable", "start", "5:40-5:62", "5:44-5:49"),
    ("variable", "count", "6:3-6:21", "6:7-6:12"),
    ("function", "increment", "8:3-12:4", "8:12-8:21"),
    ("element", "div", "15:1-21:7", "15:2-15:5"),
    ("field", "counter", "15:6-15:21", "15:13-15:20"),
    ("element", "button", "16:3-16:51", "16:4-16:10"),
    ("element", "span", "18:5-18:35", "18:6-18:10"),
    ("function", "row", "20:3-20:48", "20:13-20:16"),
    ("element", "i", "20:28-20:38", "20:29-20:30"),
    ("module", "<style>", "23:1-25:9", "23:1-23:8"),
    ("class", ".counter", "24:3-24:27", "24:3-24:11"),
    ("field", "color", "24:14-24:25", "24:14-24:19"),
];

const CARD_ASTRO: &[Expected] = &[
    ("module", "---", "1:1-8:4", "1:1-1:4"),
    (
        "import",
        "import Layout from '../layouts/Layout.astro';",
        "2:1-2:46",
        "-",
    ),
    ("interface", "Props", "3:1-3:34", "3:11-3:16"),
    ("function", "shout", "5:1-7:2", "5:10-5:15"),
    ("element", "Layout", "9:1-11:10", "9:2-9:8"),
    ("element", "h1", "10:3-10:45", "10:4-10:6"),
    ("field", "top", "10:7-10:15", "10:11-10:14"),
    ("field", "t", "10:16-10:25", "10:23-10:24"),
    ("module", "<script>", "12:1-14:10", "12:1-12:9"),
    ("function", "hello", "13:3-13:49", "13:12-13:17"),
    ("module", "<script is:inline>", "15:1-15:43", "15:1-15:19"),
    ("variable", "inline", "15:19-15:34", "15:23-15:29"),
    (
        "module",
        "<script type=\"application/ld+json\">",
        "16:1-16:53",
        "16:1-16:36",
    ),
    ("module", "<style>", "17:1-19:9", "17:1-17:8"),
    ("class", "h1", "18:3-18:21", "18:3-18:5"),
    ("field", "color", "18:8-18:19", "18:8-18:13"),
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

/// The `(start, end)` byte offsets of the `range` and of the `name_range`
/// of the declaration of kind `kind` named `name`.
fn offsets(result: &Value, kind: &str, name: &str) -> ((u64, u64), Option<(u64, u64)>) {
    let d = result["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == kind && d["name"] == name)
        .unwrap_or_else(|| panic!("no {kind} {name}"));
    let pair = |r: &Value| {
        (
            r["start"]["byte_offset"].as_u64().unwrap(),
            r["end"]["byte_offset"].as_u64().unwrap(),
        )
    };
    (
        pair(&d["range"]),
        (!d["name_range"].is_null()).then(|| pair(&d["name_range"])),
    )
}

// ---- source offsets ----

#[test]
fn skeleton_of_each_fixture_is_its_table_in_block_order() {
    let ws = fixture_workspace();
    for (name, expected) in FIXTURES {
        let result = json(ws.path(), &["query", "skeleton", name]);
        let actual = rows_of(&result["declarations"]);
        let expected = table(expected);
        assert_eq!(set(&actual), set(&expected), "{name}: {actual:#?}");
        assert_eq!(actual.len(), expected.len(), "{name}");
        assert_eq!(blocks(&actual), blocks(&expected), "{name}");
        assert!(result.get("parse_error").is_none(), "{name}");
    }
}

#[test]
fn the_marked_byte_offsets_hold() {
    let ws = fixture_workspace();
    let vue = json(ws.path(), &["query", "skeleton", "Counter.vue"]);
    assert_eq!(offsets(&vue, "function", "increment").0, (224, 332));

    let svelte = json(ws.path(), &["query", "skeleton", "Counter.svelte"]);
    assert_eq!(
        offsets(&svelte, "variable", "prerender").1,
        Some((51, 60)),
        "the module script's prerender name"
    );
    assert_eq!(
        offsets(&svelte, "module", "<script lang=\"ts\">").0,
        (94, 300)
    );
    assert_eq!(
        offsets(&svelte, "export", "export let start: number = 0;").0,
        (112, 141)
    );
    assert_eq!(offsets(&svelte, "variable", "start").1, Some((123, 128)));

    let astro = json(ws.path(), &["query", "skeleton", "Card.astro"]);
    assert_eq!(offsets(&astro, "function", "shout").0, (115, 197));
}

// ---- the equivalence oracle ----

/// A region of a fixture: its content range and the extension of its
/// language.
struct Region {
    file: &'static str,
    start: usize,
    end: usize,
    ext: &'static str,
}

/// The content range of the element opened by `start_tag`, at its first
/// occurrence.
fn content(text: &str, start_tag: &str) -> (usize, usize) {
    let start = text.find(start_tag).unwrap() + start_tag.len();
    let close = if start_tag.starts_with("<style") {
        "</style>"
    } else {
        "</script>"
    };
    (start, start + text[start..].find(close).unwrap())
}

/// Every well-formed region of the three fixtures.
fn regions() -> Vec<Region> {
    let mut out = Vec::new();
    let mut add = |file: &'static str, (start, end): (usize, usize), ext: &'static str| {
        out.push(Region {
            file,
            start,
            end,
            ext,
        })
    };
    let vue = read_fixture("Counter.vue");
    add("Counter.vue", content(&vue, "<script lang=\"ts\">"), "ts");
    add(
        "Counter.vue",
        content(&vue, "<script setup lang=\"ts\">"),
        "ts",
    );
    add("Counter.vue", content(&vue, "<style scoped>"), "css");
    let svelte = read_fixture("Counter.svelte");
    add(
        "Counter.svelte",
        content(&svelte, "<script context=\"module\" lang=\"ts\">"),
        "ts",
    );
    add(
        "Counter.svelte",
        content(&svelte, "<script lang=\"ts\">"),
        "ts",
    );
    add("Counter.svelte", content(&svelte, "<style>"), "css");
    let astro = read_fixture("Card.astro");
    // The frontmatter's content: from the end of the opening fence to the
    // end of its last line of code.
    assert_eq!(&astro[..3], "---");
    assert!(astro[197..].starts_with("\n---\n"));
    add("Card.astro", (3, 197), "ts");
    add("Card.astro", content(&astro, "<script>"), "ts");
    add("Card.astro", content(&astro, "<script is:inline>"), "js");
    add("Card.astro", content(&astro, "<style>"), "css");
    out
}

/// `text` with every byte outside `[start, end)` except `\n` replaced by a
/// space.
fn view(text: &str, start: usize, end: usize) -> String {
    let bytes: Vec<u8> = text
        .bytes()
        .enumerate()
        .map(|(i, b)| {
            if (start..end).contains(&i) || b == b'\n' {
                b
            } else {
                b' '
            }
        })
        .collect();
    String::from_utf8(bytes).unwrap()
}

/// `value` with every `file` key removed, at any depth.
fn without_file(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(k, _)| k.as_str() != "file")
                .map(|(k, v)| (k.clone(), without_file(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_file).collect()),
        other => other.clone(),
    }
}

fn within(range: &Value, start: usize, end: usize) -> bool {
    let s = range["start"]["byte_offset"].as_u64().unwrap() as usize;
    let e = range["end"]["byte_offset"].as_u64().unwrap() as usize;
    start <= s && e <= end
}

/// The 1-based line and byte column of byte `at` of `text`.
fn position(text: &str, at: usize) -> (u64, u64) {
    let before = &text[..at];
    let line = before.matches('\n').count() as u64 + 1;
    let column = (at - before.rfind('\n').map_or(0, |i| i + 1)) as u64 + 1;
    (line, column)
}

/// Whether the analyzer range `l:c-l:c` lies within `[start, end)` of `text`.
fn range_within(range: &str, text: &str, start: usize, end: usize) -> bool {
    let point = |p: &str| {
        let (l, c) = p.split_once(':').unwrap();
        (l.parse::<u64>().unwrap(), c.parse::<u64>().unwrap())
    };
    let (a, b) = range.split_once('-').unwrap();
    position(text, start) <= point(a) && point(b) <= position(text, end)
}

/// `items` (each with a `file`) whose file name is `name`.
fn of_file<'a>(items: &'a Value, name: &str) -> Vec<&'a Value> {
    items
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| {
            Path::new(i["file"].as_str().unwrap())
                .file_name()
                .is_some_and(|f| f == name)
        })
        .collect()
}

/// A query outcome with `file` keys removed and each of `paths` in an error
/// message replaced by `<file>`.
fn normalized(outcome: Result<Value, String>, paths: &[String]) -> Result<Value, String> {
    match outcome {
        Ok(v) => Ok(without_file(&v)),
        Err(mut message) => {
            for p in paths {
                message = message.replace(p.as_str(), "<file>");
            }
            Err(message)
        }
    }
}

#[test]
fn every_region_equals_its_view_file() {
    for r in regions() {
        let text = read_fixture(r.file);
        let view_name = format!("view.{}", r.ext);
        let ws = workspace(&[(r.file, &text), (&view_name, &view(&text, r.start, r.end))]);
        let ws_path = ws.path();
        let what = format!("{} [{}, {})", r.file, r.start, r.end);

        // Declarations and symbols.
        let component = json(ws_path, &["query", "skeleton", r.file]);
        let plain = json(ws_path, &["query", "skeleton", &view_name]);
        let inside: Vec<Value> = component["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| within(&d["range"], r.start, r.end))
            .map(without_file)
            .collect();
        assert!(!inside.is_empty(), "{what}: no declaration");
        assert_eq!(
            Value::Array(inside.clone()),
            without_file(&plain["declarations"]),
            "{what}: skeleton"
        );
        let component_symbols = json(ws_path, &["query", "symbols", r.file]);
        let plain_symbols = json(ws_path, &["query", "symbols", &view_name]);
        let symbols_inside: Vec<Value> = component_symbols["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| within(&s["range"], r.start, r.end))
            .map(without_file)
            .collect();
        assert_eq!(
            Value::Array(symbols_inside),
            without_file(&plain_symbols["symbols"]),
            "{what}: symbols"
        );

        // Function analyses, for script regions.
        if r.ext != "css" {
            let patterns = json(ws_path, &["analyze", "patterns", "--limit", "1000"]);
            let in_r: Vec<Value> = of_file(&patterns["findings"], r.file)
                .into_iter()
                .filter(|f| range_within(f["range"].as_str().unwrap(), &text, r.start, r.end))
                .map(without_file)
                .collect();
            let of_view: Vec<Value> = of_file(&patterns["findings"], &view_name)
                .into_iter()
                .map(without_file)
                .collect();
            assert_eq!(in_r, of_view, "{what}: patterns");

            let hotspots = json(ws_path, &["analyze", "hotspots", "--limit", "1000"]);
            let top = |name: &str| -> Vec<Value> {
                of_file(&hotspots["findings"], name)
                    .into_iter()
                    .flat_map(|item| item["top_functions"].as_array().unwrap().clone())
                    .collect()
            };
            let in_r: Vec<Value> = top(r.file)
                .into_iter()
                .filter(|f| range_within(f["range"].as_str().unwrap(), &text, r.start, r.end))
                .collect();
            let of_view = top(&view_name);
            assert_eq!(in_r, of_view, "{what}: hotspots top_functions");
        }

        // Definition at each declaration's name, references inside r.
        let paths = [
            ws_path.join(r.file).display().to_string(),
            ws_path.join(&view_name).display().to_string(),
            r.file.to_string(),
            view_name.clone(),
        ];
        for d in &inside {
            let Some(name_range) = d.get("name_range") else {
                continue;
            };
            let name = d["name"].as_str().unwrap();
            let line = name_range["start"]["line"].to_string();
            let column = name_range["start"]["column"].to_string();
            let at = |file: &str, query: &str| {
                outcome(
                    ws_path,
                    &[
                        "query", query, name, "--file", file, "--line", &line, "--column", &column,
                    ],
                )
            };
            let definition = normalized(at(r.file, "definition"), &paths);
            if r.ext != "css" {
                assert!(
                    definition.is_ok(),
                    "{what}: definition of {name}: {definition:?}"
                );
            }
            assert_eq!(
                definition,
                normalized(at(&view_name, "definition"), &paths),
                "{what}: definition of {name} at {line}:{column}"
            );
            let references = |file: &str| -> Result<Vec<Value>, String> {
                at(file, "references").map(|v| {
                    of_file(&v["references"], file)
                        .into_iter()
                        .filter(|p| {
                            let b = p["byte_offset"].as_u64().unwrap() as usize;
                            file != r.file || (r.start..r.end).contains(&b)
                        })
                        .map(without_file)
                        .collect()
                })
            };
            let from_component = references(r.file);
            if r.ext != "css" {
                assert!(
                    from_component.as_ref().is_ok_and(|v| !v.is_empty()),
                    "{what}: references of {name}: {from_component:?}"
                );
            }
            let from_view = references(&view_name);
            let normalize = |o: Result<Vec<Value>, String>| normalized(o.map(Value::Array), &paths);
            assert_eq!(
                normalize(from_component),
                normalize(from_view),
                "{what}: references of {name} at {line}:{column}"
            );
        }
    }
}

// ---- the other navigation tools ----

#[test]
fn symbols_batch_and_repo_outline_list_the_skeleton() {
    let ws = fixture_workspace();
    let outline = json(ws.path(), &["query", "repo-outline", "--symbols"]);
    for (name, expected) in FIXTURES {
        let expected: Vec<(String, String)> = expected
            .iter()
            .map(|(k, n, _, _)| (n.to_string(), k.to_string()))
            .collect();

        let batch = json(ws.path(), &["query", "symbols-batch", "--files", name]);
        let listed: Vec<(String, String)> = batch["results"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().unwrap().to_string(),
                    s["kind"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(batch["results"][0]["file"], name);
        assert_eq!(set(&listed), set(&expected), "{name}: {listed:#?}");
        assert_eq!(listed.len(), expected.len(), "{name}");
        assert!(batch.get("parse_error").is_none(), "{name}");
        assert!(batch["results"][0].get("parse_error").is_none(), "{name}");

        let entry = outline["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == name)
            .unwrap_or_else(|| panic!("{name} not in {outline}"));
        let language = match Path::new(name).extension().unwrap().to_str().unwrap() {
            "vue" => "Vue",
            "svelte" => "Svelte",
            _ => "Astro",
        };
        assert_eq!(entry["language"], language, "{name}");
        let names: Vec<String> = entry["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        let expected_names: Vec<String> = expected.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(
            names.iter().cloned().collect::<BTreeSet<_>>(),
            expected_names.iter().cloned().collect::<BTreeSet<_>>(),
            "{name}"
        );
        assert_eq!(names.len(), expected_names.len(), "{name}");

        let symbols = json(ws.path(), &["query", "symbols", name]);
        assert!(symbols.get("parse_error").is_none(), "{name}");
    }
}

// ---- output limits ----

/// `Many.vue`: a `<script setup>` block with 60 top-level functions.
fn many_vue() -> String {
    let mut text = String::from("<script setup lang=\"ts\">\n");
    for i in 0..60 {
        text.push_str(&format!("function f{i}() {{}}\n"));
    }
    text.push_str("</script>\n");
    text
}

#[test]
fn many_vue_holds_61_symbols() {
    let ws = workspace(&[("Many.vue", &many_vue())]);
    let result = json(ws.path(), &["query", "symbols", "Many.vue"]);
    let symbols = result["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 61, "{result}");
    assert_eq!(symbols.iter().filter(|s| s["kind"] == "module").count(), 1);
    assert_eq!(
        symbols.iter().filter(|s| s["kind"] == "function").count(),
        60
    );
}

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
fn patterns_limit_one_keeps_the_total() {
    let ws = analyzer_workspace();
    let result = json(ws.path(), &["analyze", "patterns", "--limit", "1"]);
    assert_eq!(result["findings"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(result["total"], 4);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["findings"][0]["file"], "Big.astro");
}

#[test]
fn regions_not_analyzed_lists_30_of_31() {
    let files: Vec<(String, &str)> = (0..31).map(|i| (format!("C{i:02}.vue"), LEGACY)).collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), *c)).collect();
    let ws = workspace(&files);
    let result = json(ws.path(), &["analyze", "patterns"]);
    let entries = result["details"]["regions_not_analyzed"]
        .as_array()
        .unwrap();
    assert_eq!(entries.len(), 30, "{result}");
    assert_eq!(result["details"]["regions_not_analyzed_total"], 31);
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry["file"], format!("C{i:02}.vue"));
        assert_eq!(entry["range"], "1:1-1:23");
        assert_eq!(entry["reason"], "unsupported script language: coffee");
    }
}

// ---- many regions ----

#[test]
fn many_regions_are_fast() {
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: set ACT_LITE_CHECK_PERF=1 to time Many.astro (8,000 scripts)");
        return;
    }
    let text: String = (0..8000)
        .map(|i| format!("<script>function f{i}() {{}}</script>\n"))
        .collect();
    let ws = workspace(&[("Many.astro", &text)]);

    // One warm-up run, then the timed run.
    json(ws.path(), &["query", "skeleton", "Many.astro"]);
    let started = Instant::now();
    let result = json(ws.path(), &["query", "skeleton", "Many.astro"]);
    let elapsed = started.elapsed();
    let declarations = result["declarations"].as_array().unwrap();
    assert_eq!(declarations.len(), 16_000);
    assert_eq!(
        declarations
            .iter()
            .filter(|d| d["kind"] == "module")
            .count(),
        8000
    );
    assert_eq!(
        declarations
            .iter()
            .filter(|d| d["kind"] == "function")
            .count(),
        8000
    );
    eprintln!("query skeleton Many.astro: {elapsed:?}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");

    let patterns = json(ws.path(), &["analyze", "patterns"]);
    assert_eq!(
        patterns["details"]["functions"], 8000,
        "{}",
        patterns["details"]
    );
}
