//! Embedded regions of component files (Vue, Svelte, Astro).
//!
//! A region is a script, frontmatter or style element of a component file
//! whose content is code of another language. Each well-formed region of a
//! supported language is parsed on its own: the whole file text is parsed
//! with the region's grammar and one included range, the region's content.
//! The text is not copied, and every node of a region tree carries its
//! position (row, byte column, byte offset) in the component file.

pub mod astro;
pub mod svelte;
pub mod vue;

use crate::{Grammar, Parser, ParserError, error::Result};
use act_lite_core::CompactRange;
use std::ops::Range;

/// The reason given for a region whose parse exceeded the deadline.
pub const TIMEOUT_REASON: &str = "parse timeout";

/// What a region holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionKind {
    Script,
    Frontmatter,
    Style,
}

/// The language of a region's content.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RegionLanguage {
    /// JavaScript, JSX, TypeScript, TSX, CSS or SCSS.
    Supported(Grammar),
    /// A language act-lite does not parse: the attribute value as written.
    Unsupported(String),
}

/// Whether a region can be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionState {
    /// No content, or zero-width content, and no error in or above it.
    Empty,
    /// A start tag, content and an end tag, none missing, and no error
    /// node in or above the element.
    WellFormed,
    /// Any other region start: it has a kind and a start tag only.
    Malformed,
}

/// One region of a component file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub kind: RegionKind,
    pub language: RegionLanguage,
    /// The byte range of the content in the file; `None` when no content is
    /// used (an empty or a malformed region).
    pub content: Option<Range<usize>>,
    /// The whole element (frontmatter: from fence to fence).
    pub element: CompactRange,
    /// The start tag (frontmatter: the opening `---`).
    pub start_tag: CompactRange,
    pub state: RegionState,
}

impl Region {
    /// The grammar and content range of a region that is parsed: well-formed,
    /// of supported language, with a content range.
    pub(crate) fn parse_target(&self) -> Option<(Grammar, Range<usize>)> {
        match (&self.state, &self.language, &self.content) {
            (RegionState::WellFormed, RegionLanguage::Supported(grammar), Some(content)) => {
                Some((*grammar, content.clone()))
            }
            _ => None,
        }
    }
}

/// Finds the regions of a component file in its outer tree, in document
/// order.
pub trait Detector: Send + Sync {
    fn regions(&self, tree: &tree_sitter::Tree, source: &str) -> Vec<Region>;
}

/// The tree of one parsed region. Its nodes carry positions in the
/// component file.
#[derive(Debug, Clone)]
pub struct RegionTree {
    /// The index of the region in [`crate::SourceFile::regions`].
    pub region: usize,
    pub grammar: Grammar,
    pub tree: tree_sitter::Tree,
}

/// The file text with every byte outside the region's content, except
/// `\n`, replaced by a space. Lines, byte columns and byte offsets are
/// those of the component file.
pub fn view(source: &str, region: &Region) -> String {
    let content = region.content.clone().unwrap_or(0..0);
    let bytes: Vec<u8> = source
        .bytes()
        .enumerate()
        .map(|(i, b)| {
            if content.contains(&i) || b == b'\n' {
                b
            } else {
                b' '
            }
        })
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Whether a node of a component file's outer tree lies where nothing is
/// reported: below an ERROR node, or, in a Vue file, inside a custom block
/// or a `<template>` whose first `lang` is not `html`. The block itself (the
/// top-level element) and its start tag are not inside it.
pub(crate) fn outside_structure(grammar: Grammar, node: tree_sitter::Node, source: &str) -> bool {
    let mut top = None;
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if current.kind() == "ERROR" {
            return true;
        }
        if current.parent().is_some() {
            top = Some(current);
        }
        ancestor = current.parent();
    }
    match (grammar, top) {
        (Grammar::Vue, Some(block)) if within_start_tag(block, node) => false,
        (Grammar::Vue, Some(block)) => match block.kind() {
            "element" => true,
            "template_element" => {
                vue::template_lang(block, source).is_some_and(|lang| lang != "html")
            }
            _ => false,
        },
        _ => false,
    }
}

/// Whether `node` lies in the start tag of `element`.
fn within_start_tag(element: tree_sitter::Node, node: tree_sitter::Node) -> bool {
    let mut cursor = element.walk();
    element
        .children(&mut cursor)
        .filter(|child| matches!(child.kind(), "start_tag" | "self_closing_tag"))
        .any(|tag| tag.start_byte() <= node.start_byte() && node.end_byte() <= tag.end_byte())
}

/// The framework detector for a component grammar.
pub(crate) fn detector_for(grammar: Grammar) -> Option<Box<dyn Detector>> {
    match grammar {
        Grammar::Vue => Some(Box::new(vue::VueDetector)),
        Grammar::Svelte => Some(Box::new(svelte::SvelteDetector)),
        Grammar::Astro => Some(Box::new(astro::AstroDetector)),
        _ => None,
    }
}

