//! SQL parsing integration tests.

use act_lite_parser::{Grammar, Parser, QueryType, SourceFile, Workspace, query};
use tempfile::TempDir;

fn create_sql_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.sql", content.to_string(), Grammar::Sql).unwrap()
}

#[test]
fn parses_basic_select() {
    let source = create_sql_source("SELECT id, name FROM users WHERE active = true");
    assert_eq!(source.grammar(), Grammar::Sql);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_select_with_join() {
    let source = create_sql_source(
        r#"
SELECT
    u.id,
    u.name,
    o.order_id,
    o.total
FROM users u
INNER JOIN orders o ON u.id = o.user_id
LEFT JOIN addresses a ON u.id = a.user_id
WHERE u.created_at > '2024-01-01'
ORDER BY o.total DESC
LIMIT 100
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cte() {
    let source = create_sql_source(
        r#"
WITH active_users AS (
    SELECT id, name
    FROM users
    WHERE active = true
),
recent_orders AS (
    SELECT user_id, SUM(total) as total_spent
    FROM orders
    WHERE created_at > NOW() - INTERVAL '30 days'
    GROUP BY user_id
)
SELECT au.name, ro.total_spent
FROM active_users au
JOIN recent_orders ro ON au.id = ro.user_id
ORDER BY ro.total_spent DESC
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_subquery_in_where() {
    let source = create_sql_source(
        r#"
SELECT *
FROM products
WHERE category_id IN (
    SELECT id FROM categories WHERE parent_id IS NULL
)
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_subquery_in_from() {
    let source = create_sql_source(
        r#"
SELECT avg_order.user_id, avg_order.avg_total
FROM (
    SELECT user_id, AVG(total) as avg_total
    FROM orders
    GROUP BY user_id
) AS avg_order
WHERE avg_order.avg_total > 100
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_window_functions() {
    let source = create_sql_source(
        r#"
SELECT
    id,
    name,
    department,
    salary,
    ROW_NUMBER() OVER (PARTITION BY department ORDER BY salary DESC) as rank,
    AVG(salary) OVER (PARTITION BY department) as dept_avg
FROM employees
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_case_expression() {
    let source = create_sql_source(
        r#"
SELECT
    id,
    name,
    CASE
        WHEN status = 'active' THEN 'Active User'
        WHEN status = 'pending' THEN 'Pending Approval'
        ELSE 'Inactive'
    END as status_label
FROM users
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_insert() {
    let source = create_sql_source(
        r#"INSERT INTO users (name, email, created_at)
VALUES ('John Doe', 'john@example.com', NOW())"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_insert_with_select() {
    let source = create_sql_source(
        r#"INSERT INTO user_archives (id, name, email)
SELECT id, name, email FROM users WHERE deleted_at IS NOT NULL"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_update() {
    let source = create_sql_source(
        r#"UPDATE orders
SET status = 'shipped', shipped_at = NOW()
WHERE id = 123"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_delete() {
    let source = create_sql_source(r#"DELETE FROM sessions WHERE expires_at < NOW()"#);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_create_table() {
    let source = create_sql_source(
        r#"
CREATE TABLE tasks (
    id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50) DEFAULT 'pending',
    assignee_id INTEGER REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP,
    CONSTRAINT valid_status CHECK (status IN ('pending', 'in_progress', 'completed'))
)
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_create_index() {
    let source = create_sql_source(r#"CREATE INDEX idx_tasks_status ON tasks(status)"#);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_create_view() {
    let source = create_sql_source(
        r#"
CREATE VIEW active_tasks AS
SELECT t.*, u.name as assignee_name
FROM tasks t
LEFT JOIN users u ON t.assignee_id = u.id
WHERE t.status != 'completed'
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_alter_table() {
    let source = create_sql_source(r#"ALTER TABLE tasks ADD COLUMN priority INTEGER DEFAULT 0"#);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_stored_procedure() {
    let source = create_sql_source(
        r#"CREATE FUNCTION get_user_orders() RETURNS INTEGER LANGUAGE SQL AS 'SELECT 1'"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_trigger() {
    let source = create_sql_source(
        r#"
CREATE TRIGGER update_timestamp
    BEFORE UPDATE ON tasks
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at()
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_transaction_block() {
    let source = create_sql_source(
        r#"
BEGIN;
    UPDATE accounts SET balance = balance - 100 WHERE id = 1;
    UPDATE accounts SET balance = balance + 100 WHERE id = 2;
    COMMIT;
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_single_line_comment() {
    let source = create_sql_source(
        r#"
-- This is a comment
SELECT * FROM users
-- Another comment
WHERE id = 1
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_multi_line_comment() {
    let source = create_sql_source(
        r#"
/* Multi-line
   comment */
SELECT * FROM users
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_sql_source(
        r#"CREATE TABLE users (id INT PRIMARY KEY); CREATE INDEX idx_users_id ON users(id); CREATE VIEW active_users AS SELECT * FROM users WHERE active = true"#,
    );

    let result = query::execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
            assert!(skel.declarations.len() >= 3);
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_definitions() {
    let source = create_sql_source(
        r#"CREATE TABLE users (id INT PRIMARY KEY); CREATE INDEX idx_users_id ON users(id); CREATE VIEW active_users AS SELECT * FROM users"#,
    );

    let result = query::execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
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
fn parses_union_query() {
    let source = create_sql_source(
        r#"
SELECT name FROM employees
WHERE department = 'Sales'
UNION ALL
SELECT name FROM contractors
WHERE department = 'Sales'
ORDER BY name
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_group_by_and_having() {
    let source = create_sql_source(
        r#"
SELECT department, COUNT(*) as employee_count, AVG(salary) as avg_salary
FROM employees
WHERE hire_date > '2020-01-01'
GROUP BY department
HAVING COUNT(*) > 5
ORDER BY avg_salary DESC
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cross_join() {
    let source = create_sql_source(
        r#"
SELECT *
FROM orders o
CROSS JOIN products p
WHERE o.product_id = p.id
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_multiple_ctes() {
    let source = create_sql_source(
        r#"
WITH
  cte1 AS (SELECT a FROM t1),
  cte2 AS (SELECT b FROM t2),
  cte3 AS (SELECT c FROM t3)
SELECT * FROM cte1
JOIN cte2 ON cte1.a = cte2.b
JOIN cte3 ON cte2.b = cte3.c
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn sql_parse_file_integration() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("sample.sql");

    std::fs::write(
        &file_path,
        r#"
SELECT u.id, u.name, o.total
FROM users u
INNER JOIN orders o ON u.id = o.user_id
WHERE u.created_at > '2024-01-01'
ORDER BY o.total DESC
"#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "program");
    assert!(!tree.root_node().has_error());
}

#[test]
fn sql_lsp_query_returns_not_implemented() {
    let source = create_sql_source("SELECT * FROM users");

    for query_type in [QueryType::Diagnostics, QueryType::Type, QueryType::Callers] {
        let result = query::execute_query(query_type, &source).unwrap();
        match result {
            act_lite_parser::QueryResult::NotImplemented { .. } => {}
            _ => panic!("Expected NotImplemented for {:?}", query_type),
        }
    }
}

#[test]
fn sql_location_is_one_indexed() {
    let source = create_sql_source(
        r#"
CREATE TABLE users (id INT PRIMARY KEY)
CREATE INDEX idx_users_id ON users(id)
"#,
    );

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            for decl in skel.declarations {
                assert!(
                    decl.range.start().line() >= 1,
                    "Line should be 1-indexed, got {} for {}",
                    decl.range.start().line(),
                    decl.name
                );
                assert!(
                    decl.range.start().column() >= 1,
                    "Column should be 1-indexed, got {} for {}",
                    decl.range.start().column(),
                    decl.name
                );
            }
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn sql_workspace_with_multiple_files() {
    let dir = TempDir::new().unwrap();

    std::fs::write(
        dir.path().join("001_users.sql"),
        "CREATE TABLE users (id INT PRIMARY KEY)",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("002_orders.sql"),
        "CREATE TABLE orders (id INT PRIMARY KEY)",
    )
    .unwrap();

    let mut workspace = Workspace::new(dir.path()).unwrap();
    workspace.open_file("001_users.sql").unwrap();
    workspace.open_file("002_orders.sql").unwrap();

    assert_eq!(workspace.file_count(), 2);
}

#[test]
fn parses_insert_basic() {
    let source = create_sql_source(r#"INSERT INTO users (id, name) VALUES (1, 'John')"#);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_upsert_not_supported_in_grammar() {
    let source = create_sql_source(
        r#"INSERT INTO users (id, name) VALUES (1, 'John') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name"#,
    );
    // tree-sitter-sequel does not support PostgreSQL's ON CONFLICT DO UPDATE syntax.
    // This test documents the limitation: the grammar produces an ERROR node for upserts.
    assert!(
        source.tree().root_node().has_error(),
        "ON CONFLICT DO UPDATE should produce a parse error in tree-sitter-sequel"
    );
}

#[test]
fn parses_returning_clause() {
    let source =
        create_sql_source(r#"INSERT INTO users (name) VALUES ('John') RETURNING id, created_at"#);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_recursive_cte() {
    let source = create_sql_source(
        r#"
WITH RECURSIVE subordinates AS (
    SELECT employee_id, manager_id, 1 as level
    FROM employees
    WHERE employee_id = 1
    UNION ALL
    SELECT e.employee_id, e.manager_id, s.level + 1
    FROM employees e
    INNER JOIN subordinates s ON e.manager_id = s.employee_id
)
SELECT * FROM subordinates
"#,
    );
    assert!(!source.tree().root_node().has_error());
}
