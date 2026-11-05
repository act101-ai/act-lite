//! JavaScript parsing integration tests.

use act_lite_parser::{Grammar, Parser, QueryType, SourceFile, Workspace, query};
use tempfile::TempDir;

fn create_js_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.js", content.to_string(), Grammar::JavaScript).unwrap()
}

#[test]
fn parses_javascript_function() {
    let source = create_js_source("function hello() {\n    return 'world';\n}");
    assert_eq!(source.grammar(), Grammar::JavaScript);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_javascript_class() {
    let source = create_js_source(
        r#"
class Calculator {
    constructor(initial) {
        this.value = initial;
    }

    add(n) {
        this.value += n;
        return this;
    }

    static create() {
        return new Calculator(0);
    }
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_esm_imports() {
    let source = create_js_source(
        r#"
import fs from 'fs';
import { join, resolve } from 'path';
import * as http from 'http';
import { readFile as read } from 'fs';
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_commonjs_imports() {
    let source = create_js_source(
        r#"
const fs = require('fs');
const { join } = require('path');
const path = require('path');
module.exports = { foo: 'bar' };
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_arrow_functions() {
    let source = create_js_source(
        r#"
const add = (a, b) => a + b;
const greet = name => `Hello, ${name}!`;
const complex = (x) => {
    const result = x * 2;
    return result;
};
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_jsx_syntax() {
    let source = SourceFile::from_content(
        "component.jsx",
        r#"
const Button = ({ label, onClick }) => (
    <button onClick={onClick}>
        {label}
    </button>
);
"#
        .to_string(),
        Grammar::JavaScript,
    )
    .unwrap();
    assert_eq!(source.grammar(), Grammar::JavaScript);
}

#[test]
fn parses_async_await() {
    let source = create_js_source(
        r#"
async function fetchData(url) {
    const response = await fetch(url);
    const data = await response.json();
    return data;
}

const asyncArrow = async () => {
    await Promise.all([task1(), task2()]);
};
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_destructuring() {
    let source = create_js_source(
        r#"
// Object destructuring
const { name, age } = person;
const { name: n, age: a = 30 } = user;

// Array destructuring
const [first, second] = array;
const [head, ...tail] = list;

// Parameter destructuring
function foo({ x, y }) {
    return x + y;
}

const bar = ([a, b]) => a * b;
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_spread_operator() {
    let source = create_js_source(
        r#"
// Object spread
const merged = { ...obj1, ...obj2 };

// Array spread
const combined = [...arr1, ...arr2];

// Function arguments
const result = Math.max(...numbers);

// Rest parameters
function sum(...args) {
    return args.reduce((a, b) => a + b, 0);
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_template_literals() {
    let source = create_js_source(
        r#"
const name = "World";
const greeting = `Hello, ${name}!`;
const multiline = `
    This is a
    multi-line
    string
`;
const tagged = tag`Hello ${name}!`;
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_class_fields() {
    let source = create_js_source(
        r#"
class MyClass {
    static staticField = 'value';
    publicField = 42;
    #privateField = 'private';

    constructor() {
        this.instanceField = 'data';
    }

    method() {
        return this.publicField;
    }
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_export_statements() {
    let source = create_js_source(
        r#"
// Named exports
export const PI = 3.14;
export function square(x) {
    return x * 2;
}
export class Circle {}

// Default export
export default class Calculator {}

// Re-exports
export { foo, bar } from './module.js';
export * from './utils.js';
export { default } from './config.js';
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_promises() {
    let source = create_js_source(
        r#"
const promise = new Promise((resolve, reject) => {
    setTimeout(() => resolve('done'), 1000);
});

promise
    .then(result => console.log(result))
    .catch(error => console.error(error))
    .finally(() => console.log('cleanup'));

Promise.all([p1, p2]).then(results => {});
Promise.race([p1, p2]).then(result => {});
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_object_methods() {
    let source = create_js_source(
        r#"
const obj = {
    method() {
        return 'value';
    },
    async asyncMethod() {
        await task();
    },
    get property() {
        return this._property;
    },
    set property(value) {
        this._property = value;
    },
    *generator() {
        yield 1;
        yield 2;
    }
};
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_generators() {
    let source = create_js_source(
        r#"
function* countGenerator() {
    let count = 0;
    while (true) {
        yield count++;
    }
}

const asyncGen = async function*() {
    yield await Promise.resolve(1);
};

const generatorArrow = function*() {
    yield 1;
};
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_try_catch() {
    let source = create_js_source(
        r#"
try {
    riskyOperation();
} catch (error) {
    console.error(error);
} finally {
    cleanup();
}

try {
    something();
} catch {
    // Catch without binding
    console.log('failed');
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_loops() {
    let source = create_js_source(
        r#"
// For loops
for (let i = 0; i < 10; i++) {
    console.log(i);
}

// For...of
for (const item of array) {
    console.log(item);
}

// For...in
for (const key in obj) {
    console.log(key);
}

// While
while (condition) {
    doSomething();
}

// Do...while
do {
    something();
} while (condition);
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_js_source(
        r#"
function greet(name) {
    return name;
}

const add = (a, b) => a + b;

class Calculator {}
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
    let source = create_js_source(
        r#"
function foo() {}
const bar = 1;
class Baz {}
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
fn javascript_workspace_with_multiple_files() {
    let dir = TempDir::new().unwrap();

    std::fs::write(
        dir.path().join("utils.js"),
        "function helper() { return 42; }",
    )
    .unwrap();
    std::fs::write(dir.path().join("main.js"), "class App {}").unwrap();

    let mut workspace = Workspace::new(dir.path()).unwrap();
    workspace.open_file("utils.js").unwrap();
    workspace.open_file("main.js").unwrap();

    assert_eq!(workspace.file_count(), 2);
}

#[test]
fn javascript_skeleton_query_integration() {
    let source = SourceFile::from_content(
        "test.js",
        r#"
import fs from 'fs';
import path from 'path';

function main() {
    console.log('hello');
}

class Application {
    start() {
        // implementation
    }
}

const MAX_ITEMS = 100;
"#
        .to_string(),
        Grammar::JavaScript,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            // JavaScript skeleton query captures function, class, and top-level variable declarations
            assert!(
                skel.declarations.len() >= 2,
                "Expected at least 2 declarations"
            );

            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(kinds.contains(&"function"), "Expected function declaration");
            assert!(kinds.contains(&"class"), "Expected class declaration");
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn javascript_symbols_query_integration() {
    let source = SourceFile::from_content(
        "test.js",
        r#"
class MyClass {
    constructor() {
        this.value = 0;
    }

    process() {
        return this.value * 2;
    }
}
"#
        .to_string(),
        Grammar::JavaScript,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(symbols.len() >= 3);

            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(names.contains(&"MyClass"));
            assert!(names.contains(&"constructor"));
            assert!(names.contains(&"process"));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn javascript_parse_file_integration() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("sample.js");

    std::fs::write(
        &file_path,
        r#"
function greet(name) {
    return `Hello, ${name}!`;
}

class Greeter {
    sayHello() {
        console.log("Hi!");
    }
}
"#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "program");
    assert!(!tree.root_node().has_error());
}

#[test]
fn javascript_lsp_query_returns_not_implemented() {
    let source =
        SourceFile::from_content("test.js", "const x = 42;".to_string(), Grammar::JavaScript)
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
fn javascript_location_is_one_indexed() {
    // Verify that tree-sitter 0-indexed positions are correctly converted to 1-indexed locations
    let source = SourceFile::from_content(
        "test.js",
        r#"
function hello() {
    return;
}

class Foo {
}
"#
        .to_string(),
        Grammar::JavaScript,
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

#[test]
fn parses_javascript_with_windows_line_endings() {
    let source = create_js_source("function hello() {\r\n    return 'world';\r\n}\r\n");
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_comments() {
    let source = create_js_source(
        r#"
// Single-line comment
const x = 1;

/*
 * Multi-line
 * comment
 */
const y = 2;

/* Inline comment */ const z = 3;
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_switch_statement() {
    let source = create_js_source(
        r#"
switch (value) {
    case 1:
        console.log('one');
        break;
    case 2:
        console.log('two');
        break;
    default:
        console.log('other');
}
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parse_mjs_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.mjs");

    std::fs::write(&file_path, "export const x = 42;").unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "program");
    assert!(!tree.root_node().has_error());
}

#[test]
fn javascript_skeleton_captures_imports() {
    let source = SourceFile::from_content(
        "test.js",
        r#"
import fs from 'fs';
import { join } from 'path';

function main() {}
"#
        .to_string(),
        Grammar::JavaScript,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(
                kinds.contains(&"import"),
                "Expected import in skeleton, got: {:?}",
                kinds
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn javascript_skeleton_captures_methods() {
    let source = SourceFile::from_content(
        "test.js",
        r#"
class App {
    start() {
        console.log('started');
    }
    stop() {
        console.log('stopped');
    }
}
"#
        .to_string(),
        Grammar::JavaScript,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                kinds.contains(&"method"),
                "Expected method in skeleton, got: {:?}",
                kinds
            );
            assert!(
                names.contains(&"start"),
                "Expected method 'start', got: {:?}",
                names
            );
            assert!(
                names.contains(&"stop"),
                "Expected method 'stop', got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn parse_cjs_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.cjs");

    std::fs::write(&file_path, "module.exports = { foo: 'bar' };").unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "program");
    assert!(!tree.root_node().has_error());
}
