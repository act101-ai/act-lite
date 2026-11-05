// crates/act-lite-parser/src/symbol_id.rs
//! Stable symbol identifiers for cross-operation symbol references.
//!
//! A `SymbolId` uniquely identifies a symbol within a codebase using its file path,
//! qualified name (dot-separated scope chain), and kind (function, method, class, etc.).
//!
//! Format: `<file>::<qualified_name>#<kind>`
//!
//! # Examples
//!
//! ```
//! use act_lite_parser::SymbolId;
//!
//! let id = SymbolId::new("src/auth.ts", &["authenticate"], "function");
//! assert_eq!(id.to_string(), "src/auth.ts::authenticate#function");
//!
//! let id = SymbolId::new("src/service.ts", &["UserService", "getUser"], "method");
//! assert_eq!(id.to_string(), "src/service.ts::UserService.getUser#method");
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// A stable identifier for a symbol within a codebase.
///
/// Encodes file path, qualified name (dot-separated scope chain), and symbol kind
/// into a single string representation: `<file>::<qualified_name>#<kind>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId {
    file: String,
    qualified_name: String,
    kind: String,
}

impl SymbolId {
    /// Creates a new `SymbolId` from file path, name parts (scope chain), and kind.
    ///
    /// Name parts are joined with `.` to form the qualified name.
    pub fn new(file: &str, name_parts: &[&str], kind: &str) -> Self {
        Self {
            file: file.to_string(),
            qualified_name: name_parts.join("."),
            kind: kind.to_string(),
        }
    }

    /// Parses a `SymbolId` from its string representation.
    ///
    /// Returns `None` if the string does not match the format `<file>::<qualified_name>#<kind>`
    /// or if any component is empty.
    pub fn parse(s: &str) -> Option<Self> {
        let (file, rest) = s.split_once("::")?;
        let (qualified_name, kind) = rest.rsplit_once('#')?;
        if file.is_empty() || qualified_name.is_empty() || kind.is_empty() {
            return None;
        }
        Some(Self {
            file: file.to_string(),
            qualified_name: qualified_name.to_string(),
            kind: kind.to_string(),
        })
    }

    /// Returns the file path component.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the dot-separated qualified name.
    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }

    /// Returns the symbol kind (e.g., "function", "method", "class").
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the simple (unqualified) name, i.e., the last segment of the qualified name.
    pub fn simple_name(&self) -> &str {
        self.qualified_name
            .rsplit_once('.')
            .map(|(_, name)| name)
            .unwrap_or(&self.qualified_name)
    }
}

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}#{}", self.file, self.qualified_name, self.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_id_top_level_function() {
        let id = SymbolId::new("src/auth.ts", &["authenticate"], "function");
        assert_eq!(id.to_string(), "src/auth.ts::authenticate#function");
    }

    #[test]
    fn symbol_id_nested_method() {
        let id = SymbolId::new("src/service.ts", &["UserService", "getUser"], "method");
        assert_eq!(id.to_string(), "src/service.ts::UserService.getUser#method");
    }

    #[test]
    fn symbol_id_parse_roundtrip() {
        let original = "src/service.ts::UserService.getUser#method";
        let parsed = SymbolId::parse(original).unwrap();
        assert_eq!(parsed.file(), "src/service.ts");
        assert_eq!(parsed.qualified_name(), "UserService.getUser");
        assert_eq!(parsed.kind(), "method");
        assert_eq!(parsed.to_string(), original);
    }

    #[test]
    fn symbol_id_parse_invalid() {
        assert!(SymbolId::parse("no_separator").is_none());
        assert!(SymbolId::parse("file::name_without_kind").is_none());
    }

    #[test]
    fn symbol_id_extracts_file_path() {
        let id = SymbolId::parse("src/auth.ts::authenticate#function").unwrap();
        assert_eq!(id.file(), "src/auth.ts");
    }
}
