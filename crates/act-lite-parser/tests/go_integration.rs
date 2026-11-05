//! Go parsing integration tests.

use act_lite_parser::{Grammar, SourceFile, execute_skeleton, execute_symbols};

fn create_go_source(content: &str) -> SourceFile {
    SourceFile::from_content("Test.go", content.to_string(), Grammar::Go).unwrap()
}

#[test]
fn parses_go_function() {
    let source = create_go_source(
        r#"
package main

func hello() string {
    return "world"
}
"#,
    );
    assert_eq!(source.grammar(), Grammar::Go);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_struct() {
    let source = create_go_source(
        r#"
package main

type User struct {
    ID    int    `json:"id"`
    Name  string `json:"name"`
    Email string `json:"email,omitempty"`
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_interface() {
    let source = create_go_source(
        r#"
package main

type UserService interface {
    GetUser(id int) (*User, error)
    CreateUser(name, email string) (*User, error)
    DeleteUser(id int) error
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_methods() {
    let source = create_go_source(
        r#"
package main

type Counter struct {
    value int
}

func (c *Counter) Increment() {
    c.value++
}

func (c Counter) Value() int {
    return c.value
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_imports() {
    let source = create_go_source(
        r#"
package main

import (
    "fmt"
    "net/http"

    myalias "github.com/user/package"
    . "embed"
    _ "database/sql/driver"
)
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_goroutines() {
    let source = create_go_source(
        r#"
package main

func main() {
    ch := make(chan int)

    go func() {
        ch <- 42
    }()

    result := <-ch
    _ = result
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_generics() {
    let source = create_go_source(
        r#"
package main

func Map[T, U any](items []T, fn func(T) U) []U {
    result := make([]U, len(items))
    for i, item := range items {
        result[i] = fn(item)
    }
    return result
}

type Stack[T any] struct {
    items []T
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_defer() {
    let source = create_go_source(
        r#"
package main

import "os"

func readFile(path string) error {
    f, err := os.Open(path)
    if err != nil {
        return err
    }
    defer f.Close()
    return nil
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_go_source(
        r#"
package main

type User struct {
    Name string
}

func NewUser(name string) *User {
    return &User{Name: name}
}
"#,
    );

    let result = execute_skeleton(&source).unwrap();
    // Result should be a SkeletonResult with declarations
    match result {
        act_lite_parser::QueryResult::Skeleton(s) => {
            assert!(!s.declarations.is_empty());
        }
        _ => panic!("Expected SkeletonResult"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_go_source(
        r#"
package main

const MaxSize = 100

var globalConfig = map[string]string{}

type Config struct {
    Name string
}

func Setup() {}
"#,
    );

    let result = execute_symbols(&source).unwrap();
    // Result should be Symbols with a vector
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
        }
        _ => panic!("Expected Symbols variant"),
    }
}

#[test]
fn parses_go_select_statement() {
    let source = create_go_source(
        r#"
package main

func selectExample() {
    ch1 := make(chan int)
    ch2 := make(chan int)

    select {
    case val := <-ch1:
        _ = val
    case val := <-ch2:
        _ = val
    }
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_type_switch() {
    let source = create_go_source(
        r#"
package main

func typeSwitch(i interface{}) string {
    switch v := i.(type) {
    case int:
        return "int"
    case string:
        return "string"
    default:
        return "unknown"
    }
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_const_declaration() {
    let source = create_go_source(
        r#"
package main

const (
    MaxUsers    = 1000
    DefaultPort = 8080
    APIVersion  = "v1"
)
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_var_declaration() {
    let source = create_go_source(
        r#"
package main

var (
    globalConfig = make(map[string]string)
    instanceCount int
    debugMode     bool = false
)
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_go_short_var_declaration_structure() {
    let source = create_go_source(
        r#"
package main

func main() {
    nums := [3]int{1, 2, 3}
}
"#,
    );
    assert!(!source.tree().root_node().has_error());

    // Find the short_var_declaration node
    let tree = source.tree();
    let root = tree.root_node();
    let source_bytes = source.content().as_bytes();

    fn find_short_var_decl(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
        if node.kind() == "short_var_declaration" {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_short_var_decl(child) {
                return Some(found);
            }
        }
        None
    }

    let short_var = find_short_var_decl(root).expect("Should find short_var_declaration");

    // Check structure: short_var_declaration has left:expression_list with identifier children
    // nums := [3]int{1, 2, 3}
    let left = short_var
        .child_by_field_name("left")
        .expect("Should have left field");
    assert_eq!(left.kind(), "expression_list");

    // The expression_list should contain an identifier
    let mut cursor = left.walk();
    let identifier = left
        .children(&mut cursor)
        .next()
        .expect("Should have children");
    assert_eq!(identifier.kind(), "identifier");

    let name = identifier.utf8_text(source_bytes).unwrap();
    assert_eq!(name, "nums");
}

#[test]
fn parses_go_var_spec_structure() {
    let source = create_go_source(
        r#"
package main

var count int
"#,
    );
    assert!(!source.tree().root_node().has_error());

    // Find the var_spec node
    let tree = source.tree();
    let root = tree.root_node();
    let source_bytes = source.content().as_bytes();

    fn find_var_spec(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
        if node.kind() == "var_spec" {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_var_spec(child) {
                return Some(found);
            }
        }
        None
    }

    let var_spec = find_var_spec(root).expect("Should find var_spec");

    // var_spec should have a name field with the identifier
    let name = var_spec
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source_bytes).ok())
        .expect("Should have name");

    assert_eq!(name, "count");
}

#[test]
fn parses_go_short_var_declaration_type_structure() {
    let source = create_go_source(
        r#"
package main

func main() {
    nums := [3]int{1, 2, 3}
}
"#,
    );
    assert!(!source.tree().root_node().has_error());

    // Find the short_var_declaration node
    let tree = source.tree();
    let root = tree.root_node();

    fn find_short_var_decl(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
        if node.kind() == "short_var_declaration" {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_short_var_decl(child) {
                return Some(found);
            }
        }
        None
    }

    let short_var = find_short_var_decl(root).expect("Should find short_var_declaration");

    // Check structure: short_var_declaration has left:expression_list with identifier children
    // and right:expression_list with the value
    // nums := [3]int{1, 2, 3}
    let left = short_var
        .child_by_field_name("left")
        .expect("Should have left field");
    assert_eq!(left.kind(), "expression_list");

    let right = short_var
        .child_by_field_name("right")
        .expect("Should have right field");
    assert_eq!(right.kind(), "expression_list");

    // The right side should contain the type expression [3]int{1, 2, 3}
    let mut cursor = right.walk();
    let first_child = right
        .children(&mut cursor)
        .next()
        .expect("Should have right children");
    // This should be an array_type or composite_literal
    let kind = first_child.kind();
    // In Go tree-sitter, [3]int{1, 2, 3} is a composite_literal with array_type inside
    assert!(matches!(kind, "composite_literal" | "array_type"));
}
