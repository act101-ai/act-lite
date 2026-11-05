//! Python parsing integration tests.

use act_lite_parser::{Grammar, Parser, QueryType, SourceFile, Workspace, query};
use tempfile::TempDir;

fn create_py_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.py", content.to_string(), Grammar::Python).unwrap()
}

#[test]
fn parses_python_function() {
    let source = create_py_source("def hello():\n    return 'world'");
    assert_eq!(source.grammar(), Grammar::Python);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_python_class() {
    let source = create_py_source(
        r#"
class Calculator:
    def __init__(self, initial: float = 0):
        self._value = initial

    def add(self, n: float) -> "Calculator":
        self._value += n
        return self
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_async_functions() {
    let source = create_py_source(
        r#"
import asyncio

async def fetch_data(url: str) -> dict:
    async with aiohttp.ClientSession() as session:
        async for chunk in response:
            yield chunk
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_imports() {
    let source = create_py_source(
        r#"
import os
import sys
from typing import List, Optional, Dict
from pathlib import Path
from collections.abc import Callable
import json as js
from dataclasses import dataclass, field
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_decorators() {
    let source = create_py_source(
        r#"
from functools import wraps

def my_decorator(func):
    @wraps(func)
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)
    return wrapper

@my_decorator
@staticmethod
def decorated_function():
    pass

@dataclass
class MyData:
    x: int
    y: int
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_comprehensions() {
    let source = create_py_source(
        r#"
# List comprehension
squares = [x**2 for x in range(10) if x % 2 == 0]

# Dict comprehension
mapping = {k: v for k, v in enumerate(items)}

# Set comprehension
unique = {x.lower() for x in words}

# Generator expression
gen = (x**2 for x in range(100))

# Nested comprehension
matrix = [[i * j for j in range(5)] for i in range(5)]
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_type_hints() {
    let source = create_py_source(
        r#"
from typing import List, Dict, Optional, Union, Callable

def process(
    items: List[str],
    mapping: Dict[str, int],
    callback: Callable[[str], bool],
    default: Optional[str] = None,
) -> Union[str, None]:
    return default

class Container[T]:
    def __init__(self, value: T) -> None:
        self.value = value
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_exception_handling() {
    let source = create_py_source(
        r#"
try:
    result = risky_operation()
except ValueError as e:
    print(f"Value error: {e}")
except (TypeError, KeyError):
    print("Type or key error")
except Exception:
    raise
else:
    print("Success")
finally:
    cleanup()
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_context_managers() {
    let source = create_py_source(
        r#"
with open("file.txt") as f:
    content = f.read()

with open("a.txt") as a, open("b.txt") as b:
    pass

async with aiohttp.ClientSession() as session:
    pass
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_pyi_stub_files() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("types.pyi");
    std::fs::write(
        &file_path,
        r#"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...

class MyClass:
    attr: int
    def method(self) -> None: ...
    "#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "module");
    assert!(!tree.root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_py_source(
        r#"
def greet(name: str) -> str:
    return name

class Calculator:
    pass

MAX_VALUE = 100
    "#,
    );

    let result = query::execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_py_source(
        r#"
def foo():
    pass

bar = 1

class Baz:
    pass
    "#,
    );

    let result = query::execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(!symbols.is_empty());
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn parses_python_with_windows_line_endings() {
    let source = create_py_source("def hello():\r\n    return 'world'\r\n");
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_multiline_strings() {
    let source = create_py_source(
        r#"
def foo():
    """
    This is a docstring
    with multiple lines.
    """
    pass

x = """This is a multi-line
string value"""
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_f_strings() {
    let source = create_py_source(
        r#"
name = "World"
greeting = f"Hello, {name}!"
formatted = f"{x + y:>10}"
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_walrus_operator() {
    let source = create_py_source(
        r#"
if (n := 10) > 5:
    print(n)

# In comprehension
squares = [y := x**2 for x in range(10)]
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_match_statement() {
    let source = create_py_source(
        r#"
def handle_status(status):
    match status:
        case "ok":
            return "Success"
        case "error":
            return "Failure"
        case _:
            return "Unknown"
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_nested_classes() {
    let source = create_py_source(
        r#"
class Outer:
    class Inner:
        pass

    def method(self):
        class Local:
            pass
        return Local
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_generators() {
    let source = create_py_source(
        r#"
def countdown(n):
    while n > 0:
        yield n
        n -= 1

async def async_gen():
    yield 1
    yield 2
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_lambda_expressions() {
    let source = create_py_source(
        r#"
from functools import reduce

add = lambda x, y: x + y
map_fn = map(lambda x: x * 2, range(10))
reduce_fn = reduce(lambda acc, x: acc + x, range(10), 0)
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_with_statement_with_parentheses() {
    let source = create_py_source(
        r#"
with (
    open("file1.txt") as f1,
    open("file2.txt") as f2
):
    content = f1.read() + f2.read()
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_star_expressions() {
    let source = create_py_source(
        r#"
# Argument unpacking
args = [1, 2, 3]
print(*args)

kwargs = {"key": "value"}
func(**kwargs)

# Tuple unpacking
first, *rest = range(10)

# Dict unpacking
merged = {**dict1, **dict2}
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn python_workspace_with_multiple_files() {
    let dir = TempDir::new().unwrap();

    // Create test files
    std::fs::write(dir.path().join("utils.py"), "def helper():\n    return 42").unwrap();
    std::fs::write(dir.path().join("main.py"), "class App:\n    pass").unwrap();

    let mut workspace = Workspace::new(dir.path()).unwrap();
    workspace.open_file("utils.py").unwrap();
    workspace.open_file("main.py").unwrap();

    assert_eq!(workspace.file_count(), 2);
}

#[test]
fn python_skeleton_query_integration() {
    let source = SourceFile::from_content(
        "test.py",
        r#"
import os
from typing import List

def main():
    print('hello')

class Application:
    def start(self):
        pass

MAX_ITEMS = 100
"#
        .to_string(),
        Grammar::Python,
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
            assert!(kinds.contains(&"variable"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn python_symbols_query_integration() {
    let source = SourceFile::from_content(
        "test.py",
        r#"
class MyClass:
    def __init__(self):
        self.value = 0

    def process(self):
        return self.value * 2
"#
        .to_string(),
        Grammar::Python,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(symbols.len() >= 3);

            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(names.contains(&"MyClass"));
            assert!(names.contains(&"__init__"));
            assert!(names.contains(&"process"));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn python_symbols_no_duplicate_methods() {
    // BUG-4: The method pattern in symbols.scm was capturing @definition on
    // class_definition instead of function_definition, producing duplicate
    // method entries and wrong ranges.
    let source = SourceFile::from_content(
        "test.py",
        r#"
class MyClass:
    def method_one(self):
        pass

    def method_two(self):
        pass
"#
        .to_string(),
        Grammar::Python,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            // Count how many times each method name appears
            let method_one_count = symbols.iter().filter(|s| s.name() == "method_one").count();
            let method_two_count = symbols.iter().filter(|s| s.name() == "method_two").count();

            assert_eq!(
                method_one_count,
                1,
                "method_one should appear exactly once, got {}. Symbols: {:?}",
                method_one_count,
                symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
            );
            assert_eq!(
                method_two_count, 1,
                "method_two should appear exactly once, got {}",
                method_two_count
            );

            // Verify method ranges don't span the entire class
            let class_symbol = symbols.iter().find(|s| s.name() == "MyClass").unwrap();
            let class_start = class_symbol.range().start().line();
            let class_end = class_symbol.range().end().line();

            for method_name in &["method_one", "method_two"] {
                let method = symbols.iter().find(|s| s.name() == *method_name).unwrap();
                let method_start = method.range().start().line();
                let method_end = method.range().end().line();

                // Method range should NOT equal the class range
                assert!(
                    !(method_start == class_start && method_end == class_end),
                    "Method '{}' range ({}-{}) should not equal class range ({}-{})",
                    method_name,
                    method_start,
                    method_end,
                    class_start,
                    class_end
                );
            }
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn python_skeleton_includes_type_alias() {
    // BUG-3: The @type capture name was not recognized by execute_skeleton(),
    // causing Python 3.12+ type aliases to be silently dropped.
    let source = SourceFile::from_content(
        "test.py",
        r#"
type Alias = int | str

def greet():
    pass
"#
        .to_string(),
        Grammar::Python,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let type_alias = skel.declarations.iter().find(|d| d.kind == "type_alias");
            assert!(
                type_alias.is_some(),
                "Skeleton should include type alias declaration, got: {:?}",
                skel.declarations
                    .iter()
                    .map(|d| format!("{}:{}", d.kind, d.name))
                    .collect::<Vec<_>>()
            );
            assert_eq!(type_alias.unwrap().name, "Alias");
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn python_parse_file_integration() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("sample.py");

    std::fs::write(
        &file_path,
        r#"
def greet(name: str) -> str:
    return f"Hello, {name}!"

class Greeter:
    def say_hello(self):
        print("Hi!")
"#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "module");
    assert!(!tree.root_node().has_error());
}

#[test]
fn python_lsp_query_returns_not_implemented() {
    let source =
        SourceFile::from_content("test.py", "x = 42".to_string(), Grammar::Python).unwrap();

    for query_type in [QueryType::Diagnostics, QueryType::Type, QueryType::Callers] {
        let result = query::execute_query(query_type, &source).unwrap();
        match result {
            act_lite_parser::QueryResult::NotImplemented { .. } => {}
            _ => panic!("Expected NotImplemented for {:?}", query_type),
        }
    }
}

#[test]
fn python_location_is_one_indexed() {
    // Verify that tree-sitter 0-indexed positions are correctly converted to 1-indexed locations
    let source = SourceFile::from_content(
        "test.py",
        r#"
def hello():
    pass

class Foo:
    pass
"#
        .to_string(),
        Grammar::Python,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            for decl in skel.declarations {
                // All locations should be 1-indexed (act-lite-core convention)
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
                assert!(
                    decl.range.end().line() >= 1,
                    "End line should be 1-indexed, got {} for {}",
                    decl.range.end().line(),
                    decl.name
                );
                assert!(
                    decl.range.end().column() >= 1,
                    "End column should be 1-indexed, got {} for {}",
                    decl.range.end().column(),
                    decl.name
                );
            }
        }
        _ => panic!("Expected Skeleton result"),
    }
}
