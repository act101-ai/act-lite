//! Go: each `import_spec`, interpreted or raw string.

use tree_sitter::Node;

use super::{Relationship, RelationshipKind, go_value, nodes, text, unquote};

pub(super) fn extract(root: Node, source: &str) -> Vec<Relationship> {
    nodes(root)
        .into_iter()
        .filter(|n| n.kind() == "import_spec")
        .filter_map(|spec| {
            let path = spec.child_by_field_name("path")?;
            let written = unquote(text(path, source));
            let value = if path.kind() == "raw_string_literal" {
                Ok(written.to_string())
            } else {
                go_value(written)
            };
            Some(Relationship {
                value,
                ..Relationship::new(RelationshipKind::Import, written, spec)
            })
        })
        .collect()
}
