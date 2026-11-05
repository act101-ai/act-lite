//! Swift parsing integration tests.

use act_lite_parser::{
    Grammar, QueryType, SourceFile, execute_query, execute_skeleton, execute_symbols,
};

fn create_swift_source(content: &str) -> SourceFile {
    SourceFile::from_content("Test.swift", content.to_string(), Grammar::Swift).unwrap()
}

#[test]
fn parses_swift_function() {
    let source = create_swift_source("func hello() -> String { return \"world\" }");
    assert_eq!(source.grammar(), Grammar::Swift);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_struct_with_properties() {
    let source = create_swift_source(
        r#"
        struct User {
            let id: Int
            var name: String
            var email: String?
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_class() {
    let source = create_swift_source(
        r#"
        class ViewController: UIViewController {
            var label: UILabel?

            override func viewDidLoad() {
                super.viewDidLoad()
                setup()
            }

            private func setup() {
                // Setup code
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_protocol() {
    let source = create_swift_source(
        r#"
        protocol Repository {
            associatedtype T
            func findById(_ id: Int) -> T?
            func save(_ item: T) throws
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_enum() {
    let source = create_swift_source(
        r#"
        enum Result<T> {
            case success(T)
            case failure(Error)

            var isSuccess: Bool {
                if case .success = self {
                    return true
                }
                return false
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_extension() {
    let source = create_swift_source(
        r#"
        extension String {
            func isValidEmail() -> Bool {
                return contains("@") && contains(".")
            }

            var uppercased: String {
                return self.uppercased()
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_generics() {
    let source = create_swift_source(
        r#"
        func swap<T>(_ a: inout T, _ b: inout T) {
            let temp = a
            a = b
            b = temp
        }

        struct Stack<Element> {
            private var items: [Element] = []

            mutating func push(_ item: Element) {
                items.append(item)
            }

            mutating func pop() -> Element? {
                return items.popLast()
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_closures() {
    let source = create_swift_source(
        r#"
        let numbers = [1, 2, 3, 4, 5]

        let doubled = numbers.map { $0 * 2 }

        let evenNumbers = numbers.filter { number in
            return number % 2 == 0
        }

        let sum = numbers.reduce(0) { $0 + $1 }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_optionals() {
    let source = create_swift_source(
        r#"
        var optionalString: String? = "Hello"

        if let unwrapped = optionalString {
            print(unwrapped)
        }

        let name = optionalString ?? "Default"

        guard let value = optionalString else {
            return
        }

        optionalString?.append(" World")
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_property_wrappers() {
    let source = create_swift_source(
        r#"
        @propertyWrapper
        struct Clamped<T: Comparable> {
            private var value: T
            private let range: ClosedRange<T>

            var wrappedValue: T {
                get { value }
                set { value = min(max(range.lowerBound, newValue), range.upperBound) }
            }

            init(wrappedValue: T, _ range: ClosedRange<T>) {
                self.range = range
                self.value = min(max(range.lowerBound, wrappedValue), range.upperBound)
            }
        }

        struct Settings {
            @Clamped(0...100) var volume: Int = 50
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_async_await() {
    let source = create_swift_source(
        r#"
        func fetchData(from url: URL) async throws -> Data {
            let (data, _) = try await URLSession.shared.data(from: url)
            return data
        }

        func processData() async {
            do {
                let url = URL(string: "https://example.com")!
                let data = try await fetchData(from: url)
                print("Received \(data.count) bytes")
            } catch {
                print("Error: \(error)")
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_computed_properties() {
    let source = create_swift_source(
        r#"
        struct Rectangle {
            var width: Double
            var height: Double

            var area: Double {
                return width * height
            }

            var perimeter: Double {
                get {
                    return 2 * (width + height)
                }
            }

            var aspectRatio: Double {
                get { width / height }
                set { width = height * newValue }
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_subscripts() {
    let source = create_swift_source(
        r#"
        struct Matrix {
            private var grid: [[Int]]

            subscript(row: Int, column: Int) -> Int {
                get {
                    return grid[row][column]
                }
                set {
                    grid[row][column] = newValue
                }
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_swift_type_aliases() {
    let source = create_swift_source(
        r#"
        typealias UserID = Int
        typealias Coordinate = (x: Double, y: Double)
        typealias CompletionHandler = (Result<Data, Error>) -> Void

        func fetchUser(id: UserID, completion: @escaping CompletionHandler) {
            // Implementation
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_swift_source(
        r#"
        import Foundation

        func greet(name: String) -> String {
            return "Hello, \(name)!"
        }

        struct User {
            let id: Int
            var name: String
        }

        protocol Repository {
            func save(_ item: User)
        }

        class UserService {
            func findUser(id: Int) -> User? {
                return nil
            }
        }
        "#,
    );

    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(s) => {
            // Must find the import
            assert!(
                s.declarations.iter().any(|d| d.kind == "import"),
                "Expected import declaration"
            );
            // Must find greet function
            assert!(
                s.declarations
                    .iter()
                    .any(|d| d.kind == "function" && d.name == "greet"),
                "Expected greet function"
            );
            // Must find User (struct — represented as class_declaration in tree-sitter-swift)
            assert!(
                s.declarations.iter().any(|d| d.name == "User"),
                "Expected User type"
            );
            // Must find Repository (protocol → interface)
            assert!(
                s.declarations
                    .iter()
                    .any(|d| d.kind == "interface" && d.name == "Repository"),
                "Expected Repository interface"
            );
            // Must find UserService (class)
            assert!(
                s.declarations.iter().any(|d| d.name == "UserService"),
                "Expected UserService class"
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_swift_source(
        r#"
        func foo() {}
        let bar = 1
        class Baz {
            var field = "value"
            func method() {}
        }
        "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"foo"),
                "Expected foo function symbol; got: {:?}",
                names
            );
            assert!(
                names.contains(&"Baz"),
                "Expected Baz class symbol; got: {:?}",
                names
            );
            assert!(
                names.contains(&"method"),
                "Expected method symbol; got: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_includes_init_declaration() {
    let source = create_swift_source(
        r#"
        class User {
            let id: Int
            init(id: Int) {
                self.id = id
            }
        }
        "#,
    );
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(s) => {
            // User class must appear
            assert!(
                s.declarations.iter().any(|d| d.name == "User"),
                "Expected User class in skeleton"
            );
            assert!(
                s.declarations
                    .iter()
                    .any(|d| d.kind == "function" && d.name == "init"),
                "Expected init_declaration as function named \"init\" in skeleton"
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_includes_init_declaration() {
    let source = create_swift_source(
        r#"
        struct Point {
            var x: Double
            var y: Double
            init(x: Double, y: Double) {
                self.x = x
                self.y = y
            }
        }
        "#,
    );
    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let has_init = symbols
                .iter()
                .any(|s| s.kind() == act_lite_core::SymbolKind::Constructor);
            assert!(has_init, "Expected init_declaration in symbols");
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn references_query_returns_references() {
    // Source includes simple_identifier, type_identifier, navigation_expression,
    // and call_expression — exercising all four capture patterns in references.scm.
    let source = create_swift_source(
        r#"
        class Engine {
            var rpm: Int = 0
            func start() {}
        }
        class Car {
            let engine = Engine()
            func drive() {
                engine.start()
                engine.rpm = 3000
                let speed = engine.rpm
                _ = speed
            }
        }
        "#,
    );
    let result = execute_query(QueryType::References, &source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            // The scoped references.scm excludes declaration sites and captures:
            //   - call_expression > simple_identifier: "Engine" (1)
            //   - navigation_expression > simple_identifier: "engine" x3 (3)
            //   - navigation_suffix > simple_identifier: "start", "rpm", "rpm" (3)
            //   - user_type > type_identifier: "Int" (1)
            // Total: 8 references. The threshold verifies all four patterns fire.
            assert!(
                references.len() >= 8,
                "Expected at least 8 references from a source with calls and member access; got {}",
                references.len()
            );
        }
        other => panic!("Expected References result, got {:?}", other),
    }
}

#[test]
fn skeleton_distinguishes_struct_from_class() {
    let source = create_swift_source(
        r#"
        struct Point { var x: Double; var y: Double }
        class Node { var value: Int = 0 }
        enum Status { case active, inactive }
        actor Worker {}
        extension String {}
        "#,
    );
    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(s) => {
            let point = s
                .declarations
                .iter()
                .find(|d| d.name == "Point")
                .expect("Expected Point struct");
            assert_eq!(
                point.kind, "struct",
                "Point must be labelled as struct, got: {}",
                point.kind
            );

            let node = s
                .declarations
                .iter()
                .find(|d| d.name == "Node")
                .expect("Expected Node class");
            assert_eq!(node.kind, "class", "Node must be labelled as class");

            let status = s
                .declarations
                .iter()
                .find(|d| d.name == "Status")
                .expect("Expected Status enum");
            assert_eq!(
                status.kind, "enum",
                "Status must be labelled as enum, got: {}",
                status.kind
            );

            // actor and extension currently map to class (acceptable)
        }
        _ => panic!("Expected Skeleton result"),
    }
}
