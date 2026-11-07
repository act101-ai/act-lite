//! Vue single-file components: the script and style regions the Vue
//! detector finds, the outer structure the Vue queries report, and every
//! location in the component file's own coordinates.

use act_lite_core::CompactRange;
use act_lite_parser::regions::{RegionKind, RegionLanguage, RegionState, view};
use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_skeleton, execute_symbols};
use std::collections::BTreeSet;
use streaming_iterator::StreamingIterator;

/// The committed fixture (`ä` on line 1 is two bytes).
const COUNTER: &str = include_str!("../../act-lite-cli/tests/fixtures/components/Counter.vue");

/// `(kind, name, range, name_range)`.
type Row = (String, String, String, Option<String>);

fn load(name: &str, text: &str) -> SourceFile {
    SourceFile::from_content(name, text.to_string(), Grammar::Vue).unwrap()
}

fn row(kind: &str, name: &str, range: &str, name_range: Option<&str>) -> Row {
    (
        kind.to_string(),
        name.to_string(),
        range.to_string(),
        name_range.map(str::to_string),
    )
}

fn rows_of(result: QueryResult) -> Vec<Row> {
    let QueryResult::Skeleton(skeleton) = result else {
        panic!("not a skeleton");
    };
    skeleton
        .declarations
        .into_iter()
        .map(|d| {
            (
                d.kind,
                d.name,
                CompactRange::from(&d.range).to_string(),
                d.name_range.map(|r| CompactRange::from(&r).to_string()),
            )
        })
        .collect()
}

fn skeleton(source: &SourceFile) -> Vec<Row> {
    rows_of(execute_skeleton(source).unwrap())
}

/// `(kind, name, range)`.
type SymbolRow = (String, String, String);

fn symbols(source: &SourceFile) -> Vec<SymbolRow> {
    let QueryResult::Symbols { symbols } = execute_symbols(source).unwrap() else {
        panic!("not symbols");
    };
    symbols
        .into_iter()
        .map(|s| {
            (
                format!("{:?}", s.kind()),
                s.name().to_string(),
                CompactRange::from(s.range()).to_string(),
            )
        })
        .collect()
}

fn symbol(kind: &str, name: &str, range: &str) -> SymbolRow {
    (kind.to_string(), name.to_string(), range.to_string())
}

fn set<T: Ord + Clone>(items: &[T]) -> BTreeSet<T> {
    items.iter().cloned().collect()
}

/// The names of the `module` rows, in output order.
fn blocks(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.0 == "module")
        .map(|r| r.1.clone())
        .collect()
}

/// The view file of region `index`: every byte outside its content blanked.
fn view_of(source: &SourceFile, index: usize, path: &str, grammar: Grammar) -> SourceFile {
    let region = &source.regions()[index];
    SourceFile::from_content(path, view(source.content(), region), grammar).unwrap()
}

/// The first node of `kind`, or MISSING node when `kind` is `MISSING`.
fn find<'t>(node: tree_sitter::Node<'t>, kind: &str) -> Option<tree_sitter::Node<'t>> {
    if node.kind() == kind || (kind == "MISSING" && node.is_missing()) {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    children.into_iter().find_map(|child| find(child, kind))
}

fn compact(node: tree_sitter::Node) -> String {
    let (s, e) = (node.start_position(), node.end_position());
    format!(
        "{}:{}-{}:{}",
        s.row + 1,
        s.column + 1,
        e.row + 1,
        e.column + 1
    )
}

