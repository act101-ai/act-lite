//! Integration tests for workspace and query execution.

use act_lite_parser::{Grammar, QueryType, SourceFile, Workspace, query};
use tempfile::TempDir;

#[test]
fn workspace_with_multiple_files() {
    let dir = TempDir::new().unwrap();

    // Create test files
    std::fs::write(dir.path().join("file1.ts"), "function foo() {}").unwrap();
    std::fs::write(dir.path().join("file2.ts"), "class Bar {}").unwrap();

    let mut workspace = Workspace::new(dir.path()).unwrap();
    workspace.open_file("file1.ts").unwrap();
    workspace.open_file("file2.ts").unwrap();

    assert_eq!(workspace.file_count(), 2);
}

#[test]
fn skeleton_query_integration() {
    let source = SourceFile::from_content(
        "test.ts",
        r#"
import { foo } from './foo';

function main() {
  console.log('hello');
}

class Application {
  start() {}
}

interface Config {
  port: number;
}
"#
        .to_string(),
        Grammar::TypeScript,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(skel.declarations.len() >= 4);

            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(kinds.contains(&"import"));
            assert!(kinds.contains(&"function"));
            assert!(kinds.contains(&"class"));
            assert!(kinds.contains(&"interface"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn lsp_query_returns_not_implemented() {
    let source =
        SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
            .unwrap();

    for query_type in [QueryType::Diagnostics, QueryType::Type, QueryType::Callers] {
        let result = query::execute_query(query_type, &source).unwrap();
        match result {
            act_lite_parser::QueryResult::NotImplemented { .. } => {}
            _ => panic!("Expected NotImplemented for {:?}", query_type),
        }
    }
}

#[test]
fn typescript_skeleton_captures_generator_functions() {
    let source = SourceFile::from_content(
        "test.ts",
        r#"
function* generateIds(): Generator<number> {
    let id = 0;
    while (true) yield id++;
}

function regularFunc() {}
"#
        .to_string(),
        Grammar::TypeScript,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                kinds.contains(&"function"),
                "Expected generator to appear as function in skeleton, got: {:?}",
                kinds
            );
            assert!(
                names.contains(&"generateIds"),
                "Expected generator 'generateIds' in skeleton, got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

// TypeScript symbols query coverage tests

#[test]
fn typescript_symbols_captures_arrow_functions() {
    let source = SourceFile::from_content(
        "test.ts",
        r#"
const handler = (req: Request) => { return req; };
export const validate = (input: string): boolean => true;
const simple = () => {};
"#
        .to_string(),
        Grammar::TypeScript,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"handler"),
                "Arrow function 'handler' not found in symbols: {:?}",
                names
            );
            assert!(
                names.contains(&"validate"),
                "Arrow function 'validate' not found in symbols: {:?}",
                names
            );
            assert!(
                names.contains(&"simple"),
                "Arrow function 'simple' not found in symbols: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn typescript_symbols_captures_generator_functions() {
    let source = SourceFile::from_content(
        "test.ts",
        r#"
function* generateIds(): Generator<number> {
    let id = 0;
    while (true) yield id++;
}
"#
        .to_string(),
        Grammar::TypeScript,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"generateIds"),
                "Generator function 'generateIds' not found in symbols: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn typescript_symbols_captures_destructuring() {
    let source = SourceFile::from_content(
        "test.ts",
        r#"
const { name, age } = user;
const [first, ...rest] = items;
"#
        .to_string(),
        Grammar::TypeScript,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"name"),
                "Destructured 'name' not found in symbols: {:?}",
                names
            );
            assert!(
                names.contains(&"age"),
                "Destructured 'age' not found in symbols: {:?}",
                names
            );
            assert!(
                names.contains(&"first"),
                "Array destructured 'first' not found in symbols: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

// C# Integration Tests

#[test]
fn csharp_workspace_with_multiple_files() {
    let dir = TempDir::new().unwrap();

    // Create test files
    std::fs::write(
        dir.path().join("Program.cs"),
        "namespace App { class Program { static void Main() {} } }",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Service.cs"),
        "namespace App { public class Service { } }",
    )
    .unwrap();

    let mut workspace = Workspace::new(dir.path()).unwrap();
    workspace.open_file("Program.cs").unwrap();
    workspace.open_file("Service.cs").unwrap();

    assert_eq!(workspace.file_count(), 2);
}

#[test]
fn csharp_skeleton_query_integration() {
    let source = SourceFile::from_content(
        "test.cs",
        r#"
using System;
using System.Collections.Generic;

namespace Sample.Application
{
    public class Program
    {
        public static void Main(string[] args)
        {
            Console.WriteLine("Hello");
        }
    }

    public interface IService
    {
        void Process();
    }

    public enum Status
    {
        Active,
        Inactive
    }
}
"#
        .to_string(),
        Grammar::CSharp,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(skel.declarations.len() >= 4);

            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(kinds.contains(&"import")); // using directives
            assert!(kinds.contains(&"class"));
            assert!(kinds.contains(&"interface"));
            assert!(kinds.contains(&"enum"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn csharp_symbols_query_integration() {
    let source = SourceFile::from_content(
        "test.cs",
        r#"
namespace Sample
{
    public class MyClass
    {
        private int _value;
        public string Name { get; set; }

        public MyClass(int value)
        {
            _value = value;
        }

        public void Process()
        {
        }
    }
}
"#
        .to_string(),
        Grammar::CSharp,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(symbols.len() >= 3);

            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(names.contains(&"MyClass"));
            assert!(names.contains(&"Name"));
            assert!(names.contains(&"Process"));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn csharp_parse_file_integration() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("Sample.cs");

    std::fs::write(
        &file_path,
        r#"
namespace Test
{
    public record Person(string Name, int Age);

    public struct Point
    {
        public int X { get; init; }
        public int Y { get; init; }
    }

    public delegate void Handler(object sender, EventArgs e);
}
"#,
    )
    .unwrap();

    let mut parser = act_lite_parser::Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "compilation_unit");
    assert!(!tree.root_node().has_error());
}

#[test]
fn csharp_lsp_query_returns_not_implemented() {
    let source = SourceFile::from_content(
        "test.cs",
        "public class Foo {}".to_string(),
        Grammar::CSharp,
    )
    .unwrap();

    for query_type in [QueryType::Diagnostics, QueryType::Type, QueryType::Callers] {
        let result = query::execute_query(query_type, &source).unwrap();
        match result {
            act_lite_parser::QueryResult::NotImplemented { .. } => {}
            _ => panic!("Expected NotImplemented for {:?}", query_type),
        }
    }
}
