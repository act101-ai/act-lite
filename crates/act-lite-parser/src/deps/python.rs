//! Python: `import a.b`, `from X import n`, and the names a module binds at
//! its top level.

use tree_sitter::Node;

use super::{ModuleNames, Relationship, RelationshipKind, nodes, text};

pub(super) fn extract(root: Node, source: &str) -> Vec<Relationship> {
    let mut found = Vec::new();
    for node in nodes(root) {
        match node.kind() {
            "import_statement" => {
                for name in field_children(node, "name") {
                    let dotted = name.child_by_field_name("name").unwrap_or(name);
                    found.push(Relationship::new(
                        RelationshipKind::Import,
                        text(dotted, source),
                        node,
                    ));
                }
            }
            "import_from_statement" => {
                let Some(module) = node.child_by_field_name("module_name") else {
                    continue;
                };
                let module = text(module, source);
                let mut imported: Vec<&str> = field_children(node, "name")
                    .into_iter()
                    .map(|n| text(n.child_by_field_name("name").unwrap_or(n), source))
                    .collect();
                if has_child(node, "wildcard_import") {
                    imported.push("*");
                }
                for name in imported {
                    let specifier = if module.ends_with('.') {
                        format!("{module}{name}")
                    } else {
                        format!("{module}.{name}")
                    };
                    found.push(Relationship {
                        imported: Some(name.to_string()),
                        ..Relationship::new(RelationshipKind::FromImport, &specifier, node)
                    });
                }
            }
            _ => {}
        }
    }
    found
}

/// The names bound by the module's top-level statements, also inside
/// top-level `if`, `try`, `with` and loop blocks and by a top-level `for`
/// target or `with … as` alias, not inside a function or class.
pub(super) fn module_names(root: Node, source: &str) -> ModuleNames {
    let mut names = ModuleNames::default();
    bind_block(root, source, &mut names);
    names
}

fn bind_block(block: Node, source: &str, names: &mut ModuleNames) {
    let mut cursor = block.walk();
    for statement in block.named_children(&mut cursor) {
        bind_statement(statement, source, names);
    }
}

fn bind_statement(statement: Node, source: &str, names: &mut ModuleNames) {
    let insert = |names: &mut ModuleNames, node: Node| {
        names.names.insert(text(node, source).to_string());
    };
    match statement.kind() {
        "function_definition" | "class_definition" => {
            if let Some(name) = statement.child_by_field_name("name") {
                insert(names, name);
            }
        }
        "decorated_definition" => {
            if let Some(definition) = statement.child_by_field_name("definition") {
                bind_statement(definition, source, names);
            }
        }
        "expression_statement" => {
            let mut cursor = statement.walk();
            for child in statement.named_children(&mut cursor) {
                let mut assignment = Some(child);
                while let Some(a) = assignment.filter(|a| a.kind() == "assignment") {
                    // An annotation without a value (`n: int`) binds nothing.
                    let right = a.child_by_field_name("right");
                    if let Some(left) = a.child_by_field_name("left")
                        && right.is_some()
                    {
                        bind_target(left, source, names);
                    }
                    assignment = right;
                }
            }
        }
        "import_statement" => {
            for name in field_children(statement, "name") {
                match name.child_by_field_name("alias") {
                    Some(alias) => insert(names, alias),
                    None => {
                        let dotted = text(name, source);
                        let top = dotted.split('.').next().unwrap_or(dotted).trim();
                        names.names.insert(top.to_string());
                    }
                }
            }
        }
        "import_from_statement" | "future_import_statement" => {
            for name in field_children(statement, "name") {
                insert(names, name.child_by_field_name("alias").unwrap_or(name));
            }
            if has_child(statement, "wildcard_import") {
                names.glob = true;
            }
        }
        "global_statement" => {
            let mut cursor = statement.walk();
            for name in statement.named_children(&mut cursor) {
                if name.kind() == "identifier" {
                    insert(names, name);
                }
            }
        }
        "if_statement" | "try_statement" | "with_statement" | "for_statement"
        | "while_statement" => {
            if let Some(left) = statement.child_by_field_name("left") {
                bind_target(left, source, names);
            }
            let mut cursor = statement.walk();
            for child in statement.named_children(&mut cursor) {
                if child.kind() == "with_clause" {
                    let mut cursor = child.walk();
                    for item in child.named_children(&mut cursor) {
                        let alias = item
                            .child_by_field_name("value")
                            .filter(|v| v.kind() == "as_pattern")
                            .and_then(|v| v.child_by_field_name("alias"));
                        if let Some(alias) = alias {
                            bind_target(alias, source, names);
                        }
                    }
                } else if child.kind() == "block" {
                    bind_block(child, source, names);
                } else if child.kind().ends_with("_clause") {
                    let mut cursor = child.walk();
                    for block in child.named_children(&mut cursor) {
                        if block.kind() == "block" {
                            bind_block(block, source, names);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// An assignment, `for` or `with … as` target: a name, or the names of a
/// tuple or list target.
fn bind_target(target: Node, source: &str, names: &mut ModuleNames) {
    match target.kind() {
        "identifier" => {
            names.names.insert(text(target, source).to_string());
        }
        "pattern_list"
        | "tuple_pattern"
        | "list_pattern"
        | "list_splat_pattern"
        | "as_pattern_target"
        | "tuple"
        | "list"
        | "list_splat"
        | "parenthesized_expression" => {
            let mut cursor = target.walk();
            for child in target.named_children(&mut cursor) {
                bind_target(child, source, names);
            }
        }
        _ => {}
    }
}

fn field_children<'t>(node: Node<'t>, field: &str) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

fn has_child(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).any(|c| c.kind() == kind)
}
