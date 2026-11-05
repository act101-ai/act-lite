//! Discover JavaScript AST node types.

use tree_sitter::Parser;

fn main() {
    let code = r#"
// Top-level function
function greet(name) {
  return `Hello, ${name}!`;
}

// Arrow function
const add = (a, b) => a + b;

// Class with methods
class Calculator {
  constructor(initial) {
    this.value = initial;
  }

  add(n) {
    this.value += n;
    return this;
  }

  static create() {
    return new Calculator(0);
  }
}

// CommonJS export
module.exports = { greet, add, Calculator };

// ESM export
export { greet, add };
export default Calculator;

// Import patterns
const fs = require('fs');
import path from 'path';
import { join } from 'path';
"#;

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .unwrap();

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    fn print_tree(node: tree_sitter::Node, depth: usize, max_depth: usize) {
        let indent = "  ".repeat(depth);
        println!("{}{:?}", indent, node.kind());
        if depth < max_depth {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                print_tree(child, depth + 1, max_depth);
            }
        }
    }

    // Print tree to depth 3 to see top-level structure
    print_tree(root, 0, 3);
}
