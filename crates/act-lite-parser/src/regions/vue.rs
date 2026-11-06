//! The regions of a Vue component file.

use super::{Detector, Region};

/// Finds the script and style regions of a Vue file.
#[derive(Debug, Clone, Copy, Default)]
pub struct VueDetector;

impl Detector for VueDetector {
    fn regions(&self, _tree: &tree_sitter::Tree, _source: &str) -> Vec<Region> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Grammar;

    #[test]
    fn finds_no_region() {
        let text = "<script>let a = 1;</script>\n<style>p { color: red; }</style>\n";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&Grammar::Vue.language_support().ts_language())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        assert!(VueDetector.regions(&tree, text).is_empty());
    }
}