fn counter_table() -> Vec<Row> {
    vec![
        row(
            "module",
            "<script lang=\"ts\">",
            "2:1-4:10",
            Some("2:1-2:19"),
        ),
        row(
            "export",
            "export default { name: 'Counter' }",
            "3:1-3:35",
            None,
        ),
        row(
            "module",
            "<script setup lang=\"ts\">",
            "6:1-15:10",
            Some("6:1-6:25"),
        ),
        row("import", "import { ref } from 'vue'", "7:1-7:26", None),
        row("variable", "props", "8:1-8:47", Some("8:7-8:12")),
        row("variable", "count", "9:1-9:31", Some("9:7-9:12")),
        row("function", "increment", "10:1-14:2", Some("10:10-10:19")),
        row("module", "<template>", "17:1-22:12", Some("17:1-17:11")),
        row("element", "div", "18:3-21:9", Some("18:4-18:7")),
        row("field", "app", "18:8-18:16", Some("18:12-18:15")),
        row("field", "counter", "18:17-18:36", Some("18:24-18:31")),
        row("field", "big", "18:17-18:36", Some("18:32-18:35")),
        row("element", "button", "19:5-19:45", Some("19:6-19:12")),
        row("element", "span", "20:5-20:46", Some("20:6-20:10")),
        row("module", "<style scoped>", "24:1-26:9", Some("24:1-24:15")),
        row("class", ".counter", "25:1-25:25", Some("25:1-25:9")),
        row("field", "color", "25:12-25:23", Some("25:12-25:17")),
    ]
}

#[test]
fn counter_skeleton_is_the_table_in_block_order() {
    let source = load("Counter.vue", COUNTER);
    let actual = skeleton(&source);
    let expected = counter_table();
    assert_eq!(set(&actual), set(&expected), "{actual:#?}");
    assert_eq!(actual.len(), expected.len(), "{actual:#?}");
    assert_eq!(blocks(&actual), blocks(&expected));
}

#[test]
fn counter_increment_has_the_byte_offsets_of_the_file() {
    let source = load("Counter.vue", COUNTER);
    let QueryResult::Skeleton(result) = execute_skeleton(&source).unwrap() else {
        panic!("not a skeleton");
    };
    let increment = result
        .declarations
        .iter()
        .find(|d| d.name == "increment")
        .expect("increment is declared");
    assert_eq!(increment.range.start().byte_offset(), Some(224));
    assert_eq!(increment.range.end().byte_offset(), Some(332));
    assert_eq!(
        &COUNTER[224..332],
        "function increment(step: number): void {\n  if (step > 0 && count.value < 10) {\n    count.value += step\n  }\n}"
    );
}

#[test]
fn counter_symbols_are_the_blocks_the_template_and_the_region_symbols() {
    let source = load("Counter.vue", COUNTER);
    let actual = symbols(&source);
    let expected = [
        symbol("Module", "<script lang=\"ts\">", "2:1-2:19"),
        symbol("Unknown", "name", "3:18-3:22"),
        symbol("Module", "<script setup lang=\"ts\">", "6:1-6:25"),
        symbol("Variable", "props", "8:7-8:12"),
        symbol("Field", "start", "8:29-8:34"),
        symbol("Variable", "count", "9:7-9:12"),
        symbol("Function", "increment", "10:10-10:19"),
        symbol("Module", "<template>", "17:1-17:11"),
        symbol("Class", "div", "18:4-18:7"),
        symbol("Field", "app", "18:12-18:15"),
        symbol("Field", "counter", "18:24-18:31"),
        symbol("Field", "big", "18:32-18:35"),
        symbol("Class", "button", "19:6-19:12"),
        symbol("Class", "span", "20:6-20:10"),
        symbol("Module", "<style scoped>", "24:1-24:15"),
        symbol("Unknown", ".counter", "25:1-25:9"),
        symbol("Field", "color", "25:12-25:17"),
    ];
    assert_eq!(set(&actual), set(&expected), "{actual:#?}");
    // `start` is only the member of the type literal, never a prop.
    let starts: Vec<&SymbolRow> = actual.iter().filter(|s| s.1 == "start").collect();
    assert_eq!(starts, [&symbol("Field", "start", "8:29-8:34")]);
}

