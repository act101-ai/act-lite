//! Svelte component files: the regions the Svelte detector finds, the outer
//! structure of the markup, and the declarations of each script and style
//! region at its location in the component file.

use act_lite_core::{CompactRange, Range, Symbol};
use act_lite_parser::regions::svelte::SvelteDetector;
use act_lite_parser::regions::{Detector, Region, RegionKind, RegionLanguage, RegionState, view};
use act_lite_parser::{
    Grammar, QueryResult, SourceFile, execute_skeleton, execute_symbol, execute_symbols,
};
use std::collections::BTreeSet;
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// The committed `Counter.svelte` fixture.
fn counter() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../act-lite-cli/tests/fixtures/components/Counter.svelte");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load(name: &str, text: &str) -> SourceFile {
    SourceFile::from_content(name, text.to_string(), Grammar::Svelte).unwrap()
}

/// The outer tree of a Svelte file, without regions.
fn outer_tree(text: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&Grammar::Svelte.language_support().ts_language())
        .unwrap();
    parser.parse(text, None).unwrap()
}

fn regions_of(text: &str) -> Vec<Region> {
    SvelteDetector.regions(&outer_tree(text), text)
}

/// `(kind, name, range, name_range)` of a declaration.
type Row = (String, String, String, String);

fn row(kind: &str, name: &str, range: &str, name_range: &str) -> Row {
    (
        kind.to_string(),
        name.to_string(),
        range.to_string(),
        name_range.to_string(),
    )
}

fn declarations(source: &SourceFile) -> Vec<act_lite_parser::Declaration> {
    let QueryResult::Skeleton(skeleton) = execute_skeleton(source).unwrap() else {
        panic!("not a skeleton");
    };
    skeleton.declarations
}

fn rows(source: &SourceFile) -> Vec<Row> {
    declarations(source)
        .into_iter()
        .map(|d| {
            (
                d.kind,
                d.name,
                CompactRange::from(&d.range).to_string(),
                d.name_range
                    .map(|r| CompactRange::from(&r).to_string())
                    .unwrap_or_else(|| "-".to_string()),
            )
        })
        .collect()
}

/// `(kind, name, range)` of a symbol; the kind as `SymbolKind` prints it.
type SymbolRow = (String, String, String);

fn symbols(source: &SourceFile) -> Vec<Symbol> {
    let QueryResult::Symbols { symbols } = execute_symbols(source).unwrap() else {
        panic!("not symbols");
    };
    symbols
}

fn symbol_rows(source: &SourceFile) -> Vec<SymbolRow> {
    symbols(source)
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

/// The `Counter.svelte` skeleton, in block order.
fn counter_table() -> Vec<Row> {
    vec![
        row(
            "module",
            "<script context=\"module\" lang=\"ts\">",
            "1:1-3:10",
            "1:1-1:36",
        ),
        row("export", "export const prerender = true;", "2:3-2:33", "-"),
        row("variable", "prerender", "2:10-2:33", "2:16-2:25"),
        row("element", "p", "5:1-5:15", "5:2-5:3"),
        row("module", "<script lang=\"ts\">", "5:15-13:10", "5:15-5:33"),
        row("export", "export let start: number = 0;", "5:33-5:62", "-"),
        row("variable", "start", "5:40-5:62", "5:44-5:49"),
        row("variable", "count", "6:3-6:21", "6:7-6:12"),
        row("function", "increment", "8:3-12:4", "8:12-8:21"),
        row("element", "div", "15:1-21:7", "15:2-15:5"),
        row("field", "counter", "15:6-15:21", "15:13-15:20"),
        row("element", "button", "16:3-16:51", "16:4-16:10"),
        row("element", "span", "18:5-18:35", "18:6-18:10"),
        row("function", "row", "20:3-20:48", "20:13-20:16"),
        row("element", "i", "20:28-20:38", "20:29-20:30"),
        row("module", "<style>", "23:1-25:9", "23:1-23:8"),
        row("class", ".counter", "24:3-24:27", "24:3-24:11"),
        row("field", "color", "24:14-24:25", "24:14-24:19"),
    ]
}

fn blocks(rows: &[Row]) -> Vec<Row> {
    rows.iter().filter(|r| r.0 == "module").cloned().collect()
}

fn find<'a>(
    declarations: &'a [act_lite_parser::Declaration],
    kind: &str,
    name: &str,
) -> &'a act_lite_parser::Declaration {
    declarations
        .iter()
        .find(|d| d.kind == kind && d.name == name)
        .unwrap_or_else(|| panic!("no {kind} {name}"))
}

