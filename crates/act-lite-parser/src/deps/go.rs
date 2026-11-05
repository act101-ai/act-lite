//! Go: each `import_spec`, interpreted or raw string.

use tree_sitter::Node;

use super::{Relationship, RelationshipKind, nodes, text, unquote};

pub(super) fn extract(root: Node, source: &str) -> Vec<Relationship> {
    nodes(root)
        .into_iter()
        .filter(|n| n.kind() == "import_spec")
        .filter_map(|spec| {
            let path = spec.child_by_field_name("path")?;
            Some(Relationship::new(
                RelationshipKind::Import,
                unquote(text(path, source)),
                spec,
            ))
        })
        .collect()
}