#[test]
fn counter_has_three_well_formed_regions_and_no_parse_error() {
    let source = load("Counter.vue", COUNTER);
    let regions = source.regions();
    let found: Vec<(RegionKind, RegionLanguage, RegionState, String)> = regions
        .iter()
        .map(|r| (r.kind, r.language.clone(), r.state, r.start_tag.to_string()))
        .collect();
    let ts = RegionLanguage::Supported(Grammar::TypeScript);
    assert_eq!(
        found,
        [
            (
                RegionKind::Script,
                ts.clone(),
                RegionState::WellFormed,
                "2:1-2:19".to_string()
            ),
            (
                RegionKind::Script,
                ts,
                RegionState::WellFormed,
                "6:1-6:25".to_string()
            ),
            (
                RegionKind::Style,
                RegionLanguage::Supported(Grammar::Css),
                RegionState::WellFormed,
                "24:1-24:15".to_string()
            ),
        ]
    );
    let start = COUNTER.find("<script setup lang=\"ts\">").unwrap() + 24;
    let end = COUNTER.find("</script>\n\n<template>").unwrap();
    assert_eq!(regions[1].content, Some(start..end));
    assert_eq!(source.region_trees().len(), 3);
    assert!(!source.has_any_error());
}

#[test]
fn counter_region_declarations_equal_those_of_their_views() {
    let source = load("Counter.vue", COUNTER);
    let all = set(&skeleton(&source));
    for (index, path, grammar) in [
        (0, "view.ts", Grammar::TypeScript),
        (1, "view.ts", Grammar::TypeScript),
        (2, "view.css", Grammar::Css),
    ] {
        let viewed = skeleton(&view_of(&source, index, path, grammar));
        assert!(!viewed.is_empty(), "region {index}");
        for declaration in viewed {
            assert!(all.contains(&declaration), "{declaration:?}");
        }
    }
}

#[test]
fn an_error_inside_the_scoped_style_sets_only_parse_error() {
    let broken = COUNTER.replace(".counter { color: red; }", ".counter { color: red; } } {");
    let source = load("Counter.vue", &broken);
    assert!(!source.tree().root_node().has_error());
    assert_eq!(source.region_trees().len(), 3);
    assert!(source.region_trees()[2].tree.root_node().has_error());
    assert!(source.has_any_error());
}

#[test]
fn langs_vue_reports_three_blocks_and_nothing_else() {
    let text = "<template lang=\"pug\">\ndiv\n  p {{ a }}\n</template>\n<script lang=\"coffee\">\nx = 1\n</script>\n<i18n lang=\"json\">{\"a\":1}</i18n>\n";
    let source = load("Langs.vue", text);
    let actual = skeleton(&source);
    assert_eq!(
        actual,
        [
            row(
                "module",
                "<template lang=\"pug\">",
                "1:1-4:12",
                Some("1:1-1:22")
            ),
            row(
                "module",
                "<script lang=\"coffee\">",
                "5:1-7:10",
                Some("5:1-5:23")
            ),
            row(
                "module",
                "<i18n lang=\"json\">",
                "8:1-8:33",
                Some("8:1-8:19")
            ),
        ]
    );
    assert_eq!(source.regions().len(), 1);
    assert_eq!(
        source.regions()[0].language,
        RegionLanguage::Unsupported("coffee".to_string())
    );
    assert!(source.region_trees().is_empty());
    assert!(!source.has_any_error());
}

#[test]
fn tsx_vue_is_a_tsx_region() {
    let text =
        "<script setup lang=\"tsx\">\nconst view = () => <div class=\"x\">hi</div>\n</script>\n";
    let source = load("Tsx.vue", text);
    assert_eq!(
        source.regions()[0].language,
        RegionLanguage::Supported(Grammar::Tsx)
    );
    let actual = skeleton(&source);
    assert_eq!(
        actual,
        [
            row(
                "module",
                "<script setup lang=\"tsx\">",
                "1:1-3:10",
                Some("1:1-1:26")
            ),
            row("variable", "view", "2:1-2:43", Some("2:7-2:11")),
        ]
    );
    let viewed = view_of(&source, 0, "view.tsx", Grammar::Tsx);
    let mut expected = vec![symbol("Module", "<script setup lang=\"tsx\">", "1:1-1:26")];
    expected.extend(symbols(&viewed));
    assert_eq!(set(&symbols(&source)), set(&expected));
    assert!(!source.has_any_error());
    // Parsed as TypeScript, the same content has an error.
    let as_ts = view_of(&source, 0, "view.ts", Grammar::TypeScript);
    assert!(as_ts.tree().root_node().has_error());
}

