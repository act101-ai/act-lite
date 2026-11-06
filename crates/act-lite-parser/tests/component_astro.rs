//! Astro component files: the frontmatter, script and style regions, the
//! language each one is parsed with, the outer structure, and every
//! location in the component file's own coordinates.

use act_lite_core::CompactRange;
use act_lite_parser::regions::{RegionKind, RegionLanguage, RegionState, view};
use act_lite_parser::{Grammar, QueryResult, SourceFile, execute_skeleton, execute_symbols};
use streaming_iterator::StreamingIterator;

const CARD: &str = include_str!("../../act-lite-cli/tests/fixtures/components/Card.astro");

fn astro(name: &str, text: &str) -> SourceFile {
    SourceFile::from_content(name, text.to_string(), Grammar::Astro).unwrap()
}

/// A declaration without its file: kind, name, range, name range.
type Row = (String, String, String, Option<String>);

fn row(kind: &str, name: &str, range: &str, name_range: Option<&str>) -> Row {
    (
        kind.to_string(),
        name.to_string(),
        range.to_string(),
        name_range.map(str::to_string),
    )
}

fn skeleton(source: &SourceFile) -> Vec<Row> {
    let QueryResult::Skeleton(skeleton) = execute_skeleton(source).unwrap() else {
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

/// A symbol without its file: kind, name, range.
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

fn sorted<T: Ord + Clone>(items: &[T]) -> Vec<T> {
    let mut items = items.to_vec();
    items.sort();
    items
}

/// The `module` rows in their order.
fn blocks(rows: &[Row]) -> Vec<Row> {
    rows.iter().filter(|r| r.0 == "module").cloned().collect()
}

/// The skeleton of `Card.astro` as the design's table gives it.
fn card_table() -> Vec<Row> {
    vec![
        row("module", "---", "1:1-8:4", Some("1:1-1:4")),
        row(
            "import",
            "import Layout from '../layouts/Layout.astro';",
            "2:1-2:46",
            None,
        ),
        row("interface", "Props", "3:1-3:34", Some("3:11-3:16")),
        row("function", "shout", "5:1-7:2", Some("5:10-5:15")),
        row("element", "Layout", "9:1-11:10", Some("9:2-9:8")),
        row("element", "h1", "10:3-10:45", Some("10:4-10:6")),
        row("field", "top", "10:7-10:15", Some("10:11-10:14")),
        row("field", "t", "10:16-10:25", Some("10:23-10:24")),
        row("module", "<script>", "12:1-14:10", Some("12:1-12:9")),
        row("function", "hello", "13:3-13:49", Some("13:12-13:17")),
        row(
            "module",
            "<script is:inline>",
            "15:1-15:43",
            Some("15:1-15:19"),
        ),
        row("variable", "inline", "15:19-15:34", Some("15:23-15:29")),
        row(
            "module",
            "<script type=\"application/ld+json\">",
            "16:1-16:53",
            Some("16:1-16:36"),
        ),
        row("module", "<style>", "17:1-19:9", Some("17:1-17:8")),
        row("class", "h1", "18:3-18:21", Some("18:3-18:5")),
        row("field", "color", "18:8-18:19", Some("18:8-18:13")),
    ]
}

fn assert_skeleton(source: &SourceFile, want: &[Row]) {
    let got = skeleton(source);
    assert_eq!(sorted(&got), sorted(want), "declaration set");
    assert_eq!(blocks(&got), blocks(want), "block order");
}

#[test]
fn card_skeleton_is_the_table_in_document_order() {
    let source = astro("Card.astro", CARD);
    assert_skeleton(&source, &card_table());
    assert_eq!(skeleton(&source), card_table());
}

#[test]
fn card_shout_has_its_byte_offsets_in_the_file() {
    let source = astro("Card.astro", CARD);
    let QueryResult::Skeleton(skeleton) = execute_skeleton(&source).unwrap() else {
        panic!("not a skeleton");
    };
    let shout = skeleton
        .declarations
        .iter()
        .find(|d| d.name == "shout")
        .expect("shout");
    assert_eq!(shout.range.start().byte_offset(), Some(115));
    assert_eq!(shout.range.end().byte_offset(), Some(197));
}

#[test]
fn card_has_no_parse_error() {
    assert!(!astro("Card.astro", CARD).has_any_error());
}

#[test]
fn card_regions_have_their_kinds_languages_and_content() {
    let source = astro("Card.astro", CARD);
    let regions: Vec<_> = source
        .regions()
        .iter()
        .map(|r| {
            (
                r.kind,
                r.language.clone(),
                r.state,
                r.start_tag.to_string(),
                r.element.to_string(),
            )
        })
        .collect();
    let ts = RegionLanguage::Supported(Grammar::TypeScript);
    let well = RegionState::WellFormed;
    assert_eq!(
        regions,
        [
            (
                RegionKind::Frontmatter,
                ts.clone(),
                well,
                "1:1-1:4".into(),
                "1:1-8:4".into()
            ),
            (
                RegionKind::Script,
                ts,
                well,
                "12:1-12:9".into(),
                "12:1-14:10".into()
            ),
            (
                RegionKind::Script,
                RegionLanguage::Supported(Grammar::JavaScript),
                well,
                "15:1-15:19".into(),
                "15:1-15:43".into()
            ),
            (
                RegionKind::Style,
                RegionLanguage::Supported(Grammar::Css),
                well,
                "17:1-17:8".into(),
                "17:1-19:9".into()
            ),
        ]
    );
    assert_eq!(source.regions()[0].content, Some(3..197));
    assert_eq!(source.region_trees().len(), 4);
}

/// Each region's symbols are those of its view file; the other symbols are
/// the blocks at their start tags and the template structure.
#[test]
fn card_symbols_are_the_blocks_the_structure_and_the_region_symbols() {
    let source = astro("Card.astro", CARD);
    let got = symbols(&source);
    let mut in_regions = Vec::new();
    for tree in source.region_trees() {
        let region = &source.regions()[tree.region];
        let ext = match tree.grammar {
            Grammar::TypeScript => "ts",
            Grammar::JavaScript => "js",
            Grammar::Css => "css",
            other => panic!("unexpected region grammar {other:?}"),
        };
        let viewed =
            SourceFile::from_content(format!("Card.{ext}"), view(CARD, region), tree.grammar)
                .unwrap();
        let want = symbols(&viewed);
        assert!(!want.is_empty(), "{ext} region has symbols");
        let content = region.content.clone().unwrap();
        let inside: Vec<SymbolRow> = got
            .iter()
            .filter(|s| s.0 != "Module" && region_contains(CARD, &content, &parse(&s.2)))
            .cloned()
            .collect();
        assert_eq!(sorted(&inside), sorted(&want), "{ext} region symbols");
        in_regions.extend(inside);
    }
    let outer: Vec<SymbolRow> = got
        .iter()
        .filter(|s| !in_regions.contains(s))
        .cloned()
        .collect();
    let sym = |k: &str, n: &str, r: &str| (k.to_string(), n.to_string(), r.to_string());
    assert_eq!(
        outer,
        [
            sym("Module", "---", "1:1-1:4"),
            sym("Class", "Layout", "9:2-9:8"),
            sym("Class", "h1", "10:4-10:6"),
            sym("Field", "top", "10:11-10:14"),
            sym("Field", "t", "10:23-10:24"),
            sym("Module", "<script>", "12:1-12:9"),
            sym("Module", "<script is:inline>", "15:1-15:19"),
            sym(
                "Module",
                "<script type=\"application/ld+json\">",
                "16:1-16:36"
            ),
            sym("Module", "<style>", "17:1-17:8"),
        ]
    );
    // The destructured prop is a symbol of the frontmatter, not a skeleton
    // declaration.
    assert!(
        got.contains(&sym("Unknown", "title", "4:9-4:14")),
        "{got:?}"
    );
}

fn parse(text: &str) -> CompactRange {
    serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
}

/// Byte offset of a 1-based line and byte column.
fn byte(text: &str, line: u32, column: u32) -> usize {
    let start: usize = text
        .split_inclusive('\n')
        .take(line as usize - 1)
        .map(str::len)
        .sum();
    start + column as usize - 1
}

fn region_contains(text: &str, content: &std::ops::Range<usize>, r: &CompactRange) -> bool {
    let start = byte(text, r.start_line(), r.start_col());
    let end = byte(text, r.end_line(), r.end_col());
    content.start <= start && end <= content.end
}

#[test]
fn card_with_crlf_line_ends_keeps_every_line_and_column() {
    let crlf = CARD.replace('\n', "\r\n");
    let source = astro("Card.astro", &crlf);
    assert_eq!(source.regions()[0].kind, RegionKind::Frontmatter);
    assert_eq!(source.regions()[0].state, RegionState::WellFormed);
    assert_skeleton(&source, &card_table());
    assert!(!source.has_any_error());
}

#[test]
fn card_reports_the_frontmatter_and_both_script_regions() {
    let rows = skeleton(&astro("Card.astro", CARD));
    for name in ["shout", "hello", "inline"] {
        assert!(rows.iter().any(|r| r.1 == name), "{name}: {rows:?}");
    }
    // The JSON-LD data block has no declaration: nothing lies on line 16
    // but its block.
    let on_16: Vec<_> = rows.iter().filter(|r| r.2.starts_with("16:")).collect();
    assert_eq!(on_16.len(), 1, "{on_16:?}");
}

#[test]
fn unclosed_frontmatter_is_malformed_and_reports_nothing() {
    let source = astro("Unclosed.astro", "---\nconst a = 1;\n<p>{a}</p>\n");
    let regions = source.regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].kind, RegionKind::Frontmatter);
    assert_eq!(regions[0].state, RegionState::Malformed);
    assert_eq!(regions[0].start_tag.to_string(), "1:1-1:4");
    assert!(source.region_trees().is_empty());
    assert_eq!(skeleton(&source), []);
    assert!(source.has_any_error());
    let missing = first(source.tree().root_node(), &|n| n.is_missing()).expect("a MISSING node");
    assert_eq!(missing.kind(), "---");
    assert_eq!(
        (missing.start_position(), missing.end_position()),
        (
            tree_sitter::Point::new(2, 10),
            tree_sitter::Point::new(2, 10)
        )
    );
}

