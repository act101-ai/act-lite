//! The embedded-region model, tested with a detector of this file that marks
//! byte ranges of a synthetic component file as regions, so the model holds
//! before any framework detector finds a region.

use act_lite_core::CompactRange;
use act_lite_parser::regions::{
    Detector, Region, RegionKind, RegionLanguage, RegionState, RegionTree, view,
};
use act_lite_parser::{
    Grammar, ParseOptions, Parser, ParserError, QueryResult, SourceFile, execute_skeleton,
    execute_symbols,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// A synthetic Svelte file: a module script on lines 1-3, and an instance
/// script that starts on line 5 after multi-byte text (`ä` is two bytes).
const COMPONENT: &str = "<script context=\"module\" lang=\"ts\">
  export const prerender = true;
</script>

<p>Zähler</p><script lang=\"ts\">export let start: number = 0;
  let count = 0;

  function increment(): void {
    count += 1;
  }
</script>

<style>
  .counter { color: red; }
</style>
";

/// Returns the regions it was built with, whatever the tree holds.
struct Fixed(Vec<Region>);

impl Detector for Fixed {
    fn regions(&self, _tree: &tree_sitter::Tree, _source: &str) -> Vec<Region> {
        self.0.clone()
    }
}

/// 1-based line and byte column of `byte` in `source`.
fn point(source: &str, byte: usize) -> (u32, u32) {
    let before = &source[..byte];
    let line = before.matches('\n').count() as u32 + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, (byte - line_start) as u32 + 1)
}

fn compact(source: &str, range: std::ops::Range<usize>) -> CompactRange {
    let (sl, sc) = point(source, range.start);
    let (el, ec) = point(source, range.end);
    CompactRange::new(sl, sc, el, ec)
}

/// The region of the `nth` element (0-based) that opens with `start_tag`
/// and closes with `end_tag`.
fn region(
    source: &str,
    start_tag: &str,
    end_tag: &str,
    nth: usize,
    kind: RegionKind,
    grammar: Grammar,
) -> Region {
    let open = source
        .match_indices(start_tag)
        .nth(nth)
        .unwrap_or_else(|| panic!("no {start_tag} #{nth}"))
        .0;
    let content_start = open + start_tag.len();
    let close = content_start + source[content_start..].find(end_tag).unwrap();
    let element_end = close + end_tag.len();
    Region {
        kind,
        language: RegionLanguage::Supported(grammar),
        content: Some(content_start..close),
        element: compact(source, open..element_end),
        start_tag: compact(source, open..content_start),
        state: RegionState::WellFormed,
    }
}

fn module_script(source: &str) -> Region {
    region(
        source,
        "<script context=\"module\" lang=\"ts\">",
        "</script>",
        0,
        RegionKind::Script,
        Grammar::TypeScript,
    )
}

fn instance_script(source: &str) -> Region {
    region(
        source,
        "<script lang=\"ts\">",
        "</script>",
        0,
        RegionKind::Script,
        Grammar::TypeScript,
    )
}

fn style(source: &str) -> Region {
    region(
        source,
        "<style>",
        "</style>",
        0,
        RegionKind::Style,
        Grammar::Css,
    )
}

fn load(source: &str, regions: Vec<Region>) -> SourceFile {
    load_with(source, regions, ParseOptions::default())
}

fn load_with(source: &str, regions: Vec<Region>, mut options: ParseOptions) -> SourceFile {
    options.detector = Some(Box::new(Fixed(regions)));
    SourceFile::from_content_with(
        "Counter.svelte",
        source.to_string(),
        Grammar::Svelte,
        options,
    )
    .unwrap()
}

/// The first node of `kind` in document order.
fn find<'t>(node: tree_sitter::Node<'t>, kind: &str) -> Option<tree_sitter::Node<'t>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    children.into_iter().find_map(|child| find(child, kind))
}

fn span(node: tree_sitter::Node) -> (usize, usize, tree_sitter::Point, tree_sitter::Point) {
    (
        node.start_byte(),
        node.end_byte(),
        node.start_position(),
        node.end_position(),
    )
}

