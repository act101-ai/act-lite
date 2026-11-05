//! SQL language support.

use crate::{CommentDelimiters, Grammar, LanguageSupport, NodeKind};
use act_lite_core::registry::{LanguageCategory, LanguageId, LanguageMeta};

// Self-register SQL in the language registry
inventory::submit! {
    LanguageMeta {
        id: LanguageId("sql"),
        display_name: "SQL",
        extensions: &["sql"],
        category: LanguageCategory::Data,
    }
}

/// SQL language support implementation.
#[derive(Default)]
pub struct SqlSupport;

impl LanguageSupport for SqlSupport {
    fn grammar(&self) -> Grammar {
        Grammar::Sql
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_sequel::LANGUAGE.into()
    }

    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind {
        match node.kind() {
            // NOTE (SQL-36): NodeKind::Function is overloaded for three distinct SQL concepts:
            // 1. DML statements (select, insert, update, delete) — query execution units
            // 2. DDL callables (create_function, create_trigger) — stored routines
            // 3. Query composition units (cte) — named subquery blocks
            //
            // Future operations that distinguish "a callable stored function" from "a SELECT"
            // will need to check the underlying node kind string, not just NodeKind::Function.
            // This mapping is intentional for skeleton/symbol purposes and must not be changed
            // without updating all callers.

            // Query statements (treated as functions)
            "select" => NodeKind::Function,
            "insert" => NodeKind::Function,
            "update" => NodeKind::Function,
            "delete" => NodeKind::Function,

            // DDL statements creating objects (treated as classes)
            "create_table" => NodeKind::Class,
            "create_view" => NodeKind::Class,

            // Functions and procedures
            "create_function" => NodeKind::Function,
            "create_trigger" => NodeKind::Function,
            "function_body" => NodeKind::Block,

            // CTEs are like local functions
            "cte" => NodeKind::Function,

            // Indexes
            "create_index" => NodeKind::Variable,

            // Table structure
            "column_definition" => NodeKind::Property,
            "constraint" => NodeKind::Property,

            // Clauses (treated as blocks)
            "from" => NodeKind::Block,
            "where" => NodeKind::Block,
            "group_by" => NodeKind::Block,
            "order_by" => NodeKind::Block,
            "limit" => NodeKind::Block,

            // Joins
            "join" => NodeKind::Block,

            // NOTE (SQL-2): tree-sitter-sequel has no separate "having" wrapper node.
            // HAVING content sits in `from` as a sibling of `group_by`.
            // NOTE (SQL-5): No outer "with" wrapper node for CTEs. "cte" covers individual CTEs.
            // NOTE (SQL-6): No create_procedure node in tree-sitter-sequel grammar (folded into create_function).

            // ALTER TABLE — DDL mutation, equivalent to Class
            "alter_table" => NodeKind::Class,

            // SET operations (UNION / INTERSECT / EXCEPT all map to set_operation in tree-sitter-sequel)
            "set_operation" => NodeKind::Block,

            // DROP TABLE — schema removal
            "drop_table" => NodeKind::Class,

            // Columns added via ALTER TABLE
            "add_column" => NodeKind::Property,

            // Expressions
            "case" => NodeKind::Block,
            "subquery" => NodeKind::Block,

            // Function calls
            "invocation" => NodeKind::Call,
            "window_function" => NodeKind::Call,

            // Identifiers
            "identifier" => NodeKind::Identifier,
            "object_reference" => NodeKind::Identifier,

            // Comments
            "comment" => NodeKind::Comment,

            // Transaction blocks
            "transaction" => NodeKind::Block,

            _ => NodeKind::Unknown,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sql", "psql", "pgsql", "mysql"]
    }

    fn skeleton_query(&self) -> &'static str {
        include_str!("../../queries/sql/skeleton.scm")
    }