/// Parses each region that is parsed, in document order. `stop` is called
/// before each parse; `true` ends the load with [`ParserError::Cancelled`].
/// A region whose parse exceeds the deadline gets no tree.
pub(crate) fn parse_regions(
    parser: &mut Parser,
    source: &str,
    regions: &[Region],
    stop: Option<&(dyn Fn() -> bool + Send + Sync)>,
) -> Result<Vec<RegionTree>> {
    let mut lines: Option<LineStarts> = None;
    let mut trees = Vec::new();
    for (index, region) in regions.iter().enumerate() {
        let Some((grammar, content)) = region.parse_target() else {
            continue;
        };
        if stop.is_some_and(|stop| stop()) {
            return Err(ParserError::Cancelled);
        }
        let range = lines
            .get_or_insert_with(|| LineStarts::new(source))
            .ts_range(content);
        match parser.parse_included(source, grammar, range) {
            Ok(tree) => trees.push(RegionTree {
                region: index,
                grammar,
                tree,
            }),
            Err(ParserError::ParseTimeout { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(trees)
}

/// The byte offset at which each line of a text starts.
pub(crate) struct LineStarts(Vec<usize>);

impl LineStarts {
    pub(crate) fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            source
                .bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self(starts)
    }

    /// The tree-sitter point (0-based row, byte column) of `byte`.
    pub(crate) fn point(&self, byte: usize) -> tree_sitter::Point {
        let row = self.0.partition_point(|&start| start <= byte) - 1;
        tree_sitter::Point::new(row, byte - self.0[row])
    }

    /// The tree-sitter range of the byte range `bytes`.
    pub(crate) fn ts_range(&self, bytes: Range<usize>) -> tree_sitter::Range {
        tree_sitter::Range {
            start_byte: bytes.start,
            end_byte: bytes.end,
            start_point: self.point(bytes.start),
            end_point: self.point(bytes.end),
        }
    }

    /// The byte offset of a 1-based line and byte column.
    pub(crate) fn byte(&self, line: u32, column: u32) -> usize {
        let start = self
            .0
            .get((line as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0);
        start + (column as usize).saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(content: Option<Range<usize>>) -> Region {
        Region {
            kind: RegionKind::Script,
            language: RegionLanguage::Supported(Grammar::TypeScript),
            content,
            element: CompactRange::new(1, 1, 1, 1),
            start_tag: CompactRange::new(1, 1, 1, 1),
            state: RegionState::WellFormed,
        }
    }

    #[test]
    fn view_blanks_every_byte_outside_the_content_but_newlines() {
        let text = "<p>ä</p>\n<script>let a;\n</script>\n";
        let start = text.find("let").unwrap();
        let end = text.find("</script>").unwrap();
        let viewed = view(text, &region(Some(start..end)));
        assert_eq!(viewed.len(), text.len());
        assert_eq!(viewed, "         \n        let a;\n         \n");
        assert_eq!(view(text, &region(None)).trim(), "");
    }

    #[test]
    fn the_timeout_reason_is_parse_timeout() {
        assert_eq!(TIMEOUT_REASON, "parse timeout");
    }

    #[test]
    fn line_starts_give_byte_points_and_offsets() {
        let text = "ab\nZähler x\n\ny";
        let lines = LineStarts::new(text);
        let x = text.find('x').unwrap();
        assert_eq!(lines.point(0), tree_sitter::Point::new(0, 0));
        assert_eq!(lines.point(x), tree_sitter::Point::new(1, 8));
        assert_eq!(lines.point(text.len()), tree_sitter::Point::new(3, 1));
        assert_eq!(lines.byte(2, 9), x);
        let range = lines.ts_range(3..x);
        assert_eq!(range.start_point, tree_sitter::Point::new(1, 0));
        assert_eq!(range.end_byte, x);
    }

    #[test]
    fn only_well_formed_regions_of_supported_language_are_parsed() {
        assert!(region(Some(0..1)).parse_target().is_some());
        assert!(region(None).parse_target().is_none());
        let mut unsupported = region(Some(0..1));
        unsupported.language = RegionLanguage::Unsupported("coffee".into());
        assert!(unsupported.parse_target().is_none());
        for state in [RegionState::Empty, RegionState::Malformed] {
            let mut other = region(Some(0..1));
            other.state = state;
            assert!(other.parse_target().is_none());
        }
    }

    #[test]
    fn each_component_grammar_has_a_detector_and_no_other_grammar_has_one() {
        for grammar in Grammar::all() {
            assert_eq!(
                detector_for(grammar).is_some(),
                grammar.is_component(),
                "{grammar:?}"
            );
        }
    }

    #[test]
    fn a_stop_before_the_first_region_parse_cancels() {
        let text = "<script>let a = 1;</script>";
        let start = text.find("let").unwrap();
        let end = text.find("</script>").unwrap();
        let stop = || true;
        let result = parse_regions(
            &mut Parser::new(),
            text,
            &[region(Some(start..end))],
            Some(&stop),
        );
        assert!(matches!(result, Err(ParserError::Cancelled)));
    }
}
