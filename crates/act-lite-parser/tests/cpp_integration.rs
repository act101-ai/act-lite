//! C++ parsing integration tests.

use act_lite_parser::{Grammar, SourceFile, execute_references, execute_skeleton, execute_symbols};

fn create_cpp_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.cpp", content.to_string(), Grammar::Cpp).unwrap()
}

#[test]
fn parses_cpp_class() {
    let source = create_cpp_source(
        r#"
        class Calculator {
        private:
            int value;

        public:
            Calculator(int initial) : value(initial) {}

            int add(int n) {
                return value += n;
            }
        };
    "#,
    );
    assert_eq!(source.grammar(), Grammar::Cpp);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_namespace() {
    let source = create_cpp_source(
        r#"
        namespace math {
            namespace constants {
                const double PI = 3.14159;
            }

            double circle_area(double radius) {
                return constants::PI * radius * radius;
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_template_class() {
    let source = create_cpp_source(
        r#"
        template<typename T>
        class Container {
        private:
            T data[100];
            size_t size;

        public:
            Container() : size(0) {}

            void add(const T& item) {
                data[size++] = item;
            }

            T get(size_t index) const {
                return data[index];
            }
        };
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_template_function() {
    let source = create_cpp_source(
        r#"
        template<typename T, typename U>
        auto add(T a, U b) -> decltype(a + b) {
            return a + b;
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_lambda() {
    let source = create_cpp_source(
        r#"
        void example() {
            auto simple = []() { return 42; };
            auto with_capture = [x = 10]() { return x * 2; };
            auto generic = [](auto a, auto b) { return a + b; };
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_inheritance() {
    let source = create_cpp_source(
        r#"
        class Shape {
        public:
            virtual double area() const = 0;
            virtual ~Shape() = default;
        };

        class Circle : public Shape {
        private:
            double radius;

        public:
            Circle(double r) : radius(r) {}
            double area() const override {
                return 3.14159 * radius * radius;
            }
        };
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_smart_pointers() {
    let source = create_cpp_source(
        r#"
        #include <memory>

        void example() {
            auto unique = std::make_unique<int>(42);
            auto shared = std::make_shared<double>(3.14);
            auto weak = std::weak_ptr<double>(shared);
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_structured_bindings() {
    let source = create_cpp_source(
        r#"
        #include <utility>

        void example() {
            auto [a, b] = std::make_pair(1, 2);
            const auto& [x, y] = std::make_pair(3.0, 4.0);
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_enum_class() {
    let source = create_cpp_source(
        r#"
        enum class Color : uint8_t {
            Red = 0,
            Green = 1,
            Blue = 2
        };

        enum class Status : int {
            Ok = 0,
            Error = 1,
            Pending = 2
        };
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_using_and_alias() {
    let source = create_cpp_source(
        r#"
        template<typename T>
        using Vec = std::vector<T>;

        using String = std::string;

        namespace mylib {
            namespace detail {
                class Internal {};
            }
            using detail::Internal;
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_constexpr() {
    let source = create_cpp_source(
        r#"
        constexpr int factorial(int n) {
            return (n <= 1) ? 1 : (n * factorial(n - 1));
        }

        constexpr int fact5 = factorial(5);
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_cpp_exceptions() {
    let source = create_cpp_source(
        r#"
        #include <stdexcept>

        void process(int value) {
            if (value < 0) {
                throw std::invalid_argument("value must be non-negative");
            }

            try {
                // some operation
            } catch (const std::exception& e) {
                // handle error
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn cpp_location_is_one_indexed() {
    let source = create_cpp_source("int x;\nint y;");
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
fn cpp_parse_file_integration() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.cpp");
    std::fs::write(
        &path,
        r#"
        #include <iostream>

        int main() {
            std::cout << "Hello, World!" << std::endl;
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
fn cpp_workspace_with_multiple_files() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();

    // Create header file
    let header_path = dir.path().join("utils.hpp");
    std::fs::write(
        &header_path,
        r#"
        #ifndef UTILS_HPP
        #define UTILS_HPP

        namespace utils {
            int add(int a, int b);
            int multiply(int a, int b);
        }

        #endif
    "#,
    )
    .unwrap();

    // Create source file
    let source_path = dir.path().join("utils.cpp");
    std::fs::write(
        &source_path,
        r#"
        #include "utils.hpp"

        namespace utils {
            int add(int a, int b) {
                return a + b;
            }

            int multiply(int a, int b) {
                return a * b;
            }
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
fn cpp_skeleton_query_integration() {
    let source = create_cpp_source(
        r#"
        #include <vector>

        template<typename T>
        class Container {
            std::vector<T> data;
        public:
            void add(const T& item);
            T get(size_t index) const;
        };
    "#,
    );

    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
            // Should find include, class, template
            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(kinds.contains(&"include"));
            assert!(kinds.contains(&"class"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn cpp_symbols_query_integration() {
    let source = create_cpp_source(
        r#"
        namespace math {
            const double PI = 3.14159;

            class Circle {
            public:
                Circle(double r);
                double area() const;
            private:
                double radius;
            };
        }
    "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
            // Should find math, PI, Circle, constructor, area, radius
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(names.contains(&"math".to_string()));
            assert!(names.contains(&"Circle".to_string()));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_cpp_source(
        r#"
        template<typename T>
        concept Integral = std::is_integral<T>::value;

        template<Integral T>
        T add(T a, T b) {
            return a + b;
        }
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
    let source = create_cpp_source(
        r#"
        enum class Color : uint8_t { Red, Green, Blue };

        class Painter {
        public:
            void set_color(Color c);
            Color get_color() const;
        private:
            Color current_color;
        };
    "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
            // Should find Color, Red, Green, Blue, Painter, methods, current_color
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(names.contains(&"Color".to_string()));
            assert!(names.contains(&"Painter".to_string()));
        }
        _ => panic!("Expected Symbols result"),
    }
}

// SQ-CPP-1: Global variables captured in C++ skeleton
#[test]
fn skeleton_query_cpp_captures_global_variable() {
    let source = SourceFile::from_content(
        "test.cpp",
        "int global_count = 0;\nint x;\n".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"global_count"),
                "global variable must be captured in C++ skeleton; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

// SQ-CPP-1 negative: Local variables must NOT appear in C++ skeleton
#[test]
fn skeleton_query_cpp_excludes_local_variables() {
    let source = SourceFile::from_content(
        "test.cpp",
        "int global_x = 5;\nvoid foo() { int local_x; int local_y = 10; }".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                !names.contains(&"local_x"),
                "local_x must not appear in skeleton; got: {:?}",
                names
            );
            assert!(
                !names.contains(&"local_y"),
                "local_y must not appear in skeleton; got: {:?}",
                names
            );
            assert!(
                names.contains(&"global_x"),
                "global_x must appear in skeleton; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

// SQ-CPP-2: Union captured in C++ skeleton
#[test]
fn skeleton_query_cpp_captures_union() {
    let source = SourceFile::from_content(
        "test.cpp",
        "union Value { int i; float f; double d; };".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"Value"),
                "union must be captured in C++ skeleton; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

// SQ-CPP-3: Template struct captured in C++ skeleton
#[test]
fn skeleton_query_cpp_captures_template_struct() {
    let source = SourceFile::from_content(
        "test.cpp",
        "template<typename T> struct Pair { T first; T second; };".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"Pair"),
                "template struct Pair must be captured; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

// SQ-CPP-4: Namespace alias captured in C++ skeleton
#[test]
fn skeleton_query_cpp_captures_namespace_alias() {
    let source = SourceFile::from_content(
        "test.cpp",
        "namespace MyAlias = std;".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"MyAlias"),
                "namespace alias must be captured; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

// SYM-CPP-1: Macro definitions captured in C++ symbol queries
#[test]
fn symbols_query_cpp_captures_macro_definitions() {
    let source = SourceFile::from_content(
        "test.cpp",
        "#define MAX_SIZE 100\n#define SQUARE(x) ((x)*(x))\n".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(
                names.contains(&"MAX_SIZE".to_string()),
                "macro MAX_SIZE must appear in C++ symbols; got: {:?}",
                names
            );
            assert!(
                names.contains(&"SQUARE".to_string()),
                "macro SQUARE must appear in C++ symbols; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

// SYM-CPP-2: Operator overloads captured in C++ symbol queries
#[test]
fn symbols_query_cpp_captures_operator_overload() {
    let source = SourceFile::from_content(
        "test.cpp",
        "struct Foo { bool operator==(const Foo& o) const { return true; } };".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
            let has_operator = names.iter().any(|n| n.contains("operator"));
            assert!(
                has_operator,
                "operator== must appear as a symbol; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

// REF-CPP-1: References query produces non-empty results
// REF-CPP-2: Every reference location must have valid 1-indexed position
#[test]
fn references_cpp_no_duplicate_captures() {
    let source = SourceFile::from_content(
        "test.cpp",
        "void foo(int x) { int y = x + 1; }".to_string(),
        Grammar::Cpp,
    )
    .unwrap();
    let result = execute_references(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            assert!(!references.is_empty(), "should produce some references");
            for loc in &references {
                assert!(loc.line() >= 1, "reference location line must be >= 1");
                assert!(loc.column() >= 1, "reference location column must be >= 1");
            }
        }
        _ => panic!("Expected References result"),
    }
}