fn bytes(range: &Range) -> (usize, usize) {
    (
        range.start().byte_offset().unwrap(),
        range.end().byte_offset().unwrap(),
    )
}

#[test]
fn counter_skeleton_is_exactly_the_table_with_its_block_order() {
    let source = load("Counter.svelte", &counter());
    let got = rows(&source);
    let want = counter_table();
    assert_eq!(set(&got), set(&want), "{got:#?}");
    assert_eq!(blocks(&got), blocks(&want));
    // No declaration for the `$:` statement's assigned name.
    assert!(!got.iter().any(|r| r.1.contains("doubled")), "{got:#?}");
}

#[test]
fn counter_byte_offsets_are_those_of_the_component_file() {
    let text = counter();
    let source = load("Counter.svelte", &text);
    let declarations = declarations(&source);
    let prerender = find(&declarations, "variable", "prerender");
    assert_eq!(bytes(prerender.name_range.as_ref().unwrap()), (51, 60));
    let instance = find(&declarations, "module", "<script lang=\"ts\">");
    assert_eq!(bytes(&instance.range), (94, 300));
    let export = find(&declarations, "export", "export let start: number = 0;");
    assert_eq!(bytes(&export.range), (112, 141));
    let start = find(&declarations, "variable", "start");
    assert_eq!(bytes(start.name_range.as_ref().unwrap()), (123, 128));
    assert_eq!(&text[123..128], "start");
}

#[test]
fn counter_has_no_parse_error() {
    let source = load("Counter.svelte", &counter());
    assert!(!source.tree().root_node().has_error());
    assert!(!source.has_any_error());
    assert_eq!(source.region_trees().len(), 3);
}

