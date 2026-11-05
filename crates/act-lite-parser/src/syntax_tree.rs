//! Syntax tree wrapper with traversal methods.

use tree_sitter::{Node, Tree, TreeCursor};

/// Owned wrapper around a tree-sitter syntax tree.
///
/// Provides convenience methods for traversal and querying.
pub struct SyntaxTree {
    tree: Tree,
    source: String,
}

impl SyntaxTree {
    /// Creates a new syntax tree from a tree-sitter tree and source code.
    pub fn new(tree: Tree, source: String) -> Self {
        Self { tree, source }
    }

    /// Returns the root node of the tree.
    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    /// Returns the source code this tree was parsed from.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Finds the smallest node at the given line and column.
    ///
    /// Lines and columns are 1-indexed to match editor conventions.
    /// Returns `None` if the position is out of bounds.
    pub fn node_at_position(&self, line: u32, column: u32) -> Option<Node<'_>> {
        if line == 0 || column == 0 {
            return None;
        }

        let byte_offset = self.position_to_byte(line, column)?;
        Some(self.node_at_byte(byte_offset))
    }

    /// Finds the smallest node at the given byte offset.
    pub fn node_at_byte(&self, byte_offset: usize) -> Node<'_> {
        let root = self.root_node();
        let mut cursor = root.walk();
        let mut node = root;

        loop {
            let mut found_child = false;
            for child in node.children(&mut cursor) {
                if child.start_byte() <= byte_offset && byte_offset < child.end_byte() {
                    node = child;
                    found_child = true;
                    break;
                }
            }
            if !found_child {
                break;
            }
        }

        node
    }

    /// Returns a cursor for traversing the tree.
    pub fn walk(&self) -> TreeCursor<'_> {
        self.tree.root_node().walk()
    }

    /// Converts a 1-indexed line and column to a byte offset.
    ///
    /// The column counts chars within the line; `len + 1` is the line's end.
    /// A column past that is `None`, never a position on a later line.
    fn position_to_byte(&self, line: u32, column: u32) -> Option<usize> {
        let line_start = match line.checked_sub(2) {
            None => 0,
            Some(newlines) => self.source.match_indices('\n').nth(newlines as usize)?.0 + 1,
        };
        let line_text = self.source[line_start..].split('\n').next().unwrap_or("");
        line_text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(line_text.len()))
            .nth(column.checked_sub(1)? as usize)
            .map(|i| line_start + i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_typescript(source: &str) -> SyntaxTree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        SyntaxTree::new(tree, source.to_string())
    }

    #[test]
    fn root_node() {
        let tree = parse_typescript("const x = 1;");
        let root = tree.root_node();
        assert_eq!(root.kind(), "program");
    }

    #[test]
    fn source() {
        let source = "const x = 1;";
        let tree = parse_typescript(source);
        assert_eq!(tree.source(), source);
    }

    #[test]
    fn node_at_byte() {
        let tree = parse_typescript("const x = 1;");
        let node = tree.node_at_byte(6); // 'x'
        assert_eq!(node.kind(), "identifier");
    }

    #[test]
    fn node_at_position() {
        let tree = parse_typescript("const x = 1;");
        let node = tree.node_at_position(1, 7).unwrap(); // 'x'
        assert_eq!(node.kind(), "identifier");
    }

    #[test]
    fn node_at_position_out_of_bounds() {
        let tree = parse_typescript("const x = 1;");
        assert!(tree.node_at_position(0, 1).is_none()); // 0-indexed not allowed
        assert!(tree.node_at_position(1, 0).is_none());
        assert!(tree.node_at_position(99, 1).is_none());
    }

    #[test]
    fn position_to_byte_emoji() {
        // Emoji are multi-byte UTF-8 characters
        let tree = parse_typescript("const 😀 = 1;");
        // '😀' is at byte 6, takes 4 bytes
        let byte_pos = tree.position_to_byte(1, 7); // First char after 'const '
        assert!(byte_pos.is_some());
        let node = tree.node_at_byte(byte_pos.unwrap());
        assert!(node.kind() == "identifier" || node.kind() == "ERROR");
    }

    #[test]
    fn position_to_byte_crlf() {
        // CRLF line endings (\r\n)
        let tree = parse_typescript("const x = 1;\r\nconst y = 2;");
        let byte_pos = tree.position_to_byte(2, 1); // Start of second line
        assert!(byte_pos.is_some());
        assert_eq!(byte_pos.unwrap(), 14); // "const x = 1;\r\n" is 14 bytes
    }

    #[test]
    fn position_to_byte_eof() {
        let tree = parse_typescript("const x = 1;\n");
        // Position at EOF with column 1 (start of line 2 which is empty)
        let byte_pos = tree.position_to_byte(2, 1);
        assert!(byte_pos.is_some());
        assert_eq!(byte_pos.unwrap(), 13); // Length of source
    }

    #[test]
    fn position_to_byte_eof_past_column() {
        let tree = parse_typescript("const x = 1;");
        // Position at EOF with column > 1 should be None
        let byte_pos = tree.position_to_byte(2, 5);
        assert!(byte_pos.is_none());
    }

    #[test]
    fn position_to_byte_empty_lines() {
        let tree = parse_typescript("const x = 1;\n\nconst y = 2;");
        // Empty line 2, position at start of line 3
        let byte_pos = tree.position_to_byte(3, 1);
        assert!(byte_pos.is_some());
        assert_eq!(byte_pos.unwrap(), 14); // "const x = 1;\n\n"
    }

    #[test]
    fn position_to_byte_column_past_line_end() {
        let tree = parse_typescript("x");
        // Column 99 on a 1-character line should be None
        let byte_pos = tree.position_to_byte(1, 99);
        assert!(byte_pos.is_none());
    }

    #[test]
    fn walk_returns_cursor() {
        let tree = parse_typescript("const x = 1;");
        let cursor = tree.walk();
        // TreeCursor starts at root
        assert_eq!(cursor.node().kind(), "program");
    }

    #[test]
    fn walk_can_traverse_children() {
        let tree = parse_typescript("const x = 1;");
        let mut cursor = tree.walk();
        // Go to first child
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.node().kind(), "lexical_declaration");
    }

    #[test]
    fn node_at_byte_root() {
        let tree = parse_typescript("const x = 1;");
        // Byte 0 should be inside the root
        let node = tree.node_at_byte(0);
        // At byte 0, we're at the start of "const", which is inside lexical_declaration
        assert!(node.kind() == "const" || node.kind() == "lexical_declaration");
    }

    #[test]
    fn node_at_byte_end_of_file() {
        let tree = parse_typescript("const x = 1;");
        // At the semicolon (byte 11)
        let node = tree.node_at_byte(11);
        assert!(
            node.kind() == ";" || node.kind() == "lexical_declaration" || node.kind() == "program"
        );
    }

    #[test]
    fn node_at_position_multiline() {
        let tree = parse_typescript("const x = 1;\nconst y = 2;");
        // Line 2, column 7 should be 'y'
        let node = tree.node_at_position(2, 7).unwrap();
        assert_eq!(node.kind(), "identifier");
    }

    #[test]
    fn source_returns_original() {
        let source = "const x = 1;\nconst y = 2;";
        let tree = parse_typescript(source);
        assert_eq!(tree.source(), source);
    }

    #[test]
    fn root_node_has_no_parent() {
        let tree = parse_typescript("const x = 1;");
        let root = tree.root_node();
        assert!(root.parent().is_none());
    }

    #[test]
    fn position_to_byte_line_start_after_multibyte_chars() {
        // "日本\n" is 7 bytes but 3 chars.
        let tree = parse_typescript("日本\nx");
        assert_eq!(tree.position_to_byte(2, 1), Some(7));
    }

    #[test]
    fn position_to_byte_line_start_inside_a_multibyte_char_does_not_panic() {
        // "é日\n" is 6 bytes but 3 chars; a char-index line start (3) lands inside '日'.
        let tree = parse_typescript("é日\nx");
        assert_eq!(tree.position_to_byte(2, 1), Some(6));
    }

    #[test]
    fn node_at_position_after_multibyte_line_finds_the_named_node() {
        let source = "const 日本 = 1;\nconst y = 2;";
        let tree = parse_typescript(source);
        let node = tree.node_at_position(2, 7).unwrap();
        assert_eq!(node.utf8_text(source.as_bytes()).unwrap(), "y");
    }

    #[test]
    fn position_to_byte_column_never_spills_into_the_next_line() {
        let tree = parse_typescript("ab\ncd");
        // Column 3 is the end of line 1 (its newline); column 4+ is past it.
        assert_eq!(tree.position_to_byte(1, 3), Some(2));
        assert_eq!(tree.position_to_byte(1, 4), None);
        assert_eq!(tree.position_to_byte(1, 5), None);
    }

    #[test]
    fn position_to_byte_multiline() {
        let tree = parse_typescript("const x = 1;\nconst y = 2;");
        // Line 1, column 1 should be byte 0
        let byte_pos = tree.position_to_byte(1, 1);
        assert_eq!(byte_pos, Some(0));

        // Line 2, column 1 should be byte 13 (after "const x = 1;\n")
        let byte_pos = tree.position_to_byte(2, 1);
        assert_eq!(byte_pos, Some(13));
    }
}
