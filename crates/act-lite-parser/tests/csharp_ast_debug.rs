/// Debug test to inspect C# AST structure for inline-variable
#[test]
fn inspect_csharp_variable_declaration() {
    let code = r#"class TestClass {
public int Calculate()
{
    var result = 42;
    return result;
}}"#;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .unwrap();

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n=== C# AST for inline-variable ===");
    print_node(root, code.as_bytes(), 0);

    // Find variable_declarator specifically
    println!("\n=== Variable Declarator Children ===");
    find_and_print_declarator(root, code.as_bytes());
}

fn find_and_print_declarator(node: tree_sitter::Node, source: &[u8]) {
    if node.kind() == "variable_declarator" {
        println!("\nFound variable_declarator:");
        println!("  Child count: {}", node.child_count());
        println!("  Named child count: {}", node.named_child_count());

        println!("\n  All children:");
        for i in 0..node.child_count() {
            let child = node.child(i).unwrap();
            let text = child.utf8_text(source).unwrap_or("");
            println!(
                "    [{}] {} = {:?} (named: {})",
                i,
                child.kind(),
                text,
                child.is_named()
            );
        }

        println!("\n  Named children only:");
        let mut cursor = node.walk();
        for (i, child) in node.named_children(&mut cursor).enumerate() {
            let text = child.utf8_text(source).unwrap_or("");
            println!("    [{}] {} = {:?}", i, child.kind(), text);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_and_print_declarator(child, source);
    }
}

fn print_node(node: tree_sitter::Node, source: &[u8], indent: usize) {
    let kind = node.kind();
    let text = if node.child_count() == 0 {
        node.utf8_text(source).unwrap_or("")
    } else {
        ""
    };

    println!(
        "{}{}{}",
        "  ".repeat(indent),
        kind,
        if !text.is_empty() {
            format!(": {:?}", text)
        } else {
            String::new()
        }
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_node(child, source, indent + 1);
    }
}