#[test]
fn counter_regions_are_two_typescript_scripts_and_a_css_style() {
    let text = counter();
    let regions = regions_of(&text);
    let summary: Vec<(RegionKind, RegionLanguage, RegionState, String, String)> = regions
        .iter()
        .map(|r| {
            (
                r.kind,
                r.language.clone(),
                r.state,
                r.element.to_string(),
                r.start_tag.to_string(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                RegionKind::Script,
                RegionLanguage::Supported(Grammar::TypeScript),
                RegionState::WellFormed,
                "1:1-3:10".to_string(),
                "1:1-1:36".to_string(),
            ),
            (
                RegionKind::Script,
                RegionLanguage::Supported(Grammar::TypeScript),
                RegionState::WellFormed,
                "5:15-13:10".to_string(),
                "5:15-5:33".to_string(),
            ),
            (
                RegionKind::Style,
                RegionLanguage::Supported(Grammar::Css),
                RegionState::WellFormed,
                "23:1-25:9".to_string(),
                "23:1-23:8".to_string(),
            ),
        ]
    );
    assert_eq!(regions[1].content, Some(112..291));
}

#[test]
fn counter_symbols_are_the_outer_structure_and_the_region_symbols() {
    let text = counter();
    let source = load("Counter.svelte", &text);
    let got = symbol_rows(&source);
    let mut want = vec![
        symbol(
            "Module",
            "<script context=\"module\" lang=\"ts\">",
            "1:1-1:36",
        ),
        symbol("Class", "p", "5:2-5:3"),
        symbol("Module", "<script lang=\"ts\">", "5:15-5:33"),
        symbol("Class", "div", "15:2-15:5"),
        symbol("Field", "counter", "15:13-15:20"),
        symbol("Class", "button", "16:4-16:10"),
        symbol("Class", "span", "18:6-18:10"),
        symbol("Function", "row", "20:13-20:16"),
        symbol("Class", "i", "20:29-20:30"),
        symbol("Module", "<style>", "23:1-23:8"),
    ];
    // The region symbols equal those of each region's view file.
    let views = [
        ("view.ts", Grammar::TypeScript),
        ("view.ts", Grammar::TypeScript),
        ("view.css", Grammar::Css),
    ];
    for (region, (path, grammar)) in regions_of(&text).iter().zip(views) {
        let viewed = SourceFile::from_content(path, view(&text, region), grammar).unwrap();
        want.extend(symbol_rows(&viewed));
    }
    assert_eq!(set(&got), set(&want), "{got:#?}");
    assert!(got.contains(&symbol("Function", "row", "20:13-20:16")));
    assert!(got.contains(&symbol("Variable", "prerender", "2:16-2:25")));
    assert!(!got.iter().any(|s| s.1 == "doubled"), "{got:#?}");
}

#[test]
fn query_symbol_prerender_reads_its_context_from_the_component_file() {
    let source = load("Counter.svelte", &counter());
    let QueryResult::Symbol(found) = execute_symbol(&source, "prerender").unwrap() else {
        panic!("not a symbol");
    };
    assert_eq!(
        CompactRange::from(found.symbol.range()).to_string(),
        "2:16-2:25"
    );
    assert_eq!(found.source, "prerender");
    assert_eq!(
        found.context_before,
        "<script context=\"module\" lang=\"ts\">"
    );
    assert_eq!(
        found.context_after,
        "</script>\n\n<p>Zähler</p><script lang=\"ts\">export let start: number = 0;"
    );
}

#[test]
fn crlf_keeps_every_line_and_column_and_shifts_each_offset_by_its_line() {
    let lf_text = counter();
    let crlf_text = lf_text.replace('\n', "\r\n");
    let lf = declarations(&load("Counter.svelte", &lf_text));
    let crlf_source = load("Crlf.svelte", &crlf_text);
    assert!(!crlf_source.has_any_error());
    let crlf = declarations(&crlf_source);
    let key = |d: &act_lite_parser::Declaration| {
        (
            d.kind.clone(),
            d.name.clone(),
            CompactRange::from(&d.range).to_string(),
            d.name_range
                .as_ref()
                .map(|r| CompactRange::from(r).to_string()),
        )
    };
    assert_eq!(lf.len(), counter_table().len());
    let lf_keys: Vec<_> = lf.iter().map(key).collect();
    let crlf_keys: Vec<_> = crlf.iter().map(key).collect();
    assert_eq!(set(&lf_keys), set(&crlf_keys));
    let shifted = |range: &Range| {
        (
            range.start().byte_offset().unwrap() + range.start().line() as usize - 1,
            range.end().byte_offset().unwrap() + range.end().line() as usize - 1,
        )
    };
    for d in &lf {
        let c = crlf
            .iter()
            .find(|c| key(c) == key(d))
            .unwrap_or_else(|| panic!("no CRLF row for {d:?}"));
        assert_eq!(bytes(&c.range), shifted(&d.range), "{d:?}");
        if let (Some(a), Some(b)) = (&d.name_range, &c.name_range) {
            assert_eq!(bytes(b), shifted(a), "{d:?}");
        }
    }
}

#[test]
fn ts_svelte_has_an_unsupported_script_reported_as_a_block_only() {
    let text = "<script lang=\"typescript\">\nlet a: number = 1\n</script>\n";
    let regions = regions_of(text);
    assert_eq!(regions.len(), 1);
    assert_eq!(
        regions[0].language,
        RegionLanguage::Unsupported("typescript".to_string())
    );
    let source = load("Ts.svelte", text);
    assert!(source.region_trees().is_empty());
    assert_eq!(
        rows(&source),
        vec![row(
            "module",
            "<script lang=\"typescript\">",
            "1:1-3:10",
            "1:1-1:27"
        )]
    );
}

#[test]
fn module_svelte_has_two_typescript_regions() {
    let text = "<script module lang=\"ts\">\n  export const x = 1;\n</script>\n<script lang=\"ts\" generics=\"T\">\n  let { items }: { items: T[] } = $props();\n</script>\n";
    let regions = regions_of(text);
    assert_eq!(regions.len(), 2);
    for region in &regions {
        assert_eq!(region.kind, RegionKind::Script);
        assert_eq!(
            region.language,
            RegionLanguage::Supported(Grammar::TypeScript)
        );
        assert_eq!(region.state, RegionState::WellFormed);
    }
    let source = load("Module.svelte", text);
    assert_eq!(source.region_trees().len(), 2);
    assert!(!source.has_any_error());
    let got = rows(&source);
    assert_eq!(
        blocks(&got),
        vec![
            row(
                "module",
                "<script module lang=\"ts\">",
                "1:1-3:10",
                "1:1-1:26"
            ),
            row(
                "module",
                "<script lang=\"ts\" generics=\"T\">",
                "4:1-6:10",
                "4:1-4:32"
            ),
        ]
    );
    assert!(
        got.iter().any(|r| r.0 == "variable" && r.1 == "x"),
        "{got:#?}"
    );
}

#[test]
fn nested_svelte_script_inside_markup_is_no_region() {
    let text = "<svelte:head><script>var g = 1;</script></svelte:head>\n";
    assert!(regions_of(text).is_empty());
    let source = load("Nested.svelte", text);
    assert_eq!(
        rows(&source),
        vec![row("element", "svelte:head", "1:1-1:55", "1:2-1:13")]
    );
    assert!(!symbol_rows(&source).iter().any(|s| s.1 == "g"));
}

#[test]
fn class_svelte_value_holding_an_expression_is_not_reported() {
    let text = "<p class=\"a {x}\" id=\"i\">t</p>\n";
    let source = load("Class.svelte", text);
    assert_eq!(
        set(&rows(&source)),
        set(&[
            row("element", "p", "1:1-1:30", "1:2-1:3"),
            row("field", "i", "1:18-1:24", "1:22-1:23"),
        ])
    );
    assert_eq!(
        set(&symbol_rows(&source)),
        set(&[
            symbol("Class", "p", "1:2-1:3"),
            symbol("Field", "i", "1:22-1:23"),
        ])
    );
}

#[test]
fn expr_svelte_template_expression_is_not_a_symbol() {
    let text = "<p>{onlyhere}</p>\n";
    let source = load("Expr.svelte", text);
    assert_eq!(
        rows(&source),
        vec![row("element", "p", "1:1-1:18", "1:2-1:3")]
    );
    assert!(!symbol_rows(&source).iter().any(|s| s.1 == "onlyhere"));
}

/// The first ERROR or MISSING node of a tree in document order.
fn first_error(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    children.into_iter().find_map(first_error)
}

fn compact_of(node: tree_sitter::Node) -> String {
    CompactRange::new(
        node.start_position().row as u32 + 1,
        node.start_position().column as u32 + 1,
        node.end_position().row as u32 + 1,
        node.end_position().column as u32 + 1,
    )
    .to_string()
}

#[test]
fn amp_svelte_error_in_markup_leaves_the_script_region_parsed() {
    let text = "<p>Set & go</p>\n<script>function f() {}</script>\n";
    let source = load("Amp.svelte", text);
    let error = first_error(source.tree().root_node()).unwrap();
    assert_eq!(compact_of(error), "1:8-1:12");
    assert!(source.has_any_error());
    let regions = source.regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].state, RegionState::WellFormed);
    assert_eq!(
        regions[0].language,
        RegionLanguage::Supported(Grammar::JavaScript)
    );
    assert_eq!(source.region_trees().len(), 1);
    assert!(
        symbol_rows(&source).contains(&symbol("Function", "f", "2:18-2:19")),
        "{:#?}",
        symbol_rows(&source)
    );
    assert!(rows(&source).contains(&row("module", "<script>", "2:1-2:33", "2:1-2:9")));
}

