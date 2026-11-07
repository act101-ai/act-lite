//! Vue single-file components through the built binary: `query skeleton`
//! and `query symbols` report the outer structure and the declarations of
//! the script and style regions, every location in the component file.

#[allow(dead_code)] // the MCP helpers of the shared module are not used here
mod langs_added;

use langs_added::cli_json;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The design text of `Counter.vue` (line 1 holds two `ä`, 2 bytes each).
const COUNTER: &str = "<!-- Zähler: ein Zähler -->
<script lang=\"ts\">
export default { name: 'Counter' }
</script>

<script setup lang=\"ts\">
import { ref } from 'vue'
const props = defineProps<{ start: number }>()
const count = ref(props.start)
function increment(step: number): void {
  if (step > 0 && count.value < 10) {
    count.value += step
  }
}
</script>

<template>
  <div id=\"app\" class=\"counter big\">
    <button @click=\"increment(1)\">+</button>
    <span v-if=\"count > 5\">{{ count }}</span>
  </div>
</template>

<style scoped>
.counter { color: red; }
</style>
";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/components/Counter.vue")
}

/// A temporary workspace holding each `(path, content)` of `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        std::fs::write(dir.path().join(rel), content).unwrap();
    }
    dir
}

fn counter_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture(), dir.path().join("Counter.vue")).unwrap();
    dir
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

fn row(kind: &str, name: &str, range: &str, name_range: &str) -> Row {
    (
        kind.to_string(),
        name.to_string(),
        range.to_string(),
        name_range.to_string(),
    )
}

fn skeleton(ws: &Path, file: &str) -> (Vec<Row>, Value) {
    let result = cli_json(ws, &["query", "skeleton", file]);
    let rows = result["declarations"]
        .as_array()
        .unwrap_or_else(|| panic!("no declarations: {result}"))
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
        .collect();
    (rows, result)
}

/// `(kind, name, range)`, and whether `parse_error` is `true`.
fn symbols(ws: &Path, file: &str) -> (Vec<(String, String, String)>, bool) {
    let result = cli_json(ws, &["query", "symbols", file]);
    let rows = result["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("no symbols: {result}"))
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
                span(&s["range"]),
            )
        })
        .collect();
    let parse_error = match result.get("parse_error") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(other) => panic!("parse_error is {other}"),
    };
    (rows, parse_error)
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

/// The content range of the first element opened by `start_tag`.
fn content(text: &str, start_tag: &str, end_tag: &str) -> (usize, usize) {
    let start = text.find(start_tag).unwrap() + start_tag.len();
    (start, start + text[start..].find(end_tag).unwrap())
}

#[test]
fn the_fixture_holds_the_design_bytes() {
    assert_eq!(std::fs::read_to_string(fixture()).unwrap(), COUNTER);
}

