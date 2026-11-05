//! JavaScript, JSX, TypeScript and TSX: `import`, `export … from`,
//! `import(…)`, `require(…)` and `import x = require(…)`.

use tree_sitter::Node;

use super::{Relationship, RelationshipKind, nodes, text, unquote};

pub(super) fn extract(root: Node, source: &str) -> Vec<Relationship> {
    let mut found = Vec::new();
    for node in nodes(root) {
        match node.kind() {
            "import_statement" => {
                if let Some(target) = node.child_by_field_name("source") {
                    let kind = if node.child(1).is_some_and(|c| c.kind() == "type") {
                        RelationshipKind::TypeImport
                    } else {
                        RelationshipKind::Import
                    };
                    found.push(Relationship::new(kind, unquote(text(target, source)), node));
                } else if let Some(target) = require_clause_source(node) {
                    found.push(Relationship::new(
                        RelationshipKind::Require,
                        unquote(text(target, source)),
                        node,
                    ));
                }
            }
            "export_statement" => {
                if let Some(target) = node.child_by_field_name("source") {
                    found.push(Relationship::new(
                        RelationshipKind::Reexport,
                        unquote(text(target, source)),
                        node,
                    ));
                }
            }
            "call_expression" => found.extend(call(node, source)),
            _ => {}
        }
    }
    found
}

fn require_clause_source(statement: Node) -> Option<Node> {
    let mut cursor = statement.walk();
    let clause = statement
        .named_children(&mut cursor)
        .find(|c| c.kind() == "import_require_clause")?;
    clause.child_by_field_name("source")
}

/// `import(…)` and `require(…)`: the first argument, as written when it is
/// not a string or a template string without a substitution.
fn call(node: Node, source: &str) -> Option<Relationship> {
    let function = node.child_by_field_name("function")?;
    let kind = match function.kind() {
        "import" => RelationshipKind::DynamicImport,
        "identifier" if text(function, source) == "require" => RelationshipKind::Require,
        _ => return None,
    };
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let first = arguments
        .named_children(&mut cursor)
        .find(|c| c.kind() != "comment")?;
    let literal = match first.kind() {
        "string" => true,
        "template_string" => {
            let mut cursor = first.walk();
            !first
                .named_children(&mut cursor)
                .any(|c| c.kind() == "template_substitution")
        }
        _ => false,
    };
    let written = text(first, source);
    let specifier = if literal { unquote(written) } else { written };
    Some(Relationship {
        literal,
        ..Relationship::new(kind, specifier, node)
    })
}