    fn symbols_query(&self) -> &'static str {
        include_str!("../../queries/sql/symbols.scm")
    }

    fn references_query(&self) -> &'static str {
        include_str!("../../queries/sql/references.scm")
    }

    fn comment_delimiters(&self) -> CommentDelimiters {
        CommentDelimiters {
            line: Some("--"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        }
    }

    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn function_body_kind(&self) -> &'static str {
        ""
    }

    fn decision_point_kinds(&self) -> &'static [&'static str] {
        // SQL has CASE WHEN (conditional expressions) and IF statements in PL/pgSQL/T-SQL,
        // but pure SQL files do not have cyclomatic complexity in the procedural sense.
        // tree-sitter-sequel does not expose a "case" decision point node for branching.
        // Revisit when stored procedure support is deepened.
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{find_node_by_kind, parse_with_support};

    crate::language_support_tests!(SqlSupport, Grammar::Sql, &["sql", "psql", "pgsql", "mysql"]);

    /// Extracts an identifier name starting at `byte_offset` in `source_str`.
    /// Reads ASCII alphanumeric characters and underscores.
    fn name_at(source_str: &str, byte_offset: usize) -> String {
        let bytes = source_str.as_bytes();
        let mut end = byte_offset;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        String::from_utf8_lossy(&bytes[byte_offset..end]).to_string()
    }

    #[test]
    fn symbols_query_finds_table_name() {
        use crate::SourceFile;
        use crate::query::QueryResult;
        use crate::query::execute_symbols;

        let source = SourceFile::from_content(
            "schema.sql",
            "CREATE TABLE users (id INT PRIMARY KEY, name TEXT NOT NULL)".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
                assert!(
                    names.contains(&"users"),
                    "should find 'users' table definition, found: {names:?}"
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_finds_cte_name() {
        use crate::SourceFile;
        use crate::query::QueryResult;
        use crate::query::execute_symbols;

        let source = SourceFile::from_content(
            "query.sql",
            "WITH active_users AS (SELECT * FROM users WHERE is_active = true) SELECT * FROM active_users".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
                assert!(
                    names.contains(&"active_users"),
                    "should find 'active_users' CTE name, found: {names:?}"
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_finds_view_name() {
        use crate::SourceFile;
        use crate::query::QueryResult;
        use crate::query::execute_symbols;

        let source = SourceFile::from_content(
            "schema.sql",
            "CREATE VIEW active_users AS SELECT * FROM users WHERE is_active = true".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
                assert!(
                    names.contains(&"active_users"),
                    "should find 'active_users' view, found: {names:?}"
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_finds_function_name() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_symbols};

        let source = SourceFile::from_content(
            "functions.sql",
            "CREATE OR REPLACE FUNCTION get_user_count() RETURNS INTEGER AS $$ BEGIN RETURN 1; END; $$ LANGUAGE plpgsql".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
                assert!(
                    names.contains(&"get_user_count"),
                    "should find 'get_user_count' function definition, found: {names:?}"
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn node_kind_select() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "SELECT * FROM users");
        let select_node = find_node_by_kind(tree.root_node(), "select").unwrap();
        assert_eq!(support.node_kind(&select_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_create_table() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "CREATE TABLE users (id INT)");
        let create_node = find_node_by_kind(tree.root_node(), "create_table").unwrap();
        assert_eq!(support.node_kind(&create_node), NodeKind::Class);
    }

    #[test]
    fn node_kind_invocation() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "SELECT COUNT(*) FROM users");
        let call_node = find_node_by_kind(tree.root_node(), "invocation")
            .expect("expected invocation node in COUNT(*) query");
        assert_eq!(support.node_kind(&call_node), NodeKind::Call);
    }

    #[test]
    fn node_kind_comment() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "-- comment\nSELECT 1");
        let comment_node = find_node_by_kind(tree.root_node(), "comment")
            .expect("expected comment node in -- comment query");
        assert_eq!(support.node_kind(&comment_node), NodeKind::Comment);
    }

    #[test]
    fn node_kind_cte() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "WITH cte AS (SELECT 1) SELECT * FROM cte");
        let cte_node =
            find_node_by_kind(tree.root_node(), "cte").expect("expected cte node in WITH query");
        assert_eq!(support.node_kind(&cte_node), NodeKind::Function);
    }

    #[test]
    fn node_kind_from_clause() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "SELECT * FROM users");
        let from_node = find_node_by_kind(tree.root_node(), "from")
            .expect("expected from node in SELECT * FROM users");
        assert_eq!(support.node_kind(&from_node), NodeKind::Block);
    }

    #[test]
    fn function_parent_kinds_is_empty_for_sql() {
        let support = SqlSupport;
        assert!(support.function_parent_kinds().is_empty());
    }

    #[test]
    fn function_body_kind_is_empty_for_sql() {
        let support = SqlSupport;
        assert_eq!(support.function_body_kind(), "");
    }

    #[test]
    fn decision_point_kinds_is_empty_for_sql() {
        let support = SqlSupport;
        assert!(support.decision_point_kinds().is_empty());
    }

    #[test]
    fn node_kind_alter_table() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "ALTER TABLE users ADD COLUMN bio TEXT");
        let node =
            find_node_by_kind(tree.root_node(), "alter_table").expect("expected alter_table node");
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_set_operation() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "SELECT id FROM users UNION SELECT id FROM admins");
        let node = find_node_by_kind(tree.root_node(), "set_operation")
            .expect("expected set_operation node for UNION");
        assert_eq!(support.node_kind(&node), NodeKind::Block);
    }

    #[test]
    fn node_kind_drop_table() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "DROP TABLE users");
        let node =
            find_node_by_kind(tree.root_node(), "drop_table").expect("expected drop_table node");
        assert_eq!(support.node_kind(&node), NodeKind::Class);
    }

    #[test]
    fn node_kind_add_column() {
        let support = SqlSupport;
        let tree = parse_with_support(&support, "ALTER TABLE users ADD COLUMN bio TEXT");
        let node = find_node_by_kind(tree.root_node(), "add_column")
            .expect("expected add_column node in ALTER TABLE ... ADD COLUMN");
        assert_eq!(support.node_kind(&node), NodeKind::Property);
    }

    #[test]
    fn node_kind_having_not_a_separate_node() {
        let support = SqlSupport;
        // tree-sitter-sequel has no separate "having" wrapper node.
        // HAVING content sits in `from` as a sibling of `group_by`. This test
        // documents that limitation.
        let tree = parse_with_support(
            &support,
            "SELECT user_id, COUNT(*) FROM orders GROUP BY user_id HAVING COUNT(*) > 5",
        );
        let having_node = find_node_by_kind(tree.root_node(), "having");
        assert!(
            having_node.is_none(),
            "tree-sitter-sequel has no 'having' wrapper node; HAVING sits in 'from'"
        );
        let group_by = find_node_by_kind(tree.root_node(), "group_by")
            .expect("expected group_by node in GROUP BY ... HAVING query");
        assert_eq!(support.node_kind(&group_by), NodeKind::Block);
    }

    #[test]
    fn node_kind_with_clause_not_a_separate_node() {
        let support = SqlSupport;
        // tree-sitter-sequel has no outer "with" wrapper node. CTEs are individual "cte" nodes.
        let tree = parse_with_support(
            &support,
            "WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a JOIN b ON true",
        );
        let with_node = find_node_by_kind(tree.root_node(), "with");
        assert!(
            with_node.is_none(),
            "tree-sitter-sequel has no outer 'with' wrapper node"
        );
    }

    #[test]
    fn skeleton_query_captures_alter_table() {
        use crate::SourceFile;
        use crate::query::execute_skeleton;

        let source = SourceFile::from_content(
            "test.sql",
            "ALTER TABLE users ADD COLUMN email TEXT NOT NULL".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            crate::query::QueryResult::Skeleton(skel) => {
                assert!(
                    !skel.declarations.is_empty(),
                    "skeleton should capture ALTER TABLE nodes"
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_captures_drop_table() {
        use crate::SourceFile;
        use crate::query::execute_skeleton;

        let source =
            SourceFile::from_content("test.sql", "DROP TABLE users".to_string(), Grammar::Sql)
                .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            crate::query::QueryResult::Skeleton(skel) => {
                assert!(
                    !skel.declarations.is_empty(),
                    "skeleton should capture DROP TABLE nodes"
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_captures_drop_view() {
        use crate::SourceFile;
        use crate::query::execute_skeleton;

        let source = SourceFile::from_content(
            "test.sql",
            "DROP VIEW active_users".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            crate::query::QueryResult::Skeleton(skel) => {
                assert!(
                    !skel.declarations.is_empty(),
                    "skeleton should capture DROP VIEW nodes"
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_captures_drop_index() {
        use crate::SourceFile;
        use crate::query::execute_skeleton;

        let source = SourceFile::from_content(
            "test.sql",
            "DROP INDEX idx_users_email".to_string(),
            Grammar::Sql,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            crate::query::QueryResult::Skeleton(skel) => {
                assert!(
                    !skel.declarations.is_empty(),
                    "skeleton should capture DROP INDEX nodes"
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn truncate_has_no_dedicated_skeleton_node() {
        use crate::SourceFile;
        use crate::query::execute_skeleton;

        // NOTE (SQL-11): tree-sitter-sequel has no "truncate" wrapper node.
        // TRUNCATE TABLE produces only keyword tokens; the skeleton query cannot capture it.
        // This test documents that limitation.
        let source =
            SourceFile::from_content("test.sql", "TRUNCATE TABLE users".to_string(), Grammar::Sql)
                .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            crate::query::QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations.is_empty(),
                    "TRUNCATE TABLE produces no skeleton node in tree-sitter-sequel"
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn references_query_finds_from_table_and_where_column() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "SELECT * FROM users WHERE user_id = 1";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"users".to_string()),
                    "should capture 'users' as FROM table reference, got: {names:?}"
                );
                assert!(
                    names.contains(&"user_id".to_string()),
                    "should capture 'user_id' as bare WHERE clause identifier, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_finds_update_target() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "UPDATE orders SET status = 'done' WHERE id = 1";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"orders".to_string()),
                    "should capture 'orders' as UPDATE target table, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_finds_delete_target() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "DELETE FROM sessions WHERE expires_at IS NOT NULL";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"sessions".to_string()),
                    "should capture 'sessions' as DELETE FROM table reference, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_finds_insert_target() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "INSERT INTO users (id) VALUES (1)";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"users".to_string()),
                    "should capture 'users' as INSERT target table, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    // SQL-16: SELECT list column identifiers are (field (identifier)) nodes in tree-sitter-sequel.
    // The (field (identifier) @reference) pattern in references.scm captures them correctly.
    #[test]
    fn references_query_finds_select_list_columns() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "SELECT id, name FROM users";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"id".to_string()),
                    "should capture 'id' from SELECT list, got: {names:?}"
                );
                assert!(
                    names.contains(&"name".to_string()),
                    "should capture 'name' from SELECT list, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    // SQL-18: Subquery table references in FROM clauses.
    // tree-sitter-sequel parses inner FROM clauses identically to top-level FROM clauses
    // — both produce (from (relation (object_reference (identifier)))) — so the existing
    // FROM pattern captures subquery table references automatically.
    #[test]
    fn references_query_finds_subquery_table_reference() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "SELECT * FROM (SELECT id FROM orders) sub";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"orders".to_string()),
                    "should capture 'orders' from subquery inner FROM, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    // SQL-19: CTE name referenced in the consuming SELECT's FROM clause.
    // tree-sitter-sequel represents the CTE reference in FROM as
    // (from (relation (object_reference (identifier)))) — identical to any other table
    // reference — so the existing FROM pattern captures it.
    #[test]
    fn references_query_finds_cte_reference() {
        use crate::SourceFile;
        use crate::query::{QueryResult, execute_references};

        let source_str = "WITH active_users AS (SELECT id FROM users WHERE active = true) SELECT * FROM active_users";
        let source =
            SourceFile::from_content("q.sql", source_str.to_string(), Grammar::Sql).unwrap();
        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                let names: Vec<String> = references
                    .iter()
                    .map(|r| {
                        r.byte_offset()
                            .expect("execute_references always provides byte_offset")
                    })
                    .map(|offset| name_at(source_str, offset))
                    .collect();
                assert!(
                    names.contains(&"active_users".to_string()),
                    "should capture 'active_users' CTE reference in FROM, got: {names:?}"
                );
            }
            _ => panic!("Expected References result"),
        }
    }
}
