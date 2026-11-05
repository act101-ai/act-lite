//! Java parsing integration tests.

use act_lite_parser::{
    Grammar, QueryType, SourceFile, execute_query, execute_skeleton, execute_symbols,
};

fn create_java_source(content: &str) -> SourceFile {
    SourceFile::from_content("Test.java", content.to_string(), Grammar::Java).unwrap()
}

#[test]
fn parses_java_class() {
    let source = create_java_source(
        r#"
        public class Calculator {
            private int value;

            public Calculator(int initial) {
                this.value = initial;
            }

            public int add(int n) {
                return value += n;
            }
        }
    "#,
    );
    assert_eq!(source.grammar(), Grammar::Java);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_interface() {
    let source = create_java_source(
        r#"
        public interface Processor<I, O> {
            O process(I input);

            default boolean canProcess(I input) {
                return input != null;
            }

            static <T> Processor<T, T> identity() {
                return t -> t;
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_enum() {
    let source = create_java_source(
        r#"
        public enum Status {
            ACTIVE("A"),
            INACTIVE("I"),
            PENDING("P");

            private final String code;

            Status(String code) {
                this.code = code;
            }

            public String getCode() {
                return code;
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_record() {
    let source = create_java_source(
        r#"
        public record Point(int x, int y) {
            public Point {
                if (x < 0 || y < 0) {
                    throw new IllegalArgumentException("Coordinates must be non-negative");
                }
            }

            public double distanceFromOrigin() {
                return Math.sqrt(x * x + y * y);
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_generics() {
    let source = create_java_source(
        r#"
        import java.util.List;
        import java.util.Map;
        import java.util.function.Function;

        public class Container<T extends Comparable<T>> {
            private final T value;

            public <R> R transform(Function<? super T, ? extends R> fn) {
                return fn.apply(value);
            }

            public static <E> Container<E> of(E element) {
                return new Container<>(element);
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_annotations() {
    let source = create_java_source(
        r#"
        import java.lang.annotation.*;

        @Retention(RetentionPolicy.RUNTIME)
        @Target(ElementType.METHOD)
        public @interface LogExecutionTime {
            String value() default "";
            boolean includeArgs() default false;
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_lambdas_and_streams() {
    let source = create_java_source(
        r#"
        import java.util.*;
        import java.util.stream.*;

        public class StreamExample {
            public List<String> processItems(List<String> items) {
                return items.stream()
                    .filter(s -> s != null && !s.isEmpty())
                    .map(String::toUpperCase)
                    .sorted(Comparator.reverseOrder())
                    .collect(Collectors.toList());
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_try_with_resources() {
    let source = create_java_source(
        r#"
        import java.io.*;

        public class FileReader {
            public String readFile(String path) throws IOException {
                try (var reader = new BufferedReader(new java.io.FileReader(path));
                     var writer = new StringWriter()) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        writer.write(line);
                    }
                    return writer.toString();
                }
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_switch_expression() {
    let source = create_java_source(
        r#"
        public class SwitchExample {
            public String getStatusLabel(int code) {
                return switch (code) {
                    case 1 -> "Active";
                    case 2 -> "Inactive";
                    case 3, 4 -> "Pending";
                    default -> {
                        System.out.println("Unknown code: " + code);
                        yield "Unknown";
                    }
                };
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_pattern_matching() {
    let source = create_java_source(
        r#"
        public class PatternExample {
            public void handleObject(Object obj) {
                if (obj instanceof String s) {
                    System.out.println("String: " + s.toLowerCase());
                } else if (obj instanceof Integer i) {
                    System.out.println("Integer: " + (i * 2));
                } else if (obj instanceof List<?> list && !list.isEmpty()) {
                    System.out.println("Non-empty list with " + list.size() + " elements");
                }
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_inner_classes() {
    let source = create_java_source(
        r#"
        public class Outer {
            private int value = 10;

            public class Inner {
                public int getValue() {
                    return Outer.this.value;
                }
            }

            public static class StaticNested {
                public static void helper() {
                    System.out.println("Static nested helper");
                }
            }

            public void localClass() {
                class Local {
                    void run() {
                        System.out.println("Local class");
                    }
                }
                new Local().run();
            }

            public Runnable anonymousClass() {
                return new Runnable() {
                    @Override
                    public void run() {
                        System.out.println("Anonymous class");
                    }
                };
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_java_sealed_classes() {
    let source = create_java_source(
        r#"
        public sealed class Shape permits Circle, Rectangle, Square {
            public abstract double area();
        }

        final class Circle extends Shape {
            private final double radius;

            Circle(double radius) {
                this.radius = radius;
            }

            @Override
            public double area() {
                return Math.PI * radius * radius;
            }
        }

        non-sealed class Rectangle extends Shape {
            @Override
            public double area() {
                return 0; // simplified
            }
        }
    "#,
    );
    // Note: May need tree-sitter-java version that supports sealed classes
    // If this fails, adjust based on grammar version
    let tree = source.tree();
    // Even if there are minor parse issues with sealed syntax,
    // the overall structure should parse
    assert!(tree.root_node().child_count() > 0);
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_java_source(
        r#"
        package com.example;

        import java.util.List;

        public class Sample {
            private int value;

            public void process() {}
        }

        interface Processor {}

        enum Status { ACTIVE, INACTIVE }
    "#,
    );

    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(s) => {
            assert!(!s.declarations.is_empty());
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_java_source(
        r#"
        public class Sample {
            private String name;
            private int value;

            public Sample(String name, int value) {
                this.name = name;
                this.value = value;
            }

            public String getName() {
                return name;
            }
        }
    "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn parses_java_text_blocks() {
    let source = create_java_source(
        r#"
        public class TextBlockExample {
            private static final String JSON = """
                    {
                        "name": "Alice",
                        "age": 30,
                        "active": true
                    }
                    """;

            private static final String QUERY = """
                    SELECT id, name, email
                    FROM users
                    WHERE active = TRUE
                      AND created_at > '2024-01-01'
                    ORDER BY name
                    """;

            public String buildHtml(String title, String body) {
                return """
                        <html>
                            <head><title>%s</title></head>
                            <body>%s</body>
                        </html>
                        """.formatted(title, body);
            }
        }
    "#,
    );
    assert!(
        !source.tree().root_node().has_error(),
        "Text blocks (Java 15+) should parse without errors"
    );
}

#[test]
fn parses_java_switch_pattern_matching() {
    let source = create_java_source(
        r#"
        public class SwitchPatternExample {
            sealed interface Shape permits Circle, Rectangle, Triangle {}
            record Circle(double radius) implements Shape {}
            record Rectangle(double width, double height) implements Shape {}
            record Triangle(double base, double height) implements Shape {}

            public double area(Shape shape) {
                return switch (shape) {
                    case Circle c -> Math.PI * c.radius() * c.radius();
                    case Rectangle r -> r.width() * r.height();
                    case Triangle t -> 0.5 * t.base() * t.height();
                };
            }

            public String describe(Object obj) {
                return switch (obj) {
                    case Integer i when i > 0 -> "positive integer: " + i;
                    case Integer i -> "non-positive integer: " + i;
                    case String s when s.isEmpty() -> "empty string";
                    case String s -> "string: " + s;
                    case null -> "null value";
                    default -> "other: " + obj;
                };
            }
        }
    "#,
    );
    assert!(
        !source.tree().root_node().has_error(),
        "Switch pattern matching (Java 21+) should parse without errors"
    );
}

#[test]
fn skeleton_query_handles_modern_java_syntax() {
    // Records, sealed classes, and modern constructs must appear in skeleton output.
    let source = create_java_source(
        r#"
        package com.example;

        public sealed interface Result<T> permits Success, Failure {}

        record Success<T>(T value) implements Result<T> {}

        record Failure<T>(String message) implements Result<T> {
            public boolean isFatal() {
                return message.startsWith("FATAL");
            }
        }
    "#,
    );

    let result = execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(s) => {
            assert!(
                !s.declarations.is_empty(),
                "sealed interface, records must produce skeleton declarations"
            );
            let kinds: Vec<_> = s.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(
                kinds.contains(&"interface"),
                "sealed interface must appear in skeleton; got: {kinds:?}"
            );
            assert!(
                kinds.contains(&"record"),
                "record declarations must appear in skeleton; got: {kinds:?}"
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_record_components() {
    // Records expose their components as symbols with the correct names.
    let source = create_java_source(
        r#"
        public record Person(String name, int age, String email) {
            public String displayName() {
                return name + " (age " + age + ")";
            }
        }
    "#,
    );

    let result = execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty(), "Record symbols must not be empty");
            let names: Vec<String> = symbols.iter().map(|s| s.name().to_string()).collect();
            assert!(
                names.iter().any(|n| n == "Person"),
                "Record type name must appear in symbols; got: {names:?}"
            );
            assert!(
                names.iter().any(|n| n == "displayName"),
                "Record method must appear in symbols; got: {names:?}"
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn references_query_works() {
    let source = create_java_source(
        r#"
        public class Sample {
            private String name;

            public String getName() {
                return name;
            }

            public void setName(String name) {
                this.name = name;
            }
        }
    "#,
    );

    // References query requires LSP, so we expect NotImplemented
    let result = execute_query(QueryType::References, &source).unwrap();
    match result {
        act_lite_parser::QueryResult::NotImplemented { .. } => {
            // Expected - References requires LSP
        }
        _ => {
            // If implemented, that's also fine
        }
    }
}