fn first<'t>(
    node: tree_sitter::Node<'t>,
    test: &dyn Fn(tree_sitter::Node) -> bool,
) -> Option<tree_sitter::Node<'t>> {
    if test(node) {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    children.into_iter().find_map(|child| first(child, test))
}

#[test]
fn a_frontmatter_the_grammar_ends_early_is_malformed() {
    let source = astro(
        "Regex.astro",
        "---\nfunction e(v) { return v.replace(/\"/g, \"&quot;\"); }\n---\n<p/>\n",
    );
    let regions = source.regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].kind, RegionKind::Frontmatter);
    assert_eq!(regions[0].state, RegionState::Malformed);
    assert_eq!(regions[0].start_tag.to_string(), "1:1-1:4");
    assert!(source.region_trees().is_empty());
    assert_skeleton(&source, &[row("element", "p", "4:1-4:5", Some("4:2-4:3"))]);
    assert!(source.has_any_error());
    let error = first(source.tree().root_node(), &|n| {
        n.is_error() || n.is_missing()
    })
    .expect("an error node");
    assert_eq!(error.start_position(), tree_sitter::Point::new(0, 3));
}

#[test]
fn empty_regions_are_blocks_and_nothing_else() {
    let source = astro(
        "Empty.astro",
        "---\n---\n<script src=\"/x.js\"/>\n<style></style>\n",
    );
    let states: Vec<_> = source.regions().iter().map(|r| (r.kind, r.state)).collect();
    assert_eq!(
        states,
        [
            (RegionKind::Frontmatter, RegionState::Empty),
            (RegionKind::Script, RegionState::Empty),
            (RegionKind::Style, RegionState::Empty),
        ]
    );
    assert!(source.region_trees().is_empty());
    assert_skeleton(
        &source,
        &[
            row("module", "---", "1:1-2:4", Some("1:1-1:4")),
            row(
                "module",
                "<script src=\"/x.js\"/>",
                "3:1-3:22",
                Some("3:1-3:22"),
            ),
            row("module", "<style>", "4:1-4:16", Some("4:1-4:8")),
        ],
    );
    assert!(!source.has_any_error());
}

