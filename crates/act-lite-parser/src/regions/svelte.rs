//! The regions of a Svelte component file.
//!
//! Each top-level `<script>` (the instance script and the module script) is
//! a script region, and each top-level `<style>` a style region. A
//! `<script>` or `<style>` inside markup (for example in `<svelte:head>`) is
//! passed to the page and is no region. Inside an ERROR node, the top-level
//! test uses the nearest ancestor that is not an ERROR node.

use super::{Detector, Region, RegionKind, RegionLanguage, RegionState};
use crate::Grammar;
use act_lite_core::CompactRange;
use tree_sitter::Node;

/// Finds the script and style regions of a Svelte file.
#[derive(Debug, Clone, Copy, Default)]
pub struct SvelteDetector;

impl Detector for SvelteDetector {
    fn regions(&self, tree: &tree_sitter::Tree, source: &str) -> Vec<Region> {
        let root = tree.root_node();
        let mut regions = Vec::new();
        top_level(root, root.is_error(), source, &mut regions);
        regions
    }
}

/// Collects the regions among the children of a top-level container: the
/// document, or an ERROR node whose nearest ancestor that is not an ERROR
/// node is the document. `in_error`: the container is an ERROR node.
fn top_level(container: Node, in_error: bool, source: &str, regions: &mut Vec<Region>) {
    let mut cursor = container.walk();
    for child in container.children(&mut cursor) {
        match child.kind() {
            "script_element" => {
                regions.extend(element_region(child, RegionKind::Script, in_error, source))
            }
            "style_element" => {
                regions.extend(element_region(child, RegionKind::Style, in_error, source))
            }
            // An unclosed `<script>` or `<style>`: the grammar keeps only its
            // start tag, inside an ERROR node.
            "start_tag" if in_error => {
                let kind = match tag_name(child, source) {
                    Some("script") => RegionKind::Script,
                    Some("style") => RegionKind::Style,
                    _ => continue,
                };
                let start_tag = compact(child);
                regions.push(Region {
                    kind,
                    language: language(kind, child, source),
                    content: None,
                    element: start_tag.clone(),
                    start_tag,
                    state: RegionState::Malformed,
                });
            }
            "ERROR" => top_level(child, true, source, regions),
            _ => {}
        }
    }
}

/// The region of a top-level script or style element.
fn element_region(element: Node, kind: RegionKind, in_error: bool, source: &str) -> Option<Region> {
    let start_tag = child(element, "start_tag")?;
    let content = child(element, "raw_text");
    let end_tag = child(element, "end_tag");
    let empty = content.is_none_or(|node| node.start_byte() == node.end_byte());
    let state = if in_error || element.has_error() {
        RegionState::Malformed
    } else if empty {
        RegionState::Empty
    } else if end_tag.is_none() {
        RegionState::Malformed
    } else {
        RegionState::WellFormed
    };
    Some(Region {
        kind,
        language: language(kind, start_tag, source),
        content: content
            .filter(|_| state == RegionState::WellFormed)
            .map(|node| node.byte_range()),
        element: compact(element),
        start_tag: compact(start_tag),
        state,
    })
}

/// The language a start tag's `lang` attribute selects.
fn language(kind: RegionKind, start_tag: Node, source: &str) -> RegionLanguage {
    let lang = attribute(start_tag, "lang", source);
    let grammar = match (kind, lang) {
        (RegionKind::Script, None | Some("js")) => Some(Grammar::JavaScript),
        (RegionKind::Script, Some("ts")) => Some(Grammar::TypeScript),
        (RegionKind::Style, None | Some("css")) => Some(Grammar::Css),
        (RegionKind::Style, Some("scss")) => Some(Grammar::Scss),
        _ => None,
    };
    match grammar {
        Some(grammar) => RegionLanguage::Supported(grammar),
        None => RegionLanguage::Unsupported(lang.unwrap_or_default().to_string()),
    }
}

/// The value as written of the first attribute `name` of a start tag:
/// the text inside the quotes, the unquoted value, or an expression as
/// written; `""` for an attribute without a value.
fn attribute<'s>(start_tag: Node, name: &str, source: &'s str) -> Option<&'s str> {
    let mut cursor = start_tag.walk();
    let attribute = start_tag.children(&mut cursor).find(|node| {
        node.kind() == "attribute"
            && child(*node, "attribute_name").map(|n| text(n, source)) == Some(name)
    })?;
    let value = child(attribute, "attribute_value")
        .or_else(|| {
            child(attribute, "quoted_attribute_value").and_then(|q| child(q, "attribute_value"))
        })
        .or_else(|| child(attribute, "expression"));
    Some(value.map_or("", |node| text(node, source)))
}

fn tag_name<'s>(start_tag: Node, source: &'s str) -> Option<&'s str> {
    child(start_tag, "tag_name").map(|node| text(node, source))
}

fn child<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn text<'s>(node: Node, source: &'s str) -> &'s str {
    source.get(node.byte_range()).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn regions(text: &str) -> Vec<Region> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&Grammar::Svelte.language_support().ts_language())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        SvelteDetector.regions(&tree, text)
    }

    #[test]
    fn finds_the_top_level_script_and_style() {
        let text = "<script>let a = 1;</script>\n<style>p { color: red; }</style>\n";
        let found = regions(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, RegionKind::Script);
        assert_eq!(found[0].content, Some(8..18));
        assert_eq!(found[1].kind, RegionKind::Style);
        assert!(found.iter().all(|r| r.state == RegionState::WellFormed));
    }

    #[test]
    fn a_script_in_markup_is_no_region() {
        assert!(regions("<div><script>let a;</script><style>p{}</style></div>\n").is_empty());
    }

    #[test]
    fn an_empty_lang_value_is_an_unsupported_language() {
        let found = regions("<script lang=\"\">let a;</script>\n");
        assert_eq!(
            found[0].language,
            RegionLanguage::Unsupported(String::new())
        );
    }
}