#[test]
fn nest_t_vue_reports_the_nested_template_as_an_element() {
    let text = "<template>\n  <template v-if=\"a\"><p>x</p></template>\n</template>\n";
    let source = load("NestT.vue", text);
    assert_eq!(
        skeleton(&source),
        [
            row("module", "<template>", "1:1-3:12", Some("1:1-1:11")),
            row("element", "template", "2:3-2:41", Some("2:4-2:12")),
            row("element", "p", "2:22-2:30", Some("2:23-2:24")),
        ]
    );
    assert!(source.regions().is_empty());
    assert!(!source.has_any_error());
}

#[test]
fn src_vue_is_an_empty_region_with_its_block() {
    let text = "<script src=\"./a.ts\"></script>\n<template><p/></template>\n";
    let source = load("Src.vue", text);
    assert_eq!(
        skeleton(&source),
        [
            row(
                "module",
                "<script src=\"./a.ts\">",
                "1:1-1:31",
                Some("1:1-1:22")
            ),
            row("module", "<template>", "2:1-2:26", Some("2:1-2:11")),
            row("element", "p", "2:11-2:15", Some("2:12-2:13")),
        ]
    );
    assert_eq!(source.regions().len(), 1);
    assert_eq!(source.regions()[0].state, RegionState::Empty);
    assert_eq!(source.regions()[0].content, None);
    assert!(source.region_trees().is_empty());
    assert!(!source.has_any_error());
}

#[test]
fn two_vue_reports_both_script_setup_blocks() {
    let text = "<script setup lang=\"ts\">\nconst a = 1\n</script>\n<script setup lang=\"ts\">\nconst b = 2\n</script>\n";
    let source = load("Two.vue", text);
    assert_eq!(
        skeleton(&source),
        [
            row(
                "module",
                "<script setup lang=\"ts\">",
                "1:1-3:10",
                Some("1:1-1:25")
            ),
            row("variable", "a", "2:1-2:12", Some("2:7-2:8")),
            row(
                "module",
                "<script setup lang=\"ts\">",
                "4:1-6:10",
                Some("4:1-4:25")
            ),
            row("variable", "b", "5:1-5:12", Some("5:7-5:8")),
        ]
    );
    let variables: Vec<SymbolRow> = symbols(&source)
        .into_iter()
        .filter(|s| s.0 == "Variable")
        .collect();
    assert_eq!(
        variables,
        [
            symbol("Variable", "a", "2:7-2:8"),
            symbol("Variable", "b", "5:7-5:8")
        ]
    );
    assert_eq!(source.region_trees().len(), 2);
    assert!(!source.has_any_error());
}

#[test]
fn unclosed_vue_is_a_malformed_region_with_an_empty_skeleton() {
    let text =
        "<script setup lang=\"ts\">\nconst a = 1\n\n<template>\n  <p>{{ a }}</p>\n</template>\n";
    let source = load("Unclosed.vue", text);
    let regions = source.regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].kind, RegionKind::Script);
    assert_eq!(regions[0].state, RegionState::Malformed);
    assert_eq!(regions[0].start_tag.to_string(), "1:1-1:25");
    assert_eq!(regions[0].content, None);
    assert!(source.region_trees().is_empty());
    assert_eq!(skeleton(&source), Vec::<Row>::new());
    assert_eq!(symbols(&source), Vec::<SymbolRow>::new());
    assert!(source.has_any_error());
}