/// A declaration without its file: kind, name, range and name range with
/// their byte offsets.
type Row = (String, String, String, Option<usize>, Option<String>);

fn rows(result: QueryResult) -> Vec<Row> {
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
                d.range.start().byte_offset(),
                d.name_range.map(|r| CompactRange::from(&r).to_string()),
            )
        })
        .collect()
}

type SymbolRow = (String, String, String, Option<usize>);

fn symbol_rows(result: QueryResult) -> Vec<SymbolRow> {
    let QueryResult::Symbols { symbols } = result else {
        panic!("not symbols");
    };
    symbols
        .into_iter()
        .map(|s| {
            (
                format!("{:?}", s.kind()),
                s.name().to_string(),
                CompactRange::from(s.range()).to_string(),
                s.range().start().byte_offset(),
            )
        })
        .collect()
}

fn view_file(source: &str, region: &Region, path: &str, grammar: Grammar) -> SourceFile {
    SourceFile::from_content(path, view(source, region), grammar).unwrap()
}

/// Sorted by range start, then by range end descending.
fn assert_document_order(ranges: &[CompactRange]) {
    let key = |r: &CompactRange| {
        (
            r.start_line(),
            r.start_col(),
            std::cmp::Reverse((r.end_line(), r.end_col())),
        )
    };
    for pair in ranges.windows(2) {
        assert!(key(&pair[0]) <= key(&pair[1]), "out of order: {pair:?}");
    }
}

fn parse_compact(text: &str) -> CompactRange {
    serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
}

#[test]
fn a_region_tree_carries_the_positions_of_its_view() {
    let instance = instance_script(COMPONENT);
    assert_eq!(instance.start_tag.start_line(), 5);
    // `Zähler` holds a two-byte character: the byte column is not the
    // character column.
    let line5 = COMPONENT.lines().nth(4).unwrap();
    let char_col = line5[..line5.find("<script").unwrap()].chars().count() as u32 + 1;
    assert_eq!(instance.start_tag.start_col(), char_col + 1);

    let source = load(COMPONENT, vec![instance.clone()]);
    let trees = source.region_trees();
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0].region, 0);
    assert_eq!(trees[0].grammar, Grammar::TypeScript);

    let in_file = find(trees[0].tree.root_node(), "function_declaration").unwrap();
    let viewed = view_file(COMPONENT, &instance, "view.ts", Grammar::TypeScript);
    let in_view = find(viewed.tree().root_node(), "function_declaration").unwrap();
    assert_eq!(span(in_file), span(in_view));
    assert_eq!(
        &COMPONENT[in_file.start_byte()..in_file.end_byte()],
        "function increment(): void {\n    count += 1;\n  }"
    );
    // The root lies within the region's content.
    let content = instance.content.clone().unwrap();
    let root = trees[0].tree.root_node();
    assert!(root.start_byte() >= content.start && root.end_byte() <= content.end);
}

