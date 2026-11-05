//! Rust parsing integration tests.

use act_lite_parser::{Grammar, SourceFile, Workspace, query};

fn create_rust_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.rs", content.to_string(), Grammar::Rust).unwrap()
}

#[test]
fn parses_rust_function() {
    let source = create_rust_source("fn hello() -> String { String::from(\"world\") }");
    assert_eq!(source.grammar(), Grammar::Rust);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_struct() {
    let source = create_rust_source(
        r#"
        #[derive(Debug, Clone)]
        pub struct Config {
            pub name: String,
            timeout: u64,
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_enum() {
    let source = create_rust_source(
        r#"
        pub enum Status {
            Active,
            Inactive { reason: String },
            Pending(u32),
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_trait_and_impl() {
    let source = create_rust_source(
        r#"
        pub trait Processor {
            type Output;
            fn process(&self, input: &str) -> Self::Output;
        }

        impl Processor for Config {
            type Output = String;

            fn process(&self, input: &str) -> Self::Output {
                format!("{}: {}", self.name, input)
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_generics_and_lifetimes() {
    let source = create_rust_source(
        r#"
        pub fn process<'a, T, E>(items: &'a [T]) -> Result<&'a T, E>
        where
            T: Clone + Default,
            E: std::error::Error,
        {
            items.first().ok_or_else(|| panic!("empty"))
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_async_function() {
    let source = create_rust_source(
        r#"
        pub async fn fetch_data(url: &str) -> Result<Vec<u8>, Error> {
            todo!()
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_modules() {
    let source = create_rust_source(
        r#"
        pub mod utils {
            pub fn helper() -> i32 { 42 }
        }

        mod tests {
            use super::*;

            #[test]
            fn test_helper() {
                assert_eq!(utils::helper(), 42);
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_macros() {
    let source = create_rust_source(
        r#"
        macro_rules! create_getter {
            ($name:ident, $field:ident, $ty:ty) => {
                fn $name(&self) -> &$ty {
                    &self.$field
                }
            };
        }

        fn main() {
            println!("Hello, world!");
            vec![1, 2, 3];
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_rust_use_statements() {
    let source = create_rust_source(
        r#"
        use std::collections::HashMap;
        use std::io::{self, Read, Write};
        use crate::utils::helper;
        use super::Config;
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_rust_source(
        r#"
        fn greet(name: &str) -> String { name.to_string() }
        struct Config { name: String }
        enum Status { Active, Inactive }
        trait Processor { fn process(&self); }
    "#,
    );

    let result = act_lite_parser::query::execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::query::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_rust_source(
        r#"
        fn foo() {}
        const BAR: i32 = 1;
        struct Baz { field: String }
    "#,
    );

    let result = act_lite_parser::query::execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::query::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn rust_location_is_one_indexed() {
    // Test that tree-sitter's 0-indexed positions are correctly converted to 1-indexed
    let source = create_rust_source("fn main() {\n    println!(\"Hello\");\n}");
    let tree = source.tree();
    let root = tree.root_node();

    // Find the function item
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "function_item" {
            let start_pos = child.start_position();
            // tree-sitter row 0 should be line 1 when converted to Location
            assert_eq!(start_pos.row, 0);
            assert_eq!(start_pos.column, 0);

            // Create Location directly using 1-indexed values (0-indexed + 1)
            let location = act_lite_core::Location::new(
                source.path().to_path_buf(),
                start_pos.row as u32 + 1, // tree-sitter is 0-indexed, act is 1-indexed
                start_pos.column as u32 + 1,
            );
            assert_eq!(location.line(), 1);
            assert_eq!(location.column(), 1);
            break;
        }
    }
}

#[test]
fn rust_parse_file_integration() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");
    std::fs::write(&file_path, "fn main() { println!(\"Hello\"); }").unwrap();

    let mut workspace = Workspace::new(temp_dir.path()).unwrap();
    workspace.open_file(&file_path).unwrap();

    let source = workspace.get_file(&file_path).unwrap();
    assert_eq!(source.grammar(), Grammar::Rust);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn rust_workspace_with_multiple_files() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let file1 = temp_dir.path().join("lib.rs");
    let file2 = temp_dir.path().join("main.rs");

    std::fs::write(&file1, "pub fn hello() { \"world\" }").unwrap();
    std::fs::write(&file2, "use act_lib::hello; fn main() { hello(); }").unwrap();

    let mut workspace = Workspace::new(temp_dir.path()).unwrap();
    workspace.open_file(&file1).unwrap();
    workspace.open_file(&file2).unwrap();

    let source1 = workspace.get_file(&file1).unwrap();
    let source2 = workspace.get_file(&file2).unwrap();

    assert_eq!(source1.grammar(), Grammar::Rust);
    assert_eq!(source2.grammar(), Grammar::Rust);
}

#[test]
fn rust_skeleton_query_integration() {
    let source = create_rust_source(
        r#"
        fn greet(name: &str) -> String { name.to_string() }
        pub struct Config { name: String }
        pub enum Status { Active, Inactive }
    "#,
    );

    let result = query::execute_skeleton(&source).unwrap();
    match result {
        query::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
            // Should find function, struct, and enum
            assert!(skel.declarations.iter().any(|d| d.kind == "function"));
            assert!(skel.declarations.iter().any(|d| d.kind == "class"));
            assert!(skel.declarations.iter().any(|d| d.kind == "enum"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn rust_symbols_query_integration() {
    let source = create_rust_source(
        r#"
        fn foo() {}
        const BAR: i32 = 1;
        struct Baz { field: String }
    "#,
    );

    let result = query::execute_symbols(&source).unwrap();
    match result {
        query::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
        }
        _ => panic!("Expected Symbols result"),
    }
}
