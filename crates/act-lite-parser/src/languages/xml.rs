//! XML language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register XML in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("xml"),
        display_name: "XML",
        extensions: &["xml", "xsd", "xsl", "xslt", "svg", "plist", "csproj", "fsproj", "vbproj", "props", "targets", "resx", "xaml"],
        category: LanguageCategory::Data,
    }
}

/// XML language support implementation.
#[derive(Default)]
pub struct XmlSupport;

impl LanguageSupport for XmlSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Xml
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_xml::LANGUAGE_XML.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            "document" => NodeKind::Module,
            "element" | "STag" | "ETag" | "EmptyElemTag" => NodeKind::Object,
            "Attribute" => NodeKind::Property,
            "Name" => NodeKind::Identifier,
            "AttValue" | "CharData" => NodeKind::Expression,
            "Comment" => NodeKind::Comment,
            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[
            "xml", "xsd", "xsl", "xslt", "svg", "plist", "csproj", "fsproj", "vbproj", "props",
            "targets", "resx", "xaml",
        ]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/xml/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/xml/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/xml/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: None,
            block_start: Some("<!--"),
            block_end: Some("-->"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar() {
        assert_eq!(XmlSupport.grammar(), Grammar::Xml);
    }

    #[test]
    fn ts_language() {
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&XmlSupport.ts_language()).is_ok());
    }

    #[test]
    fn skeleton_returns_declarations() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_skeleton};
        let source = SourceFile::from_content(
            "test.xml",
            "<project><name>t</name></project>\n".to_string(),
            Grammar::Xml,
        )
        .unwrap();
        match execute_skeleton(&source).unwrap() {
            QueryResult::Skeleton(s) => assert!(
                s.declarations.iter().any(|d| d.name == "project"),
                "expected 'project', got: {:?}",
                s.declarations
            ),
            _ => panic!("wrong result type"),
        }
    }
}