#[test]
fn skeleton_and_symbols_merge_blocks_and_region_declarations() {
    const TAGS: [&str; 2] = [
        "<script context=\"module\" lang=\"ts\">",
        "<script lang=\"ts\">",
    ];
    let regions = vec![module_script(COMPONENT), instance_script(COMPONENT)];
    let source = load(COMPONENT, regions.clone());
    let outer = load(COMPONENT, Vec::new());

    let merged = rows(execute_skeleton(&source).unwrap());
    let mut expected = rows(execute_skeleton(&outer).unwrap());
    for (region, tag) in regions.iter().zip(TAGS) {
        expected.push((
            "module".to_string(),
            tag.to_string(),
            region.element.to_string(),
            COMPONENT.find(tag),
            Some(region.start_tag.to_string()),
        ));
        expected.extend(rows(
            execute_skeleton(&view_file(
                COMPONENT,
                region,
                "view.ts",
                Grammar::TypeScript,
            ))
            .unwrap(),
        ));
    }
    let mut sorted_merged = merged.clone();
    sorted_merged.sort();
    expected.sort();
    assert_eq!(sorted_merged, expected);
    assert!(
        merged
            .iter()
            .any(|r| r.0 == "module" && r.1 == "<script lang=\"ts\">")
    );
    assert!(
        merged
            .iter()
            .any(|r| r.0 == "function" && r.1 == "increment")
    );
    let ranges: Vec<CompactRange> = merged.iter().map(|r| parse_compact(&r.2)).collect();
    assert_document_order(&ranges);
    // A block comes before its own content.
    let block = merged
        .iter()
        .position(|r| r.1 == "<script lang=\"ts\">")
        .unwrap();
    let function = merged.iter().position(|r| r.1 == "increment").unwrap();
    assert!(block < function);

    let merged = symbol_rows(execute_symbols(&source).unwrap());
    let mut expected = symbol_rows(execute_symbols(&outer).unwrap());
    for (region, tag) in regions.iter().zip(TAGS) {
        expected.push((
            "Module".to_string(),
            tag.to_string(),
            region.start_tag.to_string(),
            COMPONENT.find(tag),
        ));
        expected.extend(symbol_rows(
            execute_symbols(&view_file(
                COMPONENT,
                region,
                "view.ts",
                Grammar::TypeScript,
            ))
            .unwrap(),
        ));
    }
    let mut sorted_merged = merged.clone();
    sorted_merged.sort();
    expected.sort();
    assert_eq!(sorted_merged, expected);
    assert!(merged.iter().any(|s| s.1 == "increment"));
    let ranges: Vec<CompactRange> = merged.iter().map(|s| parse_compact(&s.2)).collect();
    assert_document_order(&ranges);
}

#[test]
fn parse_error_is_set_when_only_a_region_tree_has_an_error() {
    let clean = load(COMPONENT, vec![instance_script(COMPONENT)]);
    assert!(!clean.tree().root_node().has_error());
    assert!(!clean.has_any_error());

    let broken = COMPONENT.replace("let count = 0;", "let count = ;");
    let source = load(&broken, vec![instance_script(&broken)]);
    assert!(!source.tree().root_node().has_error());
    assert!(source.region_trees()[0].tree.root_node().has_error());
    assert!(source.has_any_error());
}

#[test]
fn a_region_parse_past_the_deadline_has_no_tree_and_keeps_its_block() {
    let mut body = String::new();
    for i in 0..20_000 {
        body.push_str(&format!("  const v{i}: number = {i} + {i};\n"));
    }
    let text = format!("<p>Zähler</p><script lang=\"ts\">\n{body}</script>\n");
    let script = instance_script(&text);
    let options = ParseOptions {
        timeout: Some(Duration::from_nanos(1)),
        ..ParseOptions::default()
    };
    let source = load_with(&text, vec![script.clone()], options);
    assert_eq!(source.regions().len(), 1);
    assert!(source.region_trees().is_empty());
    assert!(!source.tree().root_node().has_error());
    assert!(source.has_any_error());
    let declarations = rows(execute_skeleton(&source).unwrap());
    assert!(
        declarations
            .iter()
            .any(|r| r.0 == "module" && r.2 == script.element.to_string()),
        "{declarations:?}"
    );
    assert!(!declarations.iter().any(|r| r.0 == "variable"));
}

#[test]
fn two_regions_of_one_file_are_parsed_separately() {
    let broken = COMPONENT.replace("export const prerender = true;", "export const = (;");
    let regions = vec![module_script(&broken), instance_script(&broken)];
    let source = load(&broken, regions.clone());
    let trees = source.region_trees();
    assert_eq!(trees.len(), 2);
    assert_eq!((trees[0].region, trees[1].region), (0, 1));
    assert!(trees[0].tree.root_node().has_error());
    assert!(!trees[1].tree.root_node().has_error());
    // The second tree is the tree of its own view, not a continuation of
    // the first region.
    let viewed = view_file(&broken, &regions[1], "view.ts", Grammar::TypeScript);
    assert_eq!(
        trees[1].tree.root_node().to_sexp(),
        viewed.tree().root_node().to_sexp()
    );
}