#[test]
fn bad_ts_vue_is_well_formed_with_a_missing_paren_in_its_region() {
    let text = "<template><p/></template>\n<script lang=\"ts\">\nfunction f(a: number {\n  return a\n}\nfunction g(b: number) { return b }\n</script>\n";
    let source = load("BadTs.vue", text);
    assert_eq!(source.regions().len(), 1);
    assert_eq!(source.regions()[0].state, RegionState::WellFormed);
    assert!(!source.tree().root_node().has_error());
    let tree = &source.region_trees()[0].tree;
    let missing = find(tree.root_node(), "MISSING").expect("a MISSING node");
    assert_eq!(missing.kind(), ")");
    assert_eq!(compact(missing), "3:21-3:21");
    assert!(source.has_any_error());

    let functions: Vec<SymbolRow> = symbols(&source)
        .into_iter()
        .filter(|s| s.0 == "Function")
        .collect();
    assert_eq!(
        functions,
        [
            symbol("Function", "f", "3:10-3:11"),
            symbol("Function", "g", "6:10-6:11")
        ]
    );

    let mut expected = vec![
        row("module", "<template>", "1:1-1:26", Some("1:1-1:11")),
        row("element", "p", "1:11-1:15", Some("1:12-1:13")),
        row(
            "module",
            "<script lang=\"ts\">",
            "2:1-7:10",
            Some("2:1-2:19"),
        ),
    ];
    expected.extend(skeleton(&view_of(
        &source,
        0,
        "view.ts",
        Grammar::TypeScript,
    )));
    let actual = skeleton(&source);
    assert_eq!(set(&actual), set(&expected));
    assert_eq!(
        blocks(&actual),
        ["<template>", "<script lang=\"ts\">"].map(String::from)
    );
}

#[test]
fn a_script_or_style_below_the_top_level_is_not_a_region() {
    let text = "<template><div><script>var a = 1</script><style>p{}</style></div></template>\n<docs><script>var b = 2</script></docs>\n";
    let source = load("Deep.vue", text);
    assert!(source.regions().is_empty(), "{:?}", source.regions());
    let actual = skeleton(&source);
    assert!(
        !actual
            .iter()
            .any(|r| r.1.contains("script") || r.1.contains("style")),
        "{actual:?}"
    );
    assert!(!actual.iter().any(|r| r.1 == "a" || r.1 == "b"));
}

#[test]
fn template_lang_html_content_is_structure() {
    let text = "<template lang=\"html\"><p class=\"h\">a</p></template>\n";
    let source = load("Html.vue", text);
    assert_eq!(
        skeleton(&source),
        [
            row(
                "module",
                "<template lang=\"html\">",
                "1:1-1:52",
                Some("1:1-1:23")
            ),
            row("element", "p", "1:23-1:41", Some("1:24-1:25")),
            row("field", "h", "1:26-1:35", Some("1:33-1:34")),
        ]
    );
}

#[test]
fn with_lang_given_twice_the_first_counts() {
    // As JavaScript, `let x: number` would be a parse error.
    let text = "<script lang=\"ts\" lang=\"js\">let x: number = 1</script>\n";
    let source = load("Twice.vue", text);
    assert_eq!(
        source.regions()[0].language,
        RegionLanguage::Supported(Grammar::TypeScript)
    );
    assert!(!source.has_any_error());
    let text = "<script lang=\"js\" lang=\"ts\">let x: number = 1</script>\n";
    let source = load("Twice.vue", text);
    assert_eq!(
        source.regions()[0].language,
        RegionLanguage::Supported(Grammar::JavaScript)
    );
}

#[test]
fn script_and_style_languages_follow_lang() {
    for (tag, kind, language) in [
        ("<script>", RegionKind::Script, Some(Grammar::JavaScript)),
        (
            "<script lang=\"js\">",
            RegionKind::Script,
            Some(Grammar::JavaScript),
        ),
        (
            "<script lang=\"jsx\">",
            RegionKind::Script,
            Some(Grammar::Jsx),
        ),
        (
            "<script lang=\"ts\">",
            RegionKind::Script,
            Some(Grammar::TypeScript),
        ),
        (
            "<script lang=\"tsx\">",
            RegionKind::Script,
            Some(Grammar::Tsx),
        ),
        ("<script lang=\"TS\">", RegionKind::Script, None),
        ("<style>", RegionKind::Style, Some(Grammar::Css)),
        (
            "<style lang=\"css\">",
            RegionKind::Style,
            Some(Grammar::Css),
        ),
        (
            "<style lang=\"scss\">",
            RegionKind::Style,
            Some(Grammar::Scss),
        ),
        ("<style lang=\"sass\">", RegionKind::Style, None),
    ] {
        let end = if kind == RegionKind::Script {
            "</script>"
        } else {
            "</style>"
        };
        let text = format!("{tag}\nx\n{end}\n");
        let source = load("Lang.vue", &text);
        let regions = source.regions();
        assert_eq!(regions.len(), 1, "{tag}");
        assert_eq!(regions[0].kind, kind, "{tag}");
        let expected = match language {
            Some(grammar) => RegionLanguage::Supported(grammar),
            None => {
                let value = tag.split('"').nth(1).unwrap();
                RegionLanguage::Unsupported(value.to_string())
            }
        };
        assert_eq!(regions[0].language, expected, "{tag}");
    }
}

