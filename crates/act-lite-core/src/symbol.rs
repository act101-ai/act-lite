//! Symbol representation for code entities.
//!
//! A [`Symbol`] represents any named entity in source code: functions, classes,
//! variables, types, etc. Symbols have a kind, name, location, and optional
//! metadata.

use crate::Range;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The kind of a symbol.
///
/// This is a language-agnostic categorization. Language-specific details
/// are handled by the parser crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    // Declarations
    /// A function or method.
    Function,
    /// A class definition.
    Class,
    /// An interface or protocol.
    Interface,
    /// A trait (Rust) or similar construct.
    Trait,
    /// An enum definition.
    Enum,
    /// A struct definition.
    Struct,
    /// A type alias.
    TypeAlias,
    /// A module or namespace.
    Module,

    // Members
    /// A method (function inside a class/struct).
    Method,
    /// A field or property.
    Field,
    /// A constructor.
    Constructor,
    /// A getter accessor.
    Getter,
    /// A setter accessor.
    Setter,
    /// An event declaration.
    Event,
    /// An attribute/decorator.
    Attribute,

    // Values
    /// A variable declaration.
    Variable,
    /// A constant declaration.
    Constant,
    /// A function/method parameter.
    Parameter,
    /// An enum variant/member.
    EnumMember,

    // References
    /// An import statement.
    Import,
    /// An export statement.
    Export,

    // Other
    /// A generic type parameter.
    TypeParameter,
    /// A label (for loops, etc.).
    Label,
    /// Unknown or unclassified symbol.
    Unknown,
}

impl SymbolKind {
    /// Returns true if this kind represents a type definition.
    pub fn is_type_definition(&self) -> bool {
        matches!(
            self,
            SymbolKind::Class
                | SymbolKind::Interface
                | SymbolKind::Trait
                | SymbolKind::Enum
                | SymbolKind::Struct
                | SymbolKind::TypeAlias
        )
    }

    /// Returns true if this kind represents a callable.
    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor
        )
    }

    /// Returns true if this kind represents a member of a type.
    pub fn is_member(&self) -> bool {
        matches!(
            self,
            SymbolKind::Method
                | SymbolKind::Field
                | SymbolKind::Constructor
                | SymbolKind::Getter
                | SymbolKind::Setter
                | SymbolKind::Event
        )
    }

    /// Returns true if this kind represents a value (variable, constant, parameter).
    pub fn is_value(&self) -> bool {
        matches!(
            self,
            SymbolKind::Variable
                | SymbolKind::Constant
                | SymbolKind::Parameter
                | SymbolKind::EnumMember
        )
    }
}

/// Visibility/access level of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Public (accessible from anywhere).
    Public,
    /// Private (accessible only within defining scope).
    Private,
    /// Protected (accessible from subclasses).
    Protected,
    /// Package/internal (accessible within module/package).
    Internal,
    /// Visibility not determined or not applicable.
    #[default]
    Unknown,
}

/// A named entity in source code.
///
/// Symbols are the primary way to identify and reference code elements.
/// They can be resolved to ranges, used as refactoring targets, and
/// queried for relationships.
///
/// # Examples
///
/// ```
/// use act_lite_core::{Symbol, SymbolKind, Range};
/// use std::path::PathBuf;
///
/// let range = Range::from_byte_offsets(PathBuf::from("src/auth.rs"), 100, 120);
/// let symbol = Symbol::new("authenticate".to_string(), SymbolKind::Function, range);
///
/// assert_eq!(symbol.name(), "authenticate");
/// assert_eq!(symbol.kind(), SymbolKind::Function);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    /// The symbol's name.
    name: String,
    /// The kind of symbol.
    kind: SymbolKind,
    /// The full range of the symbol definition (including body).
    range: Range,
    /// The range of just the symbol's name/identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    name_range: Option<Range>,
    /// The symbol's visibility.
    #[serde(default)]
    visibility: Visibility,
    /// Fully qualified name (e.g., "module::Class::method").
    #[serde(skip_serializing_if = "Option::is_none")]
    qualified_name: Option<String>,
    /// The containing symbol (e.g., method's class).
    #[serde(skip_serializing_if = "Option::is_none")]
    container: Option<String>,
    /// Optional documentation/comments associated with the symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    documentation: Option<String>,
    /// Optional type signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    type_signature: Option<String>,
}

impl Symbol {
    /// Creates a new symbol with minimal information.
    pub fn new(name: String, kind: SymbolKind, range: Range) -> Self {
        Self {
            name,
            kind,
            range,
            name_range: None,
            visibility: Visibility::Unknown,
            qualified_name: None,
            container: None,
            documentation: None,
            type_signature: None,
        }
    }