#[test]
fn region_trees_come_from_parse_region_without_a_text_copy() {
    let instance = instance_script(COMPONENT);
    let content = instance.content.clone().unwrap();
    let tree: tree_sitter::Tree = Parser::new()
        .parse_region(COMPONENT, Grammar::TypeScript, content)
        .unwrap();
    let source = load(COMPONENT, vec![instance]);
    let RegionTree {
        tree: from_file, ..
    } = &source.region_trees()[0];
    assert_eq!(tree.root_node().to_sexp(), from_file.root_node().to_sexp());
    assert_eq!(span(tree.root_node()), span(from_file.root_node()));
    // One copy of the content: the tree reads the same `String`.
    assert!(std::ptr::eq(
        source.content().as_ptr(),
        source.tree().source().as_ptr()
    ));
}

#[test]
fn an_error_in_a_style_region_sets_has_any_error() {
    let broken = COMPONENT.replace(".counter { color: red; }", ".counter { color: red; } } {");
    let source = load(&broken, vec![instance_script(&broken), style(&broken)]);
    let trees = source.region_trees();
    assert_eq!(trees.len(), 2);
    assert_eq!(trees[1].grammar, Grammar::Css);
    assert!(!trees[0].tree.root_node().has_error());
    assert!(trees[1].tree.root_node().has_error());
    assert!(!source.tree().root_node().has_error());
    assert!(source.has_any_error());
}

#[test]
fn a_stop_after_the_first_region_parse_cancels_the_load() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let options = ParseOptions {
        detector: Some(Box::new(Fixed(vec![
            module_script(COMPONENT),
            instance_script(COMPONENT),
        ]))),
        stop: Some(Box::new(move || seen.fetch_add(1, Ordering::SeqCst) >= 1)),
        ..ParseOptions::default()
    };
    let result = SourceFile::from_content_with(
        "Counter.svelte",
        COMPONENT.to_string(),
        Grammar::Svelte,
        options,
    );
    assert!(matches!(result, Err(ParserError::Cancelled)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let options = ParseOptions {
        detector: Some(Box::new(Fixed(vec![
            module_script(COMPONENT),
            instance_script(COMPONENT),
        ]))),
        stop: Some(Box::new(|| false)),
        ..ParseOptions::default()
    };
    let source = SourceFile::from_content_with(
        "Counter.svelte",
        COMPONENT.to_string(),
        Grammar::Svelte,
        options,
    )
    .unwrap();
    assert_eq!(source.region_trees().len(), 2);
}

#[test]
fn regions_that_are_not_parsed_have_no_tree() {
    let mut unsupported = instance_script(COMPONENT);
    unsupported.language = RegionLanguage::Unsupported("coffee".to_string());
    let mut malformed = module_script(COMPONENT);
    malformed.state = RegionState::Malformed;
    malformed.content = None;
    let source = load(COMPONENT, vec![malformed, unsupported.clone()]);
    assert_eq!(source.regions().len(), 2);
    assert!(source.region_trees().is_empty());
    assert!(!source.has_any_error());
    // The unsupported region is still a block; the malformed one is not.
    let declarations = rows(execute_skeleton(&source).unwrap());
    let blocks: Vec<&Row> = declarations.iter().filter(|r| r.0 == "module").collect();
    assert_eq!(blocks.len(), 1, "{declarations:?}");
    assert_eq!(blocks[0].2, unsupported.element.to_string());
}

#[test]
fn a_file_that_is_not_a_component_has_no_regions() {
    let options = ParseOptions {
        detector: Some(Box::new(Fixed(vec![instance_script(COMPONENT)]))),
        ..ParseOptions::default()
    };
    let source = SourceFile::from_content_with(
        "a.ts",
        "function f() {}\n".to_string(),
        Grammar::TypeScript,
        options,
    )
    .unwrap();
    assert!(source.regions().is_empty());
    assert!(source.region_trees().is_empty());
    let plain =
        SourceFile::from_content("a.ts", "function f() {}\n".to_string(), Grammar::TypeScript)
            .unwrap();
    assert!(plain.regions().is_empty());
}