#[test]
fn two_symbols_of_one_name_in_two_regions_give_the_first_by_range_start() {
    let text = "<script context=\"module\">\n  function f() {}\n</script>\n<script>\n  function f() {}\n</script>\n";
    let source = load("Two.svelte", text);
    let QueryResult::Symbol(found) = execute_symbol(&source, "f").unwrap() else {
        panic!("not a symbol");
    };
    assert_eq!(
        CompactRange::from(found.symbol.range()).to_string(),
        "2:12-2:13"
    );
    let all: Vec<SymbolRow> = symbol_rows(&source)
        .into_iter()
        .filter(|s| s.1 == "f")
        .collect();
    assert_eq!(
        all,
        vec![
            symbol("Function", "f", "2:12-2:13"),
            symbol("Function", "f", "5:12-5:13"),
        ]
    );
}

/// The `(text, range)` of each node the Svelte `references.scm` captures.
fn reference_sites(text: &str) -> BTreeSet<(String, String)> {
    let support = Grammar::Svelte.language_support();
    let tree = outer_tree(text);
    let query =
        tree_sitter::Query::new(&support.ts_language(), support.references_query()).unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());
    let mut sites = BTreeSet::new();
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if query.capture_names()[capture.index as usize] == "reference" {
                let node = capture.node;
                sites.insert((text[node.byte_range()].to_string(), compact_of(node)));
            }
        }
    }
    sites
}

