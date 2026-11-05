// Test to discover tree-sitter YAML node types
#[cfg(test)]
mod tests {
    use tree_sitter::{Node, Parser};
    use tree_sitter_yaml;

    fn print_tree(node: &Node, indent: usize) {
        let indent_str = " ".repeat(indent);
        let start = node.start_position();
        let end = node.end_position();
        println!(
            "{}{} [{}:{}-{}:{}]",
            indent_str,
            node.kind(),
            start.row,
            start.column,
            end.row,
            end.column
        );
        for i in 0..node.child_count() {
            print_tree(&node.child(i).unwrap(), indent + 2);
        }
    }

    #[test]
    fn discover_yaml_node_types() {
        let code = r#"
# Sample YAML document
name: "Example"
version: 1.0
features:
  - feature1
  - feature2
database:
  host: localhost
  port: 5432
  credentials:
    user: admin
    password: secret
"#;
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_yaml::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        println!("YAML Tree Structure:");
        print_tree(&tree.root_node(), 0);
    }
}
