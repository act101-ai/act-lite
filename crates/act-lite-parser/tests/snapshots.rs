//! Snapshot tests for query result stability.

use act_lite_parser::{Grammar, SourceFile, query};
use insta::assert_json_snapshot;

#[test]
fn snapshot_skeleton_typescript() {
    let source = std::fs::read_to_string("tests/fixtures/typescript/sample.ts").unwrap();

    let source_file = SourceFile::from_content("sample.ts", source, Grammar::TypeScript).unwrap();

    let result = query::execute_skeleton(&source_file).unwrap();

    assert_json_snapshot!(result);
}

#[test]
fn snapshot_symbols_typescript() {
    let source = std::fs::read_to_string("tests/fixtures/typescript/sample.ts").unwrap();

    let source_file = SourceFile::from_content("sample.ts", source, Grammar::TypeScript).unwrap();

    let result = query::execute_symbols(&source_file).unwrap();

    assert_json_snapshot!(result);
}

#[test]
fn snapshot_skeleton_csharp() {
    let source = std::fs::read_to_string("tests/fixtures/csharp/sample.cs").unwrap();

    let source_file = SourceFile::from_content("sample.cs", source, Grammar::CSharp).unwrap();

    let result = query::execute_skeleton(&source_file).unwrap();

    assert_json_snapshot!(result);
}

#[test]
fn snapshot_symbols_csharp() {
    let source = std::fs::read_to_string("tests/fixtures/csharp/sample.cs").unwrap();

    let source_file = SourceFile::from_content("sample.cs", source, Grammar::CSharp).unwrap();

    let result = query::execute_symbols(&source_file).unwrap();

    assert_json_snapshot!(result);
}

#[test]
fn snapshot_skeleton_sql() {
    let source = std::fs::read_to_string("tests/fixtures/sql/sample.sql").unwrap();

    let source_file = SourceFile::from_content("sample.sql", source, Grammar::Sql).unwrap();

    let result = query::execute_skeleton(&source_file).unwrap();

    assert_json_snapshot!(result);
}

#[test]
fn snapshot_symbols_sql() {
    let source = std::fs::read_to_string("tests/fixtures/sql/sample.sql").unwrap();

    let source_file = SourceFile::from_content("sample.sql", source, Grammar::Sql).unwrap();

    let result = query::execute_symbols(&source_file).unwrap();

    assert_json_snapshot!(result);
}
