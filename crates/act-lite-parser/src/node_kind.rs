//! Language-agnostic node kind classifications.

use serde::{Deserialize, Serialize};

/// Semantic classification of syntax tree nodes.
///
/// This enum provides a language-independent way to identify node types.
/// Each language maps its tree-sitter node types to these categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    // Declarations
    Function,
    Class,
    Object,
    Struct,
    Interface,
    Trait,
    TypeAlias,
    Enum,
    Variable,
    Constant,
    Parameter,
    Property,
    Method,
    Constructor,
    Event,
    Module,
    TypeParameter,
    Implementation,
    Decorator,
    Element,

    // Statements
    Import,
    Export,
    Block,

    // Expressions
    Call,
    Identifier,
    Expression,

    // Other
    Comment,
    Type,
    Unknown,
}

impl NodeKind {
    /// Returns true if this is a declaration node.
    pub fn is_declaration(&self) -> bool {
        matches!(
            self,
            NodeKind::Function
                | NodeKind::Class
                | NodeKind::Object
                | NodeKind::Struct
                | NodeKind::Interface
                | NodeKind::Trait
                | NodeKind::TypeAlias
                | NodeKind::Enum
                | NodeKind::Variable
                | NodeKind::Constant
                | NodeKind::Parameter
                | NodeKind::Property
                | NodeKind::Method
                | NodeKind::Constructor
                | NodeKind::Event
                | NodeKind::Module
                | NodeKind::TypeParameter
                | NodeKind::Implementation
        )
    }

    /// Returns true if this is a statement node.
    pub fn is_statement(&self) -> bool {
        matches!(self, NodeKind::Import | NodeKind::Export | NodeKind::Block)
    }

    /// Returns true if this is an expression node.
    pub fn is_expression(&self) -> bool {
        matches!(
            self,
            NodeKind::Call | NodeKind::Identifier | NodeKind::Expression
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_declaration() {
        assert!(NodeKind::Function.is_declaration());
        assert!(NodeKind::Class.is_declaration());
        assert!(!NodeKind::Import.is_declaration());
        assert!(!NodeKind::Comment.is_declaration());
    }

    #[test]
    fn is_statement() {
        assert!(NodeKind::Import.is_statement());
        assert!(NodeKind::Export.is_statement());
        assert!(!NodeKind::Function.is_statement());
        assert!(!NodeKind::Comment.is_statement());
    }

    #[test]
    fn is_expression() {
        assert!(NodeKind::Call.is_expression());
        assert!(NodeKind::Identifier.is_expression());
        assert!(NodeKind::Expression.is_expression());
        assert!(!NodeKind::Function.is_expression());
        assert!(!NodeKind::Comment.is_expression());
    }

    #[test]
    fn trait_is_a_declaration() {
        assert!(NodeKind::Trait.is_declaration());
    }

    #[test]
    fn object_is_a_declaration() {
        assert!(NodeKind::Object.is_declaration());
    }

    #[test]
    fn implementation_is_a_declaration() {
        assert!(NodeKind::Implementation.is_declaration());
    }

    #[test]
    fn implementation_is_not_a_statement_or_expression() {
        assert!(!NodeKind::Implementation.is_statement());
        assert!(!NodeKind::Implementation.is_expression());
    }
}