#[test]
fn a_script_below_the_top_level_is_a_typescript_region() {
    let source = astro(
        "Nested.astro",
        "<div><script>function n() {}</script></div>\n",
    );
    assert_eq!(source.regions().len(), 1);
    assert_eq!(
        source.regions()[0].language,
        RegionLanguage::Supported(Grammar::TypeScript)
    );
    assert_skeleton(
        &source,
        &[
            row("element", "div", "1:1-1:44", Some("1:2-1:5")),
            row("module", "<script>", "1:6-1:38", Some("1:6-1:14")),
            row("function", "n", "1:14-1:29", Some("1:23-1:24")),
        ],
    );
    assert!(!source.has_any_error());
}

#[test]
fn a_typescript_script_and_an_inline_javascript_script_have_no_parse_error() {
    let source = astro(
        "Langs.astro",
        "<script>function f(a: number): void {}</script>\n<script is:inline>var g = 1;</script>\n",
    );
    let languages: Vec<_> = source
        .regions()
        .iter()
        .map(|r| r.language.clone())
        .collect();
    assert_eq!(
        languages,
        [
            RegionLanguage::Supported(Grammar::TypeScript),
            RegionLanguage::Supported(Grammar::JavaScript),
        ]
    );
    assert!(!source.has_any_error());
}

