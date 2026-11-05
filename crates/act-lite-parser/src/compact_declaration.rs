//! Compact declaration wrapper for token-efficient output.
//!
//! [`CompactDeclaration`] wraps [`Declaration`] with identifier-only names,
//! export deduplication, and compact string ranges.

use crate::Declaration;
use act_lite_core::CompactRange;
use serde::Serialize;

/// A compact representation of a [`Declaration`] optimized for token-efficient output.
///
/// Unlike [`Declaration`], this type:
/// - Extracts just the identifier name (e.g., `"UserService"` instead of `"class UserService { ... }"`)
/// - Deduplicates export wrappers (kind becomes inner kind, `exported` flag is set)
/// - Uses [`CompactRange`] for string-serialized ranges
///
/// # Examples
///
/// ```
/// use act_lite_parser::{CompactDeclaration, Declaration};
/// use act_lite_core::{Location, Range};
/// use std::path::PathBuf;
///
/// let range = Range::new(
///     Location::new(PathBuf::from("test.ts"), 1, 1),
///     Location::new(PathBuf::from("test.ts"), 5, 2),
/// );
/// let decl = Declaration {
///     kind: "class".to_string(),
///     name: "class UserService { ... }".to_string(),
///     range,
///     name_range: None,
///     visibility: None,
/// };
/// let compact = CompactDeclaration::from_declaration(&decl, false);
/// assert_eq!(compact.name, "UserService");
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct CompactDeclaration {
    /// The identifier name extracted from the declaration.
    pub name: String,
    /// The declaration kind (e.g., "class", "function", "const").
    pub kind: String,
    /// The source range in compact `"line:col-line:col"` format.
    pub range: CompactRange,
    /// Whether the declaration is exported. Omitted from JSON when `false`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub exported: bool,
}

impl CompactDeclaration {
    /// Creates a `CompactDeclaration` from a [`Declaration`].
    ///
    /// If `is_export_wrapper` is true or the declaration's kind is `"export"`,
    /// the inner kind is extracted from the name and the `exported` flag is set.
    pub fn from_declaration(decl: &Declaration, is_export_wrapper: bool) -> Self {
        let (kind, exported) = if is_export_wrapper || decl.kind == "export" {
            let inner_kind = extract_inner_kind(&decl.name).unwrap_or("declaration");
            (inner_kind.to_string(), true)
        } else {
            (decl.kind.clone(), false)
        };

        let name = extract_identifier(&decl.name);
        let range = CompactRange::from(&decl.range);

        Self {
            name,
            kind,
            range,
            exported,
        }
    }
}

/// Extracts the first non-keyword identifier token from a declaration name string.
///
/// Given `"class UserService { ... }"`, returns `"UserService"`.
/// Given `"export default class Foo"`, returns `"Foo"`.
fn extract_identifier(name: &str) -> String {
    let keywords = [
        "export",
        "default",
        "class",
        "function",
        "const",
        "let",
        "var",
        "interface",
        "type",
        "enum",
        "struct",
        "impl",
        "trait",
        "pub",
        "async",
        "abstract",
    ];
    for token in name.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if !clean.is_empty() && !keywords.contains(&clean) {
            return clean.to_string();
        }
    }
    // Fallback: return the last token cleaned of punctuation
    name.split_whitespace()
        .last()
        .unwrap_or(name)
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
        .to_string()
}

/// Extracts the inner declaration kind from a name string containing keywords.
///
/// Given `"export class UserService"`, returns `Some("class")`.
/// Returns `None` if no recognized kind keyword is found.
fn extract_inner_kind(name: &str) -> Option<&str> {
    let kinds = [
        "class",
        "function",
        "interface",
        "type",
        "enum",
        "const",
        "let",
        "var",
    ];
    name.split_whitespace().find(|token| kinds.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Declaration;
    use act_lite_core::{Location, Range};
    use std::path::PathBuf;

    #[test]
    fn compact_declaration_uses_identifier_name() {
        let range = Range::new(
            Location::new(PathBuf::from("test.ts"), 1, 1),
            Location::new(PathBuf::from("test.ts"), 5, 2),
        );
        let decl = Declaration {
            kind: "class".to_string(),
            name: "class UserService { ... }".to_string(),
            range,
            name_range: None,
            visibility: None,
        };
        let compact = CompactDeclaration::from_declaration(&decl, false);
        assert_eq!(compact.name, "UserService");
        assert_eq!(compact.kind, "class");
    }

    #[test]
    fn compact_declaration_deduplicates_export() {
        let range = Range::new(
            Location::new(PathBuf::from("test.ts"), 1, 1),
            Location::new(PathBuf::from("test.ts"), 1, 30),
        );
        let decl = Declaration {
            kind: "export".to_string(),
            name: "export class UserService".to_string(),
            range,
            name_range: None,
            visibility: None,
        };
        let compact = CompactDeclaration::from_declaration(&decl, true);
        assert_eq!(compact.kind, "class");
        assert!(compact.exported);
        assert_eq!(compact.name, "UserService");
    }

    #[test]
    fn compact_declaration_range_is_string() {
        let range = Range::new(
            Location::new(PathBuf::from("test.ts"), 8, 1),
            Location::new(PathBuf::from("test.ts"), 28, 2),
        );
        let decl = Declaration {
            kind: "function".to_string(),
            name: "processOrder".to_string(),
            range,
            name_range: None,
            visibility: None,
        };
        let compact = CompactDeclaration::from_declaration(&decl, false);
        let json = serde_json::to_value(&compact).unwrap();
        assert_eq!(json["range"], "8:1-28:2");
    }

    #[test]
    fn compact_declaration_not_exported_omits_field() {
        let range = Range::new(
            Location::new(PathBuf::from("test.ts"), 1, 1),
            Location::new(PathBuf::from("test.ts"), 3, 2),
        );
        let decl = Declaration {
            kind: "function".to_string(),
            name: "helper".to_string(),
            range,
            name_range: None,
            visibility: None,
        };
        let compact = CompactDeclaration::from_declaration(&decl, false);
        let json = serde_json::to_value(&compact).unwrap();
        assert!(json.get("exported").is_none());
    }

    #[test]
    fn extract_identifier_from_complex_name() {
        assert_eq!(
            extract_identifier("async function processOrder"),
            "processOrder"
        );
        assert_eq!(extract_identifier("export default class Foo"), "Foo");
        assert_eq!(extract_identifier("const MY_CONST = 42"), "MY_CONST");
        assert_eq!(extract_identifier("pub struct MyStruct"), "MyStruct");
    }

    #[test]
    fn extract_inner_kind_finds_declaration_keyword() {
        assert_eq!(
            extract_inner_kind("export class UserService"),
            Some("class")
        );
        assert_eq!(extract_inner_kind("export function foo"), Some("function"));
        assert_eq!(extract_inner_kind("export const BAR"), Some("const"));
        assert_eq!(
            extract_inner_kind("export interface IFoo"),
            Some("interface")
        );
        assert_eq!(extract_inner_kind("something unknown"), None);
    }
}