    /// Returns the symbol's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the symbol's kind.
    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    /// Returns the full range of the symbol definition.
    pub fn range(&self) -> &Range {
        &self.range
    }

    /// Returns the range of just the name/identifier.
    pub fn name_range(&self) -> Option<&Range> {
        self.name_range.as_ref()
    }

    /// Returns the symbol's visibility.
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// Returns the fully qualified name.
    pub fn qualified_name(&self) -> Option<&str> {
        self.qualified_name.as_deref()
    }

    /// Returns the containing symbol's name.
    pub fn container(&self) -> Option<&str> {
        self.container.as_deref()
    }

    /// Returns the documentation.
    pub fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }

    /// Returns the type signature.
    pub fn type_signature(&self) -> Option<&str> {
        self.type_signature.as_deref()
    }

    /// Returns the file containing this symbol.
    pub fn file(&self) -> &PathBuf {
        self.range.file()
    }

    /// Sets the name range.
    pub fn with_name_range(mut self, range: Range) -> Self {
        self.name_range = Some(range);
        self
    }

    /// Sets the visibility.
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Sets the qualified name.
    pub fn with_qualified_name(mut self, name: String) -> Self {
        self.qualified_name = Some(name);
        self
    }

    /// Sets the container.
    pub fn with_container(mut self, container: String) -> Self {
        self.container = Some(container);
        self
    }

    /// Sets the documentation.
    pub fn with_documentation(mut self, doc: String) -> Self {
        self.documentation = Some(doc);
        self
    }

    /// Sets the type signature.
    pub fn with_type_signature(mut self, sig: String) -> Self {
        self.type_signature = Some(sig);
        self
    }
}

/// A reference to a symbol (a usage, not a definition).
///
/// References track where symbols are used throughout the codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolReference {
    /// The range of the reference.
    range: Range,
    /// Whether this is a write (assignment) or read reference.
    is_write: bool,
    /// Whether this is the definition site (for completeness).
    is_definition: bool,
}

impl SymbolReference {
    /// Creates a new read reference.
    pub fn read(range: Range) -> Self {
        Self {
            range,
            is_write: false,
            is_definition: false,
        }
    }

    /// Creates a new write reference.
    pub fn write(range: Range) -> Self {
        Self {
            range,
            is_write: true,
            is_definition: false,
        }
    }

    /// Creates a definition reference.
    pub fn definition(range: Range) -> Self {
        Self {
            range,
            is_write: false,
            is_definition: true,
        }
    }

    /// Returns the range of the reference.
    pub fn range(&self) -> &Range {
        &self.range
    }

    /// Returns true if this is a write reference.
    pub fn is_write(&self) -> bool {
        self.is_write
    }

    /// Returns true if this is the definition.
    pub fn is_definition(&self) -> bool {
        self.is_definition
    }