#[test]
fn a_frontmatter_after_a_bom_an_empty_line_or_a_comment_is_a_region() {
    for (case, text) in [
        ("bom", "\u{feff}---\nconst a = 1;\n---\n<p/>\n"),
        ("empty line", "\n---\nconst a = 1;\n---\n<p/>\n"),
        ("comment", "<!-- c -->\n---\nconst a = 1;\n---\n<p/>\n"),
    ] {
        let source = astro("Front.astro", text);
        let regions = source.regions();
        assert_eq!(regions.len(), 1, "{case}");
        assert_eq!(regions[0].kind, RegionKind::Frontmatter, "{case}");
        assert_eq!(regions[0].state, RegionState::WellFormed, "{case}");
        assert_eq!(
            regions[0].language,
            RegionLanguage::Supported(Grammar::TypeScript),
            "{case}"
        );
        let rows = skeleton(&source);
        assert_eq!(blocks(&rows).len(), 1, "{case}: {rows:?}");
        assert!(rows.iter().any(|r| r.1 == "a"), "{case}: {rows:?}");
        assert!(!source.has_any_error(), "{case}");
    }
}

/// The region language of a lone `<script ...>let a = 1;</script>`, or
/// `None` when it is a data block; and the number of blocks it reports.
fn script_language(start_tag: &str) -> (Option<RegionLanguage>, usize) {
    let text = format!("{start_tag}let a = 1;</script>\n");
    let source = astro("Type.astro", &text);
    assert!(source.regions().len() <= 1, "{start_tag}");
    let rows = skeleton(&source);
    let language = source.regions().first().map(|r| r.language.clone());
    if language.is_none() {
        assert_eq!(
            rows,
            [row(
                "module",
                start_tag,
                &format!("1:1-1:{}", text.len()),
                Some(&format!("1:1-1:{}", start_tag.len() + 1)),
            )],
            "a data block reports its block only"
        );
    }
    (language, blocks(&rows).len())
}

#[test]
fn the_type_attribute_selects_the_script_language_or_a_data_block() {
    let ts = Some(RegionLanguage::Supported(Grammar::TypeScript));
    let js = Some(RegionLanguage::Supported(Grammar::JavaScript));
    for (start_tag, want) in [
        ("<script>", ts.clone()),
        ("<script src=\"/a.js\">", ts),
        ("<script type=\" module \">", js.clone()),
        ("<script type=\"TEXT/JAVASCRIPT\">", js.clone()),
        ("<script type=\"\">", js.clone()),
        ("<script type=module>", js.clone()),
        ("<script type=\"\ttext/ecmascript \">", js.clone()),
        ("<script is:inline>", js.clone()),
        ("<script define:vars={{ a }}>", js.clone()),
        ("<script type=\"text/typescript\">", None),
        ("<script type=\"application/ld+json\">", None),
        ("<script type=\"text/javascript; charset=utf-8\">", None),
        // The first `type` attribute counts.
        ("<script type=\"module\" type=\"application/json\">", js),
        ("<script type=\"application/json\" type=\"module\">", None),
    ] {
        assert_eq!(script_language(start_tag), (want, 1), "{start_tag}");
    }
}

#[test]
fn the_lang_attribute_selects_the_style_language() {
    for (start_tag, want) in [
        ("<style>", RegionLanguage::Supported(Grammar::Css)),
        (
            "<style lang=\"css\">",
            RegionLanguage::Supported(Grammar::Css),
        ),
        (
            "<style lang=\"scss\">",
            RegionLanguage::Supported(Grammar::Scss),
        ),
        (
            "<style lang=\"less\">",
            RegionLanguage::Unsupported("less".into()),
        ),
    ] {
        let text = format!("{start_tag}p {{ color: red; }}</style>\n");
        let source = astro("Style.astro", &text);
        assert_eq!(source.regions().len(), 1, "{start_tag}");
        assert_eq!(source.regions()[0].language, want, "{start_tag}");
        assert_eq!(blocks(&skeleton(&source)).len(), 1, "{start_tag}");
    }
}

