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
                    found.push(Relationship {
                        value: Ok(module_name(dotted, source)),
                        ..Relationship::new(RelationshipKind::Import, text(dotted, source), node)
                    });
                }
            }
            "import_from_statement" => {
                let Some(module) = node.child_by_field_name("module_name") else {
                    continue;
                };
                let value = module_name(module, source);
                let module = text(module, source);
                let mut imported: Vec<String> = field_children(node, "name")
                    .into_iter()
                    .map(|n| module_name(n.child_by_field_name("name").unwrap_or(n), source))
                    .collect();
                if has_child(node, "wildcard_import") {
                    imported.push("*".to_string());
                }
                for name in imported {
                    let specifier = if module.ends_with('.') {
                        format!("{module}{name}")
                    } else {
                        format!("{module}.{name}")
                    };
                    let value = if value.ends_with('.') {
                        format!("{value}{name}")
                    } else {
                        format!("{value}.{name}")
                    };
                    found.push(Relationship {
                        value: Ok(value),
                        imported: Some(name),
                        ..Relationship::new(RelationshipKind::FromImport, &specifier, node)
                    });
                }
            }
            _ => {}
        }
    }
    found
}

/// The names still bound after the module runs: those its top-level
/// statements bind, also inside top-level `if`, `try`, `with`, `match` and
/// loop blocks, by a top-level `for` target, `with … as` alias or `case`
/// pattern capture, and by a `:=` the module runs (not inside a function,
/// lambda or class body or a generator expression). An `except … as` name
/// is deleted at the end of its handler, and a module-level `del` unbinds
/// what it names.
pub(super) fn module_names(root: Node, source: &str) -> ModuleNames {
    let mut names = ModuleNames::default();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if statement.kind() == "delete_statement" {
            // Only a `del` that always runs leaves its names unbound. Its
            // targets run from left to right.
            let mut cursor = statement.walk();
            for target in statement.named_children(&mut cursor) {
                unbind_target(target, source, &mut names);
            }
        } else {
            bind_walrus(statement, source, &mut names);
            bind_statement(statement, source, &mut names);
        }
    }
    names
}

fn bind_block(block: Node, source: &str, names: &mut ModuleNames) {
    let mut cursor = block.walk();
    for statement in block.named_children(&mut cursor) {
        bind_walrus(statement, source, names);
        bind_statement(statement, source, names);
    }
}