    /// Returns the file containing this reference.
    pub fn file(&self) -> &PathBuf {
        self.range.file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_range(file: &str, start: usize, end: usize) -> Range {
        Range::from_byte_offsets(PathBuf::from(file), start, end)
    }

    #[test]
    fn symbol_kind_is_type_definition() {
        assert!(SymbolKind::Class.is_type_definition());
        assert!(SymbolKind::Interface.is_type_definition());
        assert!(SymbolKind::Struct.is_type_definition());
        assert!(!SymbolKind::Function.is_type_definition());
        assert!(!SymbolKind::Variable.is_type_definition());
    }

    #[test]
    fn symbol_kind_is_callable() {
        assert!(SymbolKind::Function.is_callable());
        assert!(SymbolKind::Method.is_callable());
        assert!(SymbolKind::Constructor.is_callable());
        assert!(!SymbolKind::Class.is_callable());
        assert!(!SymbolKind::Variable.is_callable());
    }

    #[test]
    fn symbol_kind_is_member() {
        assert!(SymbolKind::Method.is_member());
        assert!(SymbolKind::Field.is_member());
        assert!(SymbolKind::Getter.is_member());
        assert!(!SymbolKind::Function.is_member());
        assert!(!SymbolKind::Class.is_member());
    }

    #[test]
    fn symbol_kind_is_value() {
        assert!(SymbolKind::Variable.is_value());
        assert!(SymbolKind::Constant.is_value());
        assert!(SymbolKind::Parameter.is_value());
        assert!(!SymbolKind::Function.is_value());
        assert!(!SymbolKind::Class.is_value());
    }

    #[test]
    fn symbol_new() {
        let range = make_range("test.rs", 0, 100);
        let symbol = Symbol::new("foo".to_string(), SymbolKind::Function, range);
        assert_eq!(symbol.name(), "foo");
        assert_eq!(symbol.kind(), SymbolKind::Function);
        assert_eq!(symbol.visibility(), Visibility::Unknown);
    }

    #[test]
    fn symbol_with_builders() {
        let range = make_range("test.rs", 0, 100);
        let name_range = make_range("test.rs", 4, 7);
        let symbol = Symbol::new("foo".to_string(), SymbolKind::Function, range)
            .with_name_range(name_range)
            .with_visibility(Visibility::Public)
            .with_qualified_name("module::foo".to_string())
            .with_container("module".to_string())
            .with_documentation("A foo function".to_string())
            .with_type_signature("fn foo() -> i32".to_string());

        assert!(symbol.name_range().is_some());
        assert_eq!(symbol.visibility(), Visibility::Public);
        assert_eq!(symbol.qualified_name(), Some("module::foo"));
        assert_eq!(symbol.container(), Some("module"));
        assert_eq!(symbol.documentation(), Some("A foo function"));
        assert_eq!(symbol.type_signature(), Some("fn foo() -> i32"));
    }

    #[test]
    fn symbol_reference_read() {
        let range = make_range("test.rs", 50, 53);
        let reference = SymbolReference::read(range);
        assert!(!reference.is_write());
        assert!(!reference.is_definition());
    }

    #[test]
    fn symbol_reference_write() {
        let range = make_range("test.rs", 50, 53);
        let reference = SymbolReference::write(range);
        assert!(reference.is_write());
        assert!(!reference.is_definition());
    }

    #[test]
    fn symbol_reference_definition() {
        let range = make_range("test.rs", 50, 53);
        let reference = SymbolReference::definition(range);
        assert!(!reference.is_write());
        assert!(reference.is_definition());
    }

    #[test]
    fn symbol_serialization() {
        let range = make_range("test.rs", 0, 100);
        let symbol = Symbol::new("foo".to_string(), SymbolKind::Function, range)
            .with_visibility(Visibility::Public);

        let json = serde_json::to_string(&symbol).unwrap();
        let deserialized: Symbol = serde_json::from_str(&json).unwrap();

        assert_eq!(symbol, deserialized);
    }

    #[test]
    fn symbol_file() {
        let range = make_range("src/test.rs", 10, 20);
        let symbol = Symbol::new("foo".to_string(), SymbolKind::Function, range);
        assert_eq!(symbol.file(), &PathBuf::from("src/test.rs"));
    }

    #[test]
    fn symbol_reference_file() {
        let range = make_range("src/main.rs", 100, 103);
        let reference = SymbolReference::read(range);
        assert_eq!(reference.file(), &PathBuf::from("src/main.rs"));
    }

    #[test]
    fn symbol_reference_range() {
        let range = make_range("test.rs", 42, 45);
        let reference = SymbolReference::definition(range.clone());
        assert_eq!(reference.range(), &range);
    }

    #[test]
    fn visibility_all_variants() {
        // Test all Visibility variants exist and are distinct
        assert_ne!(Visibility::Public, Visibility::Private);
        assert_ne!(Visibility::Protected, Visibility::Internal);
        assert_ne!(Visibility::Internal, Visibility::Unknown);
    }

    #[test]
    fn symbol_kind_all_predicates() {
        // Test predicates for all kinds
        assert!(SymbolKind::Enum.is_type_definition());
        assert!(SymbolKind::TypeAlias.is_type_definition());
        assert!(SymbolKind::Trait.is_type_definition());

        assert!(SymbolKind::Constructor.is_callable());

        assert!(SymbolKind::Constructor.is_member());
        assert!(SymbolKind::Setter.is_member());
        assert!(SymbolKind::Event.is_member());

        assert!(SymbolKind::EnumMember.is_value());

        assert!(!SymbolKind::Module.is_type_definition());
        assert!(!SymbolKind::Module.is_callable());
        assert!(!SymbolKind::Module.is_member());
        assert!(!SymbolKind::Module.is_value());
    }

    #[test]
    fn symbol_kind_serialization() {
        let kinds = vec![
            SymbolKind::Function,
            SymbolKind::Class,
            SymbolKind::Interface,
            SymbolKind::Variable,
            SymbolKind::Method,
            SymbolKind::Field,
        ];

        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let deserialized: SymbolKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, deserialized);
        }
    }

    #[test]
    fn visibility_serialization() {
        let visibilities = vec![
            Visibility::Public,
            Visibility::Private,
            Visibility::Protected,
            Visibility::Internal,
            Visibility::Unknown,
        ];

        for vis in visibilities {
            let json = serde_json::to_string(&vis).unwrap();
            let deserialized: Visibility = serde_json::from_str(&json).unwrap();
            assert_eq!(vis, deserialized);
        }
    }
}
