// Quick test to discover CSS tree-sitter node types
use tree_sitter::Parser;

fn main() {
    let css_code = r#"
.button {
  color: red;
  margin: 10px;
}

.button:hover {
  color: blue;
}

#header {
  background: white;
}

:root {
  --main-color: blue;
}

@media (max-width: 600px) {
  .button {
    color: green;
  }
}
"#;

    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_css::LANGUAGE.into();
    parser
        .set_language(&language)
        .expect("Error loading CSS grammar");

    let tree = parser.parse(css_code, None).expect("Error parsing CSS");
    print_tree(tree.root_node(), css_code, 0);
}

fn print_tree(node: tree_sitter::Node, source: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let node_type = node.kind();
    let text = if node.child_count() == 0 {
        source[node.start_byte()..node.end_byte()]
            .trim()
            .to_string()
    } else {
        String::new()
    };

    if !text.is_empty() && text.len() < 50 {
        println!("{}{}: \"{}\"", indent, node_type, text);
    } else if !text.is_empty() {
        println!("{}{}:  (content)", indent, node_type);
    } else {
        println!("{}{}", indent, node_type);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(child, source, depth + 1);
    }
}
