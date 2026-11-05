//! Discover Scala node types from tree-sitter-scala grammar

use tree_sitter::{Node, Parser};

fn main() {
    let mut parser = Parser::new();
    let language = tree_sitter_scala::LANGUAGE.into();
    parser.set_language(&language).unwrap();

    let args: Vec<String> = std::env::args().collect();
    let file_path = args
        .get(1)
        .unwrap_or(&"/tmp/sample.scala".to_string())
        .clone();

    let source_code = std::fs::read_to_string(&file_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", file_path));

    let tree = parser.parse(&source_code, None).unwrap();
    print_tree(tree.root_node(), &source_code, 0, 250);
}

fn print_tree(node: Node, source: &str, depth: usize, max_lines: usize) {
    static mut LINE_COUNT: usize = 0;

    unsafe {
        if LINE_COUNT >= max_lines {
            return;
        }
        LINE_COUNT += 1;
    }

    let indent = "  ".repeat(depth);
    let kind = node.kind();
    let text = if node.child_count() == 0 {
        node.utf8_text(source.as_bytes()).unwrap_or("")
    } else {
        ""
    };

    if !text.is_empty() && text.len() < 50 {
        println!("{}{} [{}]", indent, kind, text.replace('\n', "\\n"));
    } else {
        println!("{}{}", indent, kind);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(child, source, depth + 1, max_lines);
    }
}