#[test]
fn an_unclosed_script_is_a_malformed_region() {
    let source = astro("Open.astro", "<p/>\n<script>let a = 1;\n<i/>\n");
    let regions = source.regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].kind, RegionKind::Script);
    assert_eq!(regions[0].state, RegionState::Malformed);
    assert_eq!(regions[0].start_tag.to_string(), "2:1-2:9");
    assert!(source.region_trees().is_empty());
    assert_eq!(
        skeleton(&source),
        [row("element", "p", "1:1-1:5", Some("1:2-1:3"))]
    );
    assert!(source.has_any_error());
}

/// The text and range of each `@reference` capture of the Astro
/// `references.scm` on `text`.
fn references(text: &str) -> Vec<(String, String)> {
    let language = Grammar::Astro.language_support().ts_language();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(text, None).unwrap();
    let query = tree_sitter::Query::new(
        &language,
        Grammar::Astro.language_support().references_query(),
    )
    .unwrap();
    let index = query.capture_index_for_name("reference").unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    let mut found = Vec::new();
    while let Some(m) = matches.next() {
        for capture in m.captures.iter().filter(|c| c.index == index) {
            let node = capture.node;
            let (s, e) = (node.start_position(), node.end_position());
            found.push((
                node.utf8_text(text.as_bytes()).unwrap().to_string(),
                format!(
                    "{}:{}-{}:{}",
                    s.row + 1,
                    s.column + 1,
                    e.row + 1,
                    e.column + 1
                ),
            ));
        }
    }
    found.sort_by_key(|(_, r)| {
        let r = parse(r);
        (r.start_line(), r.start_col())
    });
    found
}

#[test]
fn the_outer_references_query_matches_id_class_and_end_tag_names() {
    let pair = |t: &str, r: &str| (t.to_string(), r.to_string());
    assert_eq!(
        references(CARD),
        [
            pair("top", "10:11-10:14"),
            pair("t", "10:23-10:24"),
            pair("h1", "10:42-10:44"),
            pair("Layout", "11:3-11:9"),
            pair("script", "14:3-14:9"),
            pair("script", "15:36-15:42"),
            pair("script", "16:46-16:52"),
            pair("style", "19:3-19:8"),
        ]
    );
}

#[test]
fn the_outer_references_query_matches_data_and_href_values() {
    let pair = |t: &str, r: &str| (t.to_string(), r.to_string());
    assert_eq!(
        references("<p data-x=\"v\" data-y=w></p>\n<a href=\"#top\"></a>\n"),
        [
            pair("v", "1:12-1:13"),
            pair("w", "1:22-1:23"),
            pair("p", "1:26-1:27"),
            pair("#top", "2:10-2:14"),
            pair("a", "2:18-2:19"),
        ]
    );
}

#[test]
fn a_script_type_written_as_an_expression_is_an_unsupported_region() {
    for type_attr in ["type={\"text/plain\"}", "type=`text/plain`"] {
        let text = format!("<script {type_attr}>function f(a) {{ return a; }}</script>\n");
        let source = SourceFile::from_content("E.astro", text.clone(), Grammar::Astro).unwrap();
        let regions = source.regions();
        assert_eq!(regions.len(), 1, "{type_attr}");
        assert_eq!(regions[0].kind, RegionKind::Script);
        assert_eq!(
            regions[0].language,
            RegionLanguage::Unsupported(type_attr["type=".len()..].to_string()),
            "{type_attr}"
        );
        assert!(source.region_trees().is_empty(), "{type_attr}");
        let QueryResult::Skeleton(skeleton) = execute_skeleton(&source).unwrap() else {
            panic!("not a skeleton");
        };
        let rows: Vec<(String, String)> = skeleton
            .declarations
            .into_iter()
            .map(|d| (d.kind, d.name))
            .collect();
        assert_eq!(
            rows,
            vec![("module".to_string(), format!("<script {type_attr}>"))],
            "{type_attr}"
        );
    }
}