/// The `:=` targets of the parts of a statement that run at module scope:
/// not its blocks (each is a statement list of its own, read by
/// `bind_block`, or a function or class body), not a lambda's body, and not
/// a generator expression, whose body runs only when it is iterated.
fn bind_walrus(node: Node, source: &str, names: &mut ModuleNames) {
    if node.kind() == "generator_expression" {
        return;
    }
    if node.kind() == "named_expression"
        && let Some(name) = node.child_by_field_name("name")
    {
        names.names.insert(text(name, source).to_string());
    }
    let mut cursor = node.walk();
    for (i, child) in node.children(&mut cursor).enumerate() {
        let skip = match child.kind() {
            // A `match` body holds the `case` clauses, whose guards run.
            "block" => node.kind() != "match_statement",
            _ => node.kind() == "lambda" && node.field_name_for_child(i as u32) == Some("body"),
        };
        if child.is_named() && !skip {
            bind_walrus(child, source, names);
        }
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
                if child.kind() == "augmented_assignment"
                    && let Some(left) = child.child_by_field_name("left")
                {
                    bind_target(left, source, names);
                }
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
        "type_alias_statement" => {
            // `type X = …` and `type X[T] = …`.
            let mut name = statement.child_by_field_name("left");
            while let Some(n) = name.filter(|n| n.kind() != "identifier") {
                name = n.named_child(0);
            }
            if let Some(name) = name {
                insert(names, name);
            }
        }
        "import_statement" => {
            for name in field_children(statement, "name") {
                match name.child_by_field_name("alias") {
                    Some(alias) => insert(names, alias),
                    None => {
                        let dotted = module_name(name, source);
                        let top = dotted.split('.').next().unwrap_or(&dotted);
                        names.names.insert(top.to_string());
                    }
                }
            }
        }
        "import_from_statement" | "future_import_statement" => {
            for name in field_children(statement, "name") {
                match name.child_by_field_name("alias") {
                    Some(alias) => insert(names, alias),
                    None => {
                        names.names.insert(module_name(name, source));
                    }
                }
            }
            if has_child(statement, "wildcard_import") {
                names.glob = true;
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
                    // `except E as n` deletes `n` at the end of the handler,
                    // also when the handler binds it.
                    let alias = child
                        .child_by_field_name("value")
                        .filter(|v| v.kind() == "as_pattern")
                        .and_then(|v| v.child_by_field_name("alias"));
                    let mut clause = ModuleNames::default();
                    let mut cursor = child.walk();
                    for block in child.named_children(&mut cursor) {
                        if block.kind() == "block" {
                            bind_block(block, source, &mut clause);
                        }
                    }
                    if let Some(alias) = alias {
                        unbind_target(alias, source, &mut clause);
                    }
                    names.names.extend(clause.names);
                    names.glob |= clause.glob;
                }
            }
        }
        "match_statement" => {
            let Some(body) = statement.child_by_field_name("body") else {
                return;
            };
            let mut cursor = body.walk();
            for case in body.named_children(&mut cursor) {
                let mut cursor = case.walk();
                for child in case.named_children(&mut cursor) {
                    match child.kind() {
                        "case_pattern" => bind_pattern(child, source, names),
                        "block" => bind_block(child, source, names),
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}

/// The capture names of a `case` pattern: not `_`, a dotted value, a class
/// name, a keyword name or a mapping key.
fn bind_pattern(pattern: Node, source: &str, names: &mut ModuleNames) {
    match pattern.kind() {
        "identifier" => {
            names.names.insert(text(pattern, source).to_string());
        }
        "dotted_name" if pattern.named_child_count() == 1 => {
            names.names.insert(text(pattern, source).to_string());
        }
        "case_pattern" | "list_pattern" | "tuple_pattern" | "splat_pattern" | "as_pattern"
        | "union_pattern" | "class_pattern" | "keyword_pattern" | "dict_pattern" => {
            // The class name of `P(…)` and the keyword of `k=…` come first.
            let skip = usize::from(matches!(
                pattern.kind(),
                "class_pattern" | "keyword_pattern"
            ));
            let mut cursor = pattern.walk();
            for (i, child) in pattern.children(&mut cursor).enumerate() {
                if child.is_named()
                    && i >= skip
                    && pattern.field_name_for_child(i as u32) != Some("key")
                {
                    bind_pattern(child, source, names);
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

/// The names a `del` target or an `except … as` alias unbinds: a name, or
/// the names of a tuple or list, in order; an attribute or a subscript
/// unbinds no name, and a `:=` in it binds.
fn unbind_target(target: Node, source: &str, names: &mut ModuleNames) {
    match target.kind() {
        "identifier" => {
            names.names.remove(text(target, source));
        }
        "expression_list" | "tuple" | "list" | "parenthesized_expression" | "as_pattern_target" => {
            let mut cursor = target.walk();
            for child in target.named_children(&mut cursor) {
                unbind_target(child, source, names);
            }
        }
        _ => bind_walrus(target, source, names),
    }
}

/// A module name without the whitespace, comments and line continuations
/// between its tokens: a `dotted_name`'s identifiers joined by `.`, after
/// the dots of a relative import's prefix.
fn module_name(node: Node, source: &str) -> String {
    let mut out = String::new();
    let mut dotted = None;
    match node.kind() {
        "relative_import" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "import_prefix" => {
                        out.extend(text(child, source).chars().filter(|c| *c == '.'));
                    }
                    "dotted_name" => dotted = Some(child),
                    _ => {}
                }
            }
        }
        "dotted_name" => dotted = Some(node),
        _ => return text(node, source).to_string(),
    }
    if let Some(dotted) = dotted {
        let mut cursor = dotted.walk();
        let identifiers: Vec<&str> = dotted
            .named_children(&mut cursor)
            .filter(|c| c.kind() == "identifier")
            .map(|c| text(c, source))
            .collect();
        out.push_str(&identifiers.join("."));
    }
    out
}

fn field_children<'t>(node: Node<'t>, field: &str) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

fn has_child(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).any(|c| c.kind() == kind)
}