#[test]
fn skeleton_of_counter_vue_is_the_table_with_the_file_offsets() {
    let ws = counter_workspace();
    let (actual, result) = skeleton(ws.path(), "Counter.vue");
    let expected = [
        row("module", "<script lang=\"ts\">", "2:1-4:10", "2:1-2:19"),
        row(
            "export",
            "export default { name: 'Counter' }",
            "3:1-3:35",
            "-",
        ),
        row(
            "module",
            "<script setup lang=\"ts\">",
            "6:1-15:10",
            "6:1-6:25",
        ),
        row("import", "import { ref } from 'vue'", "7:1-7:26", "-"),
        row("variable", "props", "8:1-8:47", "8:7-8:12"),
        row("variable", "count", "9:1-9:31", "9:7-9:12"),
        row("function", "increment", "10:1-14:2", "10:10-10:19"),
        row("module", "<template>", "17:1-22:12", "17:1-17:11"),
        row("element", "div", "18:3-21:9", "18:4-18:7"),
        row("field", "app", "18:8-18:16", "18:12-18:15"),
        row("field", "counter", "18:17-18:36", "18:24-18:31"),
        row("field", "big", "18:17-18:36", "18:32-18:35"),
        row("element", "button", "19:5-19:45", "19:6-19:12"),
        row("element", "span", "20:5-20:46", "20:6-20:10"),
        row("module", "<style scoped>", "24:1-26:9", "24:1-24:15"),
        row("class", ".counter", "25:1-25:25", "25:1-25:9"),
        row("field", "color", "25:12-25:23", "25:12-25:17"),
    ];
    assert_eq!(set(&actual), set(&expected), "{actual:#?}");
    assert_eq!(actual.len(), expected.len());
    assert_eq!(blocks(&actual), blocks(&expected));

    let increment = result["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "increment")
        .unwrap();
    assert_eq!(increment["range"]["start"]["byte_offset"], 224);
    assert_eq!(increment["range"]["end"]["byte_offset"], 332);
    assert!(result.get("parse_error").is_none());
}

#[test]
fn symbols_of_counter_vue_are_the_blocks_the_template_and_the_regions() {
    let ws = counter_workspace();
    let (actual, parse_error) = symbols(ws.path(), "Counter.vue");
    let expected: Vec<(String, String, String)> = [
        ("module", "<script lang=\"ts\">", "2:1-2:19"),
        ("unknown", "name", "3:18-3:22"),
        ("module", "<script setup lang=\"ts\">", "6:1-6:25"),
        ("variable", "props", "8:7-8:12"),
        ("field", "start", "8:29-8:34"),
        ("variable", "count", "9:7-9:12"),
        ("function", "increment", "10:10-10:19"),
        ("module", "<template>", "17:1-17:11"),
        ("class", "div", "18:4-18:7"),
        ("field", "app", "18:12-18:15"),
        ("field", "counter", "18:24-18:31"),
        ("field", "big", "18:32-18:35"),
        ("class", "button", "19:6-19:12"),
        ("class", "span", "20:6-20:10"),
        ("module", "<style scoped>", "24:1-24:15"),
        ("unknown", ".counter", "25:1-25:9"),
        ("field", "color", "25:12-25:17"),
    ]
    .iter()
    .map(|(k, n, r)| (k.to_string(), n.to_string(), r.to_string()))
    .collect();
    assert_eq!(set(&actual), set(&expected), "{actual:#?}");
    assert!(!parse_error);
}

#[test]
fn an_error_inside_the_scoped_style_gives_parse_error() {
    let broken = COUNTER.replace(".counter { color: red; }", ".counter { color: red; } } {");
    let ws = workspace(&[("Counter.vue", &broken)]);
    let (_, parse_error) = symbols(ws.path(), "Counter.vue");
    assert!(parse_error);
}

#[test]
fn the_language_selection_fixtures_report_what_their_lang_selects() {
    let langs = "<template lang=\"pug\">\ndiv\n  p {{ a }}\n</template>\n<script lang=\"coffee\">\nx = 1\n</script>\n<i18n lang=\"json\">{\"a\":1}</i18n>\n";
    let tsx =
        "<script setup lang=\"tsx\">\nconst view = () => <div class=\"x\">hi</div>\n</script>\n";
    let nest = "<template>\n  <template v-if=\"a\"><p>x</p></template>\n</template>\n";
    let src = "<script src=\"./a.ts\"></script>\n<template><p/></template>\n";
    let ws = workspace(&[
        ("Langs.vue", langs),
        ("Tsx.vue", tsx),
        ("NestT.vue", nest),
        ("Src.vue", src),
    ]);
    let cases: [(&str, Vec<Row>); 4] = [
        (
            "Langs.vue",
            vec![
                row("module", "<template lang=\"pug\">", "1:1-4:12", "1:1-1:22"),
                row("module", "<script lang=\"coffee\">", "5:1-7:10", "5:1-5:23"),
                row("module", "<i18n lang=\"json\">", "8:1-8:33", "8:1-8:19"),
            ],
        ),
        (
            "Tsx.vue",
            vec![
                row(
                    "module",
                    "<script setup lang=\"tsx\">",
                    "1:1-3:10",
                    "1:1-1:26",
                ),
                row("variable", "view", "2:1-2:43", "2:7-2:11"),
            ],
        ),
        (
            "NestT.vue",
            vec![
                row("module", "<template>", "1:1-3:12", "1:1-1:11"),
                row("element", "template", "2:3-2:41", "2:4-2:12"),
                row("element", "p", "2:22-2:30", "2:23-2:24"),
            ],
        ),
        (
            "Src.vue",
            vec![
                row("module", "<script src=\"./a.ts\">", "1:1-1:31", "1:1-1:22"),
                row("module", "<template>", "2:1-2:26", "2:1-2:11"),
                row("element", "p", "2:11-2:15", "2:12-2:13"),
            ],
        ),
    ];
    for (file, expected) in cases {
        let (actual, _) = skeleton(ws.path(), file);
        assert_eq!(actual, expected, "{file}");
        let (_, parse_error) = symbols(ws.path(), file);
        assert!(!parse_error, "{file}");
    }

    // The TSX region's symbols are those of its view file.
    let (start, end) = content(tsx, "<script setup lang=\"tsx\">", "</script>");
    std::fs::write(ws.path().join("view.tsx"), view(tsx, start, end)).unwrap();
    let (mut expected, _) = symbols(ws.path(), "view.tsx");
    expected.push((
        "module".to_string(),
        "<script setup lang=\"tsx\">".to_string(),
        "1:1-1:26".to_string(),
    ));
    let (actual, _) = symbols(ws.path(), "Tsx.vue");
    assert_eq!(set(&actual), set(&expected));
}

#[test]
fn two_script_setup_blocks_are_both_reported() {
    let text = "<script setup lang=\"ts\">\nconst a = 1\n</script>\n<script setup lang=\"ts\">\nconst b = 2\n</script>\n";
    let ws = workspace(&[("Two.vue", text)]);
    let (actual, _) = skeleton(ws.path(), "Two.vue");
    assert_eq!(
        actual,
        [
            row(
                "module",
                "<script setup lang=\"ts\">",
                "1:1-3:10",
                "1:1-1:25"
            ),
            row("variable", "a", "2:1-2:12", "2:7-2:8"),
            row(
                "module",
                "<script setup lang=\"ts\">",
                "4:1-6:10",
                "4:1-4:25"
            ),
            row("variable", "b", "5:1-5:12", "5:7-5:8"),
        ]
    );
    let (symbols, parse_error) = symbols(ws.path(), "Two.vue");
    assert!(!parse_error);
    let variables: Vec<_> = symbols.iter().filter(|s| s.0 == "variable").collect();
    assert_eq!(
        variables,
        [
            &(
                "variable".to_string(),
                "a".to_string(),
                "2:7-2:8".to_string()
            ),
            &(
                "variable".to_string(),
                "b".to_string(),
                "5:7-5:8".to_string()
            ),
        ]
    );
}

#[test]
fn the_malformed_region_fixtures_report_their_parse_errors() {
    let unclosed =
        "<script setup lang=\"ts\">\nconst a = 1\n\n<template>\n  <p>{{ a }}</p>\n</template>\n";
    let bad = "<template><p/></template>\n<script lang=\"ts\">\nfunction f(a: number {\n  return a\n}\nfunction g(b: number) { return b }\n</script>\n";
    let (start, end) = content(bad, "<script lang=\"ts\">", "</script>");
    let ws = workspace(&[
        ("Unclosed.vue", unclosed),
        ("BadTs.vue", bad),
        ("view.ts", &view(bad, start, end)),
    ]);

    let (actual, _) = skeleton(ws.path(), "Unclosed.vue");
    assert_eq!(actual, Vec::<Row>::new());
    let (symbols_unclosed, parse_error) = symbols(ws.path(), "Unclosed.vue");
    assert!(symbols_unclosed.is_empty(), "{symbols_unclosed:?}");
    assert!(parse_error);

    let (actual, _) = skeleton(ws.path(), "BadTs.vue");
    let (mut expected, _) = skeleton(ws.path(), "view.ts");
    expected.extend([
        row("module", "<template>", "1:1-1:26", "1:1-1:11"),
        row("element", "p", "1:11-1:15", "1:12-1:13"),
        row("module", "<script lang=\"ts\">", "2:1-7:10", "2:1-2:19"),
    ]);
    assert_eq!(set(&actual), set(&expected));
    assert_eq!(blocks(&actual), ["<template>", "<script lang=\"ts\">"]);
    let (symbols_bad, parse_error) = symbols(ws.path(), "BadTs.vue");
    assert!(parse_error);
    let functions: Vec<_> = symbols_bad.iter().filter(|s| s.0 == "function").collect();
    assert_eq!(
        functions,
        [
            &(
                "function".to_string(),
                "f".to_string(),
                "3:10-3:11".to_string()
            ),
            &(
                "function".to_string(),
                "g".to_string(),
                "6:10-6:11".to_string()
            ),
        ]
    );
}
