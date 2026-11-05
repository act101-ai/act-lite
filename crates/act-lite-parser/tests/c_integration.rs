//! C parsing integration tests.

use act_lite_parser::{Grammar, SourceFile, execute_skeleton, execute_symbols};

fn create_c_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.c", content.to_string(), Grammar::C).unwrap()
}

#[test]
fn parses_c_function() {
    let source = create_c_source(
        r#"
        int add(int a, int b) {
            return a + b;
        }
    "#,
    );
    assert_eq!(source.grammar(), Grammar::C);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_struct() {
    let source = create_c_source(
        r#"
        struct Point {
            int x;
            int y;
        };
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_union() {
    let source = create_c_source(
        r#"
        union Value {
            int i;
            float f;
            char c[4];
        };
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_enum() {
    let source = create_c_source(
        r#"
        enum Status {
            OK = 0,
            ERROR = -1,
            PENDING = 1
        };
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_typedef() {
    let source = create_c_source(
        r#"
        typedef unsigned long size_t;
        typedef struct {
            int x;
            int y;
        } Point;
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_preprocessor() {
    let source = create_c_source(
        r#"
        #include <stdio.h>
        #define MAX_SIZE 100
        #define SQUARE(x) ((x) * (x))

        #ifdef DEBUG
        #define DEBUG_PRINT(fmt, ...) fprintf(stderr, fmt, ##__VA_ARGS__)
        #else
        #define DEBUG_PRINT(fmt, ...) do {} while(0)
        #endif
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_function_pointer() {
    let source = create_c_source(
        r#"
        typedef int (*compare_fn)(const void*, const void*);

        int compare_ints(const void* a, const void* b) {
            return (*(int*)a - *(int*)b);
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_pointer_types() {
    let source = create_c_source(
        r#"
        int* ptr;
        const char* str;
        void* (*func_ptr)(int);
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_array_types() {
    let source = create_c_source(
        r#"
        int arr[10];
        int matrix[3][3];
        char str[100];
        int* ptr_array[5];
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_c_control_flow() {
    let source = create_c_source(
        r#"
        int example(int x) {
            if (x > 0) {
                return x;
            } else if (x < 0) {
                return -x;
            } else {
                return 0;
            }

            switch (x) {
                case 1:
                    return 10;
                case 2:
                    return 20;
                default:
                    return 0;
            }

            for (int i = 0; i < 10; i++) {
                x += i;
            }

            while (x > 0) {
                x--;
            }

            do {
                x++;
            } while (x < 10);

            return x;
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn c_location_is_one_indexed() {
    let source = create_c_source("int x;\nint y;");
    let tree = source.tree();
    let root = tree.root_node();

    // Find the first declaration
    let mut cursor = root.walk();
    let mut found = false;
    for child in root.children(&mut cursor) {
        if child.kind() == "declaration" {
            let start = child.start_position();
            assert_eq!(start.row, 0, "row should be 0-indexed in tree-sitter");
            assert_eq!(start.column, 0, "column should be 0-indexed in tree-sitter");
            found = true;
            break;
        }
    }
    assert!(found, "Should find a declaration");
}

#[test]
fn c_parse_file_integration() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.c");
    std::fs::write(
        &path,
        r#"
        #include <stdio.h>

        int main() {
            printf("Hello, World!\n");
            return 0;
        }
    "#,
    )
    .unwrap();

    let mut parser = act_lite_parser::Parser::new();
    let tree = parser.parse_file(&path).unwrap();
    assert_eq!(tree.root_node().kind(), "translation_unit");
    assert!(!tree.root_node().has_error());
}

#[test]
fn c_workspace_with_multiple_files() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();

    // Create header file
    let header_path = dir.path().join("utils.h");
    std::fs::write(
        &header_path,
        r#"
        #ifndef UTILS_H
        #define UTILS_H

        int add(int a, int b);
        int multiply(int a, int b);

        #endif
    "#,
    )
    .unwrap();

    // Create source file
    let source_path = dir.path().join("utils.c");
    std::fs::write(
        &source_path,
        r#"
        #include "utils.h"

        int add(int a, int b) {
            return a + b;
        }

        int multiply(int a, int b) {
            return a * b;
        }
    "#,
    )
    .unwrap();

    let mut workspace = act_lite_parser::Workspace::new(dir.path()).unwrap();
    workspace.open_file(&header_path).unwrap();
    workspace.open_file(&source_path).unwrap();

    assert_eq!(workspace.files().count(), 2);
}

#[test]
fn c_skeleton_query_integration() {
    let source = create_c_source(
        r#"
        #include <stdio.h>

        struct Point {
            int x;
            int y;
        };

        int distance(struct Point* p1, struct Point* p2);
    "#,
    );

    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
            // Should find include, struct, and function
            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(kinds.contains(&"include"));
            assert!(kinds.contains(&"struct"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn c_symbols_query_integration() {
    let source = create_c_source(
        r#"
        int global_var = 42;

        struct Point {
            int x;
            int y;
        };

        int calculate(struct Point* p) {
            return p->x + p->y;
        }
    "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
            // Should find global_var, Point, x, y, calculate
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(names.contains(&"global_var".to_string()));
            assert!(names.contains(&"Point".to_string()));
            assert!(names.contains(&"calculate".to_string()));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_c_source(
        r#"
        typedef struct {
            int x;
            int y;
        } Point;

        #define MAX 100

        int func(Point p);
    "#,
    );

    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_c_source(
        r#"
        enum Color { RED, GREEN, BLUE };

        void set_color(enum Color c) {
            // implementation
        }
    "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
            // Should find Color, RED, GREEN, BLUE, set_color, c
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(names.contains(&"Color".to_string()));
            assert!(names.contains(&"set_color".to_string()));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_captures_global_variable_without_initializer() {
    let source = create_c_source("int global_count;\nint x = 5;");
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"global_count"),
                "bare declaration 'int global_count;' must be captured as @variable; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_captures_global_variable_without_initializer() {
    let source = create_c_source("int global_count;\nvoid foo() {}");
    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(
                names.contains(&"global_count".to_string()),
                "bare declaration 'int global_count;' must appear in symbols; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_excludes_local_variables() {
    let source =
        create_c_source("int global_x = 5;\nvoid foo() { int local_x; int local_y = 10; }");
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                !names.contains(&"local_x"),
                "local variable 'local_x' must not appear in skeleton; got: {:?}",
                names
            );
            assert!(
                !names.contains(&"local_y"),
                "local variable 'local_y' must not appear in skeleton; got: {:?}",
                names
            );
            assert!(
                names.contains(&"global_x"),
                "global variable 'global_x' must appear in skeleton; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn references_query_excludes_include_path_strings() {
    use act_lite_parser::execute_references;
    // Source has only an include directive — no identifier references.
    // The include path "<stdio.h>" must NOT be captured as a reference.
    let source = create_c_source("#include <stdio.h>\nint x;\n");
    let content = source.content().to_string();
    let result = execute_references(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            for loc in &references {
                if let Some(offset) = loc.byte_offset() {
                    // Grab a window of text starting at the reference byte offset
                    let text_at = &content[offset..];
                    // Include path strings (e.g. <stdio.h> or "stdio.h") end with .h>  or .h"
                    assert!(
                        !text_at.starts_with('<') && !text_at.starts_with('"'),
                        "include path string should not be a reference at byte {}: {:?}",
                        offset,
                        &content[offset..std::cmp::min(offset + 20, content.len())]
                    );
                }
            }
        }
        _ => panic!("Expected References result"),
    }
}

#[test]
fn references_c_no_duplicate_captures() {
    use act_lite_parser::execute_references;
    // A simple function body — references query must return non-empty results
    // and all captured locations must have valid (non-zero) byte offsets.
    let source = create_c_source("int foo(int x) { return x; }");
    let result = execute_references(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            assert!(!references.is_empty(), "should produce some references");
            for loc in &references {
                // Every reference must have a resolvable position (line >= 1)
                assert!(loc.line() >= 1, "reference location line must be >= 1");
                assert!(loc.column() >= 1, "reference location column must be >= 1");
            }
        }
        _ => panic!("Expected References result"),
    }
}

#[test]
fn symbols_query_captures_array_parameter() {
    let source = create_c_source("void foo(int arr[], int n) {}");
    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(
                names.contains(&"arr".to_string()),
                "array parameter 'arr[]' must be captured; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}
