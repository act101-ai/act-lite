//! The regions of a Vue component file.
//!
//! Each `<script>` and `<style>` element that is a child of the document is
//! a region; a `<script>` or `<style>` below the top level is not (Vue does
//! not process it). The `lang` attribute selects the language; when it is
//! given twice the first counts. Inside an ERROR node of the document, a
//! script or style start tag is a malformed region when its nearest
//! ancestor that is not an ERROR node is the document.

use super::{Detector, Region, RegionKind, RegionLanguage, RegionState};
use crate::Grammar;
use act_lite_core::CompactRange;

/// Finds the script and style regions of a Vue file.
#[derive(Debug, Clone, Copy, Default)]
pub struct VueDetector;

impl Detector for VueDetector {
    fn regions(&self, tree: &tree_sitter::Tree, source: &str) -> Vec<Region> {
        let mut regions = Vec::new();
        top_level(tree.root_node(), source, false, &mut regions);
        regions
    }
}

/// The regions among the children of `parent`: the document, or an ERROR
/// node whose nearest ancestor that is not an ERROR node is the document
/// (`in_error`).
fn top_level(parent: tree_sitter::Node, source: &str, in_error: bool, out: &mut Vec<Region>) {
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        match child.kind() {
            "script_element" | "style_element" => {
                if let Some(region) = element_region(child, source, in_error) {
                    out.push(region);
                }
            }
            "start_tag" if in_error => {
                if let Some(kind) = region_kind(child, source) {
                    out.push(Region {
                        kind,
                        language: language(kind, child, source),
                        content: None,
                        element: compact(child),
                        start_tag: compact(child),
                        state: RegionState::Malformed,
                    });
                }
            }
            "ERROR" => top_level(child, source, true, out),
            _ => {}
        }
    }
}

/// The region of a `script_element` or `style_element`.
fn element_region(element: tree_sitter::Node, source: &str, in_error: bool) -> Option<Region> {
    let kind = if element.kind() == "script_element" {
        RegionKind::Script
    } else {
        RegionKind::Style
    };
    let start_tag = child_of_kind(element, "start_tag")?;
    let raw_text = child_of_kind(element, "raw_text");
    let end_tag = child_of_kind(element, "end_tag");
    let has_content = raw_text.is_some_and(|text| text.end_byte() > text.start_byte());
    let clean = !in_error && !element.has_error();
    let state = if !has_content && clean {
        RegionState::Empty
    } else if clean
        && !start_tag.is_missing()
        && raw_text.is_some_and(|text| !text.is_missing())
        && end_tag.is_some_and(|tag| !tag.is_missing())
    {
        RegionState::WellFormed
    } else {
        RegionState::Malformed
    };
    let content = match state {
        RegionState::WellFormed => raw_text.map(|text| text.byte_range()),
        RegionState::Empty | RegionState::Malformed => None,
    };
    Some(Region {
        kind,
        language: language(kind, start_tag, source),
        content,
        element: compact(element),
        start_tag: compact(start_tag),
        state,
    })
}

/// Script or style, by the tag name of a start tag; `None` for any other
/// tag.
fn region_kind(start_tag: tree_sitter::Node, source: &str) -> Option<RegionKind> {
    match text(child_of_kind(start_tag, "tag_name")?, source) {
        "script" => Some(RegionKind::Script),
        "style" => Some(RegionKind::Style),
        _ => None,
    }
}

/// The language that the first `lang` attribute of `start_tag` selects.
fn language(kind: RegionKind, start_tag: tree_sitter::Node, source: &str) -> RegionLanguage {
    let lang = lang_value(start_tag, source);
    let grammar = match (kind, lang) {
        (RegionKind::Script, None | Some("js")) => Some(Grammar::JavaScript),
        (RegionKind::Script, Some("jsx")) => Some(Grammar::Jsx),
        (RegionKind::Script, Some("ts")) => Some(Grammar::TypeScript),
        (RegionKind::Script, Some("tsx")) => Some(Grammar::Tsx),
        (RegionKind::Style, None | Some("css")) => Some(Grammar::Css),
        (RegionKind::Style, Some("scss")) => Some(Grammar::Scss),
        _ => None,
    };
    match grammar {
        Some(grammar) => RegionLanguage::Supported(grammar),
        None => RegionLanguage::Unsupported(lang.unwrap_or_default().to_string()),
    }
}

/// The value of the first `lang` attribute of a `<template>` element's
/// start tag; `None` without one.
pub(crate) fn template_lang<'s>(template: tree_sitter::Node, source: &'s str) -> Option<&'s str> {
    lang_value(child_of_kind(template, "start_tag")?, source)
}

/// The value of the first `lang` attribute of `start_tag`, as written
/// (empty for an attribute without a value); `None` without one.
fn lang_value<'s>(start_tag: tree_sitter::Node, source: &'s str) -> Option<&'s str> {
    let mut cursor = start_tag.walk();
    let attribute = start_tag.children(&mut cursor).find(|child| {
        child.kind() == "attribute"
            && child_of_kind(*child, "attribute_name")
                .is_some_and(|name| text(name, source) == "lang")
    })?;
    let value = child_of_kind(attribute, "attribute_value").or_else(|| {
        child_of_kind(attribute, "quoted_attribute_value")
            .and_then(|quoted| child_of_kind(quoted, "attribute_value"))
    });
    Some(value.map_or("", |value| text(value, source)))
}

fn child_of_kind<'t>(node: tree_sitter::Node<'t>, kind: &str) -> Option<tree_sitter::Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn text<'s>(node: tree_sitter::Node, source: &'s str) -> &'s str {
    source.get(node.byte_range()).unwrap_or_default()
}

/// 1-based line and byte column of a node's start and end.
fn compact(node: tree_sitter::Node) -> CompactRange {
    let (start, end) = (node.start_position(), node.end_position());
    CompactRange::new(
        start.row as u32 + 1,
        start.column as u32 + 1,
        end.row as u32 + 1,
        end.column as u32 + 1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regions(text: &str) -> Vec<Region> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&Grammar::Vue.language_support().ts_language())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        VueDetector.regions(&tree, text)
    }

    #[test]
    fn finds_the_top_level_script_and_style() {
        let text = "<script>let a = 1;</script>\n<style>p { color: red; }</style>\n";
        let found = regions(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, RegionKind::Script);
        assert_eq!(
            found[0].language,
            RegionLanguage::Supported(Grammar::JavaScript)
        );
        assert_eq!(found[0].state, RegionState::WellFormed);
        assert_eq!(found[0].content, Some(8..18));
        assert_eq!(found[0].start_tag.to_string(), "1:1-1:9");
        assert_eq!(found[0].element.to_string(), "1:1-1:28");
        assert_eq!(found[1].kind, RegionKind::Style);
        assert_eq!(found[1].language, RegionLanguage::Supported(Grammar::Css));
    }

    #[test]
    fn an_empty_element_is_an_empty_region() {
        let found = regions("<style></style>\n<script src=\"a.js\"></script>\n");
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|r| r.state == RegionState::Empty && r.content.is_none())
        );
    }

    #[test]
    fn a_lang_attribute_without_a_value_is_unsupported() {
        let found = regions("<script lang>x</script>\n");
        assert_eq!(
            found[0].language,
            RegionLanguage::Unsupported(String::new())
        );
    }
}
