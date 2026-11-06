//! The regions of an Astro component file.
//!
//! The frontmatter is TypeScript. Every `<script>` at any depth is a region
//! unless its `type` makes it a data block: with no attribute, or only
//! `src`, it is TypeScript (Astro processes it); with any other attribute
//! it is JavaScript (Astro sends it as written). Every `<style>` at any
//! depth is a region, its language chosen by `lang`.

use super::{Detector, Region, RegionKind, RegionLanguage, RegionState};
use crate::Grammar;
use act_lite_core::CompactRange;
use tree_sitter::Node;

/// `module` and the JavaScript MIME type essence matches of the HTML
/// Living Standard: a script `type` that names JavaScript.
const JAVASCRIPT_TYPES: &[&str] = &[
    "module",
    "application/ecmascript",
    "application/javascript",
    "application/x-ecmascript",
    "application/x-javascript",
    "text/ecmascript",
    "text/javascript",
    "text/javascript1.0",
    "text/javascript1.1",
    "text/javascript1.2",
    "text/javascript1.3",
    "text/javascript1.4",
    "text/javascript1.5",
    "text/jscript",
    "text/livescript",
    "text/x-ecmascript",
    "text/x-javascript",
];

/// Finds the frontmatter, script and style regions of an Astro file.
#[derive(Debug, Clone, Copy, Default)]
pub struct AstroDetector;

impl Detector for AstroDetector {
    fn regions(&self, tree: &tree_sitter::Tree, source: &str) -> Vec<Region> {
        let mut regions = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            let region = match node.kind() {
                "frontmatter" => Some(frontmatter(node)),
                "script_element" => script(node, source),
                "style_element" => Some(style(node, source)),
                "start_tag" | "self_closing_tag" if node.parent().is_some_and(|p| p.is_error()) => {
                    unclosed(node, source)
                }
                _ => None,
            };
            regions.extend(region);
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return regions;
                }
            }
        }
    }
}

fn compact(node: Node) -> CompactRange {
    let (start, end) = (node.start_position(), node.end_position());
    CompactRange::new(
        start.row as u32 + 1,
        start.column as u32 + 1,
        end.row as u32 + 1,
        end.column as u32 + 1,
    )
}

fn child<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|c| c.kind() == kind)
}

fn under_error(node: Node) -> bool {
    std::iter::successors(node.parent(), Node::parent).any(|n| n.is_error())
}

/// Malformed when an ERROR lies above the element or an ERROR or MISSING
/// node in it; else empty when it has no content, or zero-width content;
/// else well-formed.
fn state(element: Node, content: Option<Node>) -> RegionState {
    if element.has_error() || under_error(element) {
        RegionState::Malformed
    } else if content.is_none_or(|c| c.start_byte() == c.end_byte()) {
        RegionState::Empty
    } else {
        RegionState::WellFormed
    }
}

fn region(
    kind: RegionKind,
    language: RegionLanguage,
    element: Node,
    start_tag: Node,
    content: Option<Node>,
) -> Region {
    let state = state(element, content);
    Region {
        kind,
        language,
        content: content
            .filter(|_| state == RegionState::WellFormed)
            .map(|c| c.byte_range()),
        element: compact(element),
        start_tag: compact(start_tag),
        state,
    }
}

/// The frontmatter, from its opening `---` to its closing one.
fn frontmatter(node: Node) -> Region {
    let opening = node.child(0).unwrap_or(node);
    region(
        RegionKind::Frontmatter,
        RegionLanguage::Supported(Grammar::TypeScript),
        node,
        opening,
        child(node, "frontmatter_js_block"),
    )
}

/// Each attribute of a start tag as (name, value as written); an attribute
/// without a literal value has the value "".
fn attributes<'s>(tag: Node, source: &'s str) -> Vec<(&'s str, &'s str)> {
    let text = |n: Node| n.utf8_text(source.as_bytes()).unwrap_or_default();
    let mut cursor = tag.walk();
    tag.children(&mut cursor)
        .filter(|c| c.kind() == "attribute")
        .map(|attribute| {
            let name = child(attribute, "attribute_name").map_or("", text);
            let value = child(attribute, "attribute_value")
                .or_else(|| {
                    child(attribute, "quoted_attribute_value")
                        .and_then(|q| child(q, "attribute_value"))
                })
                .map_or("", text);
            (name, value)
        })
        .collect()
}