#[test]
fn references_query_matches_id_class_data_values_and_end_tag_names() {
    let sites = reference_sites(&counter());
    let want: BTreeSet<(String, String)> = [
        ("counter", "15:13-15:20"),
        ("p", "5:13-5:14"),
        ("div", "21:3-21:6"),
        ("button", "16:44-16:50"),
        ("span", "18:30-18:34"),
        ("i", "20:36-20:37"),
    ]
    .iter()
    .map(|(t, r)| (t.to_string(), r.to_string()))
    .collect();
    assert_eq!(sites, want);

    let text = "<p data-x=\"v\" id=u class=\"a b\">t</p>\n<a href=\"#u\" class=\"c {y}\">l</a>\n";
    let sites = reference_sites(text);
    let want: BTreeSet<(String, String)> = [
        ("v", "1:12-1:13"),
        ("u", "1:18-1:19"),
        ("a b", "1:27-1:30"),
        ("p", "1:35-1:36"),
        ("#u", "2:10-2:12"),
        ("a", "2:31-2:32"),
    ]
    .iter()
    .map(|(t, r)| (t.to_string(), r.to_string()))
    .collect();
    assert_eq!(sites, want);
}

#[test]
fn empty_elements_are_empty_regions_reported_as_blocks() {
    let text = "<script></script>\n<style></style>\n<script src=\"/x.js\"></script>\n";
    let regions = regions_of(text);
    assert_eq!(regions.len(), 3);
    for region in &regions {
        assert_eq!(region.state, RegionState::Empty, "{region:?}");
        assert_eq!(region.content, None);
    }
    let source = load("Empty.svelte", text);
    assert!(source.region_trees().is_empty());
    assert!(!source.has_any_error());
    assert_eq!(
        rows(&source),
        vec![
            row("module", "<script>", "1:1-1:18", "1:1-1:9"),
            row("module", "<style>", "2:1-2:16", "2:1-2:8"),
            row("module", "<script src=\"/x.js\">", "3:1-3:30", "3:1-3:21"),
        ]
    );
}

#[test]
fn an_unclosed_script_is_a_malformed_region_and_reports_nothing() {
    let text = "<script>\nlet a = 1;\n<p>x</p>\n";
    let regions = regions_of(text);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].kind, RegionKind::Script);
    assert_eq!(regions[0].state, RegionState::Malformed);
    assert_eq!(regions[0].content, None);
    assert_eq!(regions[0].start_tag.to_string(), "1:1-1:9");
    let source = load("Unclosed.svelte", text);
    assert!(source.region_trees().is_empty());
    assert!(source.has_any_error());
    assert!(rows(&source).is_empty(), "{:#?}", rows(&source));
}

#[test]
fn style_and_script_languages_follow_lang() {
    let text = "<script lang=\"js\">let a = 1;</script>\n<script lang=ts>let b = 1;</script>\n<script lang=\"coffee\">c = 1</script>\n<style lang=\"scss\">a { b: c; }</style>\n<style lang=\"css\">a { b: c; }</style>\n<style lang=\"less\">a { b: c; }</style>\n<script lang=\"ts\" lang=\"js\">let d: number = 1;</script>\n";
    let languages: Vec<(RegionKind, RegionLanguage)> = regions_of(text)
        .into_iter()
        .map(|r| (r.kind, r.language))
        .collect();
    assert_eq!(
        languages,
        vec![
            (
                RegionKind::Script,
                RegionLanguage::Supported(Grammar::JavaScript)
            ),
            (
                RegionKind::Script,
                RegionLanguage::Supported(Grammar::TypeScript)
            ),
            (
                RegionKind::Script,
                RegionLanguage::Unsupported("coffee".to_string())
            ),
            (RegionKind::Style, RegionLanguage::Supported(Grammar::Scss)),
            (RegionKind::Style, RegionLanguage::Supported(Grammar::Css)),
            (
                RegionKind::Style,
                RegionLanguage::Unsupported("less".to_string())
            ),
            (
                RegionKind::Script,
                RegionLanguage::Supported(Grammar::TypeScript)
            ),
        ]
    );
}