#[test]
fn a_less_style_is_a_block_with_no_declaration() {
    let text = "<style lang=\"less\">\n@a: 1;\n.b { color: @a; }\n</style>\n";
    let source = load("Less.vue", text);
    assert_eq!(
        source.regions()[0].language,
        RegionLanguage::Unsupported("less".to_string())
    );
    assert_eq!(
        skeleton(&source),
        [row(
            "module",
            "<style lang=\"less\">",
            "1:1-4:9",
            Some("1:1-1:20")
        )]
    );
    assert!(source.region_trees().is_empty());
    assert!(!source.has_any_error());
}

#[test]
fn a_script_start_tag_in_an_error_at_the_document_is_a_top_level_malformed_region() {
    let text = "<template><p>a</p></template>\n<div><script lang=\"ts\">\nlet a = 1\n";
    let source = load("Open.vue", text);
    let regions = source.regions();
    assert_eq!(regions.len(), 1, "{regions:?}");
    assert_eq!(regions[0].kind, RegionKind::Script);
    assert_eq!(regions[0].state, RegionState::Malformed);
    assert_eq!(regions[0].start_tag.to_string(), "2:6-2:24");
    assert!(source.region_trees().is_empty());
    assert!(source.has_any_error());
    let actual = skeleton(&source);
    assert_eq!(
        actual,
        [
            row("module", "<template>", "1:1-1:30", Some("1:1-1:11")),
            row("element", "p", "1:11-1:19", Some("1:12-1:13")),
        ]
    );
}

/// The text and `L:C` start of every `@reference` capture of the Vue
/// references query, in document order.
fn references(text: &str) -> Vec<(String, String)> {
    let support = Grammar::Vue.language_support();
    let language = support.ts_language();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(text, None).unwrap();
    let query = tree_sitter::Query::new(&language, support.references_query()).unwrap();
    let reference = query.capture_index_for_name("reference").unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    let mut found = BTreeSet::new();
    while let Some(m) = matches.next() {
        for capture in m.captures.iter().filter(|c| c.index == reference) {
            let node = capture.node;
            found.insert((
                node.start_byte(),
                node.utf8_text(text.as_bytes()).unwrap().to_string(),
                format!(
                    "{}:{}",
                    node.start_position().row + 1,
                    node.start_position().column + 1
                ),
            ));
        }
    }
    found.into_iter().map(|(_, t, p)| (t, p)).collect()
}

#[test]
fn the_outer_references_query_follows_html_conventions() {
    let pairs = |items: &[(&str, &str)]| -> Vec<(String, String)> {
        items
            .iter()
            .map(|(t, p)| (t.to_string(), p.to_string()))
            .collect()
    };
    assert_eq!(
        references(COUNTER),
        pairs(&[
            ("script", "4:3"),
            ("script", "15:3"),
            ("app", "18:12"),
            ("counter big", "18:24"),
            ("button", "19:38"),
            ("span", "20:41"),
            ("div", "21:5"),
            ("template", "22:3"),
            ("style", "26:3"),
        ])
    );
    let text = "<template><a href=\"#top\" data-x=\"v\" data-y=w>t</a></template>\n";
    assert_eq!(
        references(text),
        pairs(&[
            ("#top", "1:20"),
            ("v", "1:34"),
            ("w", "1:44"),
            ("a", "1:49"),
            ("template", "1:53"),
        ])
    );
}