/// The value of the first attribute named `name`, as written.
fn first<'s>(attributes: &[(&str, &'s str)], name: &str) -> Option<&'s str> {
    attributes.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// Whether a script `type` value makes the element a data block: not
/// empty, not `module` and not a JavaScript MIME type, after leading and
/// trailing ASCII whitespace is removed, compared ASCII case-insensitively.
fn is_data_block(value: &str) -> bool {
    let value = value.trim_matches(|c| matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' '));
    !value.is_empty()
        && !JAVASCRIPT_TYPES
            .iter()
            .any(|t| t.eq_ignore_ascii_case(value))
}

/// The language of a script that is not a data block, or `None` for a
/// data block.
fn script_language(attributes: &[(&str, &str)]) -> Option<RegionLanguage> {
    if first(attributes, "type").is_some_and(is_data_block) {
        return None;
    }
    let grammar = if attributes.iter().all(|(name, _)| *name == "src") {
        Grammar::TypeScript
    } else {
        Grammar::JavaScript
    };
    Some(RegionLanguage::Supported(grammar))
}

fn style_language(attributes: &[(&str, &str)]) -> RegionLanguage {
    match first(attributes, "lang") {
        None | Some("css") => RegionLanguage::Supported(Grammar::Css),
        Some("scss") => RegionLanguage::Supported(Grammar::Scss),
        Some(other) => RegionLanguage::Unsupported(other.to_string()),
    }
}

fn start_tag(element: Node) -> Option<Node> {
    child(element, "start_tag").or_else(|| child(element, "self_closing_tag"))
}

fn script(node: Node, source: &str) -> Option<Region> {
    let tag = start_tag(node)?;
    let language = script_language(&attributes(tag, source))?;
    Some(region(
        RegionKind::Script,
        language,
        node,
        tag,
        child(node, "raw_text"),
    ))
}

fn style(node: Node, source: &str) -> Region {
    let tag = start_tag(node).unwrap_or(node);
    region(
        RegionKind::Style,
        style_language(&attributes(tag, source)),
        node,
        tag,
        child(node, "raw_text"),
    )
}

/// A `<script>` or `<style>` start tag that the grammar put in an ERROR
/// node with no element around it (an unclosed element): a malformed
/// region with its start tag only. A data block is never a region.
fn unclosed(tag: Node, source: &str) -> Option<Region> {
    let name = child(tag, "tag_name")?.utf8_text(source.as_bytes()).ok()?;
    let attributes = attributes(tag, source);
    let (kind, language) = match name {
        "script" => (RegionKind::Script, script_language(&attributes)?),
        "style" => (RegionKind::Style, style_language(&attributes)),
        _ => return None,
    };
    Some(Region {
        kind,
        language,
        content: None,
        element: compact(tag),
        start_tag: compact(tag),
        state: RegionState::Malformed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regions(text: &str) -> Vec<Region> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&Grammar::Astro.language_support().ts_language())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        AstroDetector.regions(&tree, text)
    }

    #[test]
    fn finds_the_frontmatter_script_and_style_in_document_order() {
        let text = "---\nconst a = 1;\n---\n<script>let a = 1;</script>\n<style>p { color: red; }</style>\n";
        let kinds: Vec<_> = regions(text).iter().map(|r| (r.kind, r.state)).collect();
        assert_eq!(
            kinds,
            [
                (RegionKind::Frontmatter, RegionState::WellFormed),
                (RegionKind::Script, RegionState::WellFormed),
                (RegionKind::Style, RegionState::WellFormed),
            ]
        );
        let script = &regions(text)[1];
        assert_eq!(&text[script.content.clone().unwrap()], "let a = 1;");
    }

    #[test]
    fn data_block_types_are_trimmed_and_compared_ascii_case_insensitively() {
        for value in [
            "",
            "  ",
            " module ",
            "TEXT/JAVASCRIPT",
            "\ttext/ecmascript\r\n",
        ] {
            assert!(!is_data_block(value), "{value:?}");
        }
        for value in [
            "text/typescript",
            "application/ld+json",
            "text/javascript; charset=utf-8",
            "text/javaſcript",
            "\u{a0}module",
        ] {
            assert!(is_data_block(value), "{value:?}");
        }
    }
}
