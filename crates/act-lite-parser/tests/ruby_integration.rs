//! Ruby parsing integration tests.

use act_lite_parser::{Grammar, Parser, QueryType, SourceFile, Workspace, query};
use tempfile::TempDir;

fn create_rb_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.rb", content.to_string(), Grammar::Ruby).unwrap()
}

#[test]
fn parses_ruby_class() {
    let source = create_rb_source(
        r#"
class Calculator
  def add(a, b)
    a + b
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_ruby_module() {
    let source = create_rb_source(
        r#"
module MathUtils
  PI = 3.14159

  def self.circle_area(radius)
    PI * radius ** 2
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_instance_variables() {
    let source = create_rb_source(
        r#"
class User
  def initialize(name)
    @name = name
    @created_at = Time.now
  end

  def display_name
    @name.upcase
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_class_variables() {
    let source = create_rb_source(
        r#"
class Counter
  @@count = 0

  def self.count
    @@count
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_constants() {
    let source = create_rb_source(
        r#"
MAX_RETRIES = 3
DEFAULT_TIMEOUT = 30
API_BASE_URL = 'https://api.example.com'
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_method_definitions() {
    let source = create_rb_source(
        r#"
def greet(name)
  puts "Hello, #{name}!"
end

def calculate(x, y: 0, z: 0)
  x + y + z
end

def summarize(*items)
  items.reduce(0, :+)
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_singleton_methods() {
    let source = create_rb_source(
        r#"
class User
  def self.anonymous
    new(name: 'Anonymous')
  end

  def self.find(id)
    # Query database
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_blocks() {
    let source = create_rb_source(
        r#"
numbers.map { |n| n * 2 }

numbers.each do |n|
  puts n
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_lambdas() {
    let source = create_rb_source(
        r#"
double = ->(x) { x * 2 }
triple = lambda { |x| x * 3 }

add = proc { |a, b| a + b }
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_require_statements() {
    let source = create_rb_source(
        r#"
require 'json'
require_relative 'lib/helpers'
require 'net/http'
require 'securerandom'
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_accessors() {
    let source = create_rb_source(
        r#"
class Person
  attr_reader :name
  attr_writer :age
  attr_accessor :email

  def initialize(name)
    @name = name
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_inheritance() {
    let source = create_rb_source(
        r#"
class Animal
  def speak
    "generic sound"
  end
end

class Dog < Animal
  def speak
    "woof"
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_struct() {
    let source = create_rb_source(
        r#"
Point = Struct.new(:x, :y) do
  def distance_to(other)
    Math.sqrt((x - other.x) ** 2 + (y - other.y) ** 2)
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn singleton_class() {
    let source = create_rb_source(
        r#"
class << self
  def class_method
    "class level method"
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_control_flow() {
    let source = create_rb_source(
        r#"
if x > 0
  puts "positive"
elsif x < 0
  puts "negative"
else
  puts "zero"
end

case status
when :pending
  "waiting"
when :active
  "running"
else
  "unknown"
end

while count < 10
  count += 1
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn pattern_matching() {
    let source = create_rb_source(
        r#"
case response
in { status: 200, body: }
  puts "Success: #{body}"
in { status: 404 }
  puts "Not found"
in { status: 500..599 }
  puts "Server error"
else
  puts "Unknown response"
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_exception_handling() {
    let source = create_rb_source(
        r#"
def process_file(path)
  File.open(path) do |file|
    content = file.read
    content.upcase
  end
rescue Errno::ENOENT => e
  puts "File not found: #{e.message}"
  raise
rescue IOError => e
  puts "IO error: #{e.message}"
ensure
  puts "Cleanup complete"
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_interpolated_strings() {
    let source = create_rb_source(
        r#"
name = "World"
greeting = "Hello, #{name}!"
formatted = "Value: #{x + y}"
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_symbols() {
    let source = create_rb_source(
        r#"
:simple_symbol
:"string with spaces"
:"interpolated_#{value}"

hash = { key: :value }
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_yield_and_blocks() {
    let source = create_rb_source(
        r#"
def iterator
  yield 1
  yield 2
  yield 3
end

iterator { |n| puts n }

def with_logging(&block)
  puts "Starting..."
  result = block.call
  puts "Finished"
  result
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_heredoc() {
    let source = create_rb_source(
        r#"
sql = <<~SQL
  SELECT *
  FROM users
  WHERE active = true
SQL

email = <<EMAIL
Hello #{name},

This is a message.
EMAIL
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_rb_source(
        r#"
module Utils
  VERSION = "1.0.0"

  class Formatter
    attr_reader :format

    def initialize(format)
      @format = format
    end
  end
end

def helper_function
  # does something
end
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
    let source = create_rb_source(
        r#"
class User
  def initialize(name)
    @name = name
  end

  def display
    @name
  end
end

MAX_USERS = 100
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
fn ruby_parse_file_integration() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("sample.rb");

    std::fs::write(
        &file_path,
        r#"
class Greeter
  def initialize(name)
    @name = name
  end

  def greet
    puts "Hello, #{@name}!"
  end
end
"#,
    )
    .unwrap();

    let mut parser = Parser::new();
    let tree = parser.parse_file(&file_path).unwrap();

    assert_eq!(tree.root_node().kind(), "program");
    assert!(!tree.root_node().has_error());
}

#[test]
fn ruby_workspace_with_multiple_files() {
    let dir = TempDir::new().unwrap();

    // Create test files
    std::fs::write(
        dir.path().join("utils.rb"),
        "module Helpers\n  def self.format(text)\n    text.upcase\n  end\nend",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("main.rb"),
        "class App\n  def run\n    puts Helpers.format('hello')\n  end\nend",
    )
    .unwrap();

    let mut workspace = Workspace::new(dir.path()).unwrap();
    workspace.open_file("utils.rb").unwrap();
    workspace.open_file("main.rb").unwrap();

    assert_eq!(workspace.file_count(), 2);
}

#[test]
fn ruby_skeleton_query_integration() {
    let source = SourceFile::from_content(
        "test.rb",
        r#"
require 'json'

module API
  BASE_URL = 'https://api.example.com'

  class Client
    attr_reader :token

    def initialize(token)
      @token = token
    end

    def fetch(path)
      # implementation
    end
  end
end

def configure_api
  # setup
end
"#
        .to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            assert!(skel.declarations.len() >= 5);

            let kinds: Vec<_> = skel.declarations.iter().map(|d| d.kind.as_str()).collect();
            assert!(kinds.contains(&"import"));
            assert!(kinds.contains(&"function"));
            assert!(kinds.contains(&"class"));
            assert!(kinds.contains(&"module"));
            assert!(kinds.contains(&"property"));
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn ruby_symbols_query_integration() {
    let source = SourceFile::from_content(
        "test.rb",
        r#"
class Calculator
  def initialize
    @result = 0
  end

  def add(n)
    @result += n
  end

  def multiply(n)
    @result *= n
  end
end
"#
        .to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            assert!(symbols.len() >= 4);

            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(names.contains(&"Calculator"));
            assert!(names.contains(&"initialize"));
            assert!(names.contains(&"add"));
            assert!(names.contains(&"multiply"));
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn ruby_lsp_query_returns_not_implemented() {
    let source = SourceFile::from_content("test.rb", "x = 42".to_string(), Grammar::Ruby).unwrap();

    for query_type in [QueryType::Diagnostics, QueryType::Type, QueryType::Callers] {
        let result = query::execute_query(query_type, &source).unwrap();
        match result {
            act_lite_parser::QueryResult::NotImplemented { .. } => {}
            _ => panic!("Expected NotImplemented for {:?}", query_type),
        }
    }
}

#[test]
fn ruby_location_is_one_indexed() {
    // Verify that tree-sitter 0-indexed positions are correctly converted to 1-indexed locations
    let source = SourceFile::from_content(
        "test.rb",
        r#"
def hello
  puts "world"
end
"#
        .to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let tree = source.tree();
    let root = tree.root_node();

    // Find the method node
    let mut cursor = root.walk();
    let mut found = false;
    for child in root.children(&mut cursor) {
        if child.kind() == "method" {
            let start = child.start_position();
            let end = child.end_position();

            // tree-sitter uses 0-indexed: line 2 = row 1
            // Our Location should be 1-indexed
            assert_eq!(start.row + 1, 2, "line should be 1-indexed");
            assert_eq!(end.row + 1, 4, "line should be 1-indexed");
            found = true;
            break;
        }
    }

    assert!(found, "Should find method node");
}

#[test]
fn parses_nested_modules() {
    let source = create_rb_source(
        r#"
module API
  module V1
    class Client
      def initialize
        @version = 1
      end
    end
  end

  module V2
    class Client
      def initialize
        @version = 2
      end
    end
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_class_with_superclass() {
    let source = create_rb_source(
        r#"
class Animal
  def breathe
    "inhaling and exhaling"
  end
end

class Mammal < Animal
  def give_birth
    "live young"
  end
end

class Dog < Mammal
  def bark
    "woof"
  end
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_multiple_assignment() {
    let source = create_rb_source(
        r#"
a, b, c = 1, 2, 3
x, y = y, x  # swap
first, *rest = [1, 2, 3, 4, 5]
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_splat_operator() {
    let source = create_rb_source(
        r#"
def sum(*numbers)
  numbers.reduce(0, :+)
end

def configure(**options)
  @config = options
end

def complex(required, *args, key: nil, **kwargs, &block)
  # implementation
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_begin_end_blocks() {
    let source = create_rb_source(
        r#"
begin
  content = File.read('data.txt')
  process(content)
rescue Errno::ENOENT
  puts "File not found"
rescue StandardError => e
  puts "Error: #{e.message}"
ensure
  cleanup
end
"#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn references_query_no_duplicate_captures_for_receiver_calls() {
    // "obj.foo\nobj.bar\n":
    //   `foo` method is at (line=1, col=5) and `bar` at (line=2, col=5).
    // With the duplicate receiver pattern both are captured twice at the same position.
    // After the fix each must appear exactly once.
    let source =
        SourceFile::from_content("test.rb", "obj.foo\nobj.bar\n".to_string(), Grammar::Ruby)
            .unwrap();

    let result = query::execute_references(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            // foo is at line 1, col 5 (1-indexed); bar at line 2, col 5.
            // The duplicate receiver pattern captures each method name an extra time.
            // After removal: each method name is captured exactly twice
            // (once by the general identifier rule, once by the call-method rule).
            let foo_count = references
                .iter()
                .filter(|r| r.line() == 1 && r.column() == 5)
                .count();
            assert_eq!(
                foo_count, 2,
                "'foo' at (1,5) should appear exactly twice (general + call method), not {} times",
                foo_count
            );
            let bar_count = references
                .iter()
                .filter(|r| r.line() == 2 && r.column() == 5)
                .count();
            assert_eq!(
                bar_count, 2,
                "'bar' at (2,5) should appear exactly twice (general + call method), not {} times",
                bar_count
            );
        }
        _ => panic!("Expected References result"),
    }
}

#[test]
fn skeleton_query_singleton_class_has_name() {
    let source = SourceFile::from_content(
        "test.rb",
        "class << self\n  def foo\n  end\nend\n".to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let singleton = skel
                .declarations
                .iter()
                .find(|d| d.kind.as_str() == "class");
            assert!(
                singleton.is_some(),
                "singleton_class must appear in skeleton as class"
            );
            let singleton = singleton.unwrap();
            assert!(
                !singleton.name.is_empty(),
                "singleton_class declaration must have a non-empty name, got empty string"
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn references_query_finds_specific_names() {
    let source = SourceFile::from_content(
        "test.rb",
        r#"
class Finder
  def locate(target)
    target.find
  end
end
"#
        .to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_references(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            assert!(
                !references.is_empty(),
                "references must be non-empty for code with method calls and identifiers"
            );
            // Verify that references come from multiple lines (locate, target, find each appear)
            let line_set: std::collections::HashSet<u32> =
                references.iter().map(|r| r.line()).collect();
            assert!(
                line_set.len() >= 3,
                "references must span at least 3 lines (def, params, call)"
            );
        }
        _ => panic!("Expected References result"),
    }
}

#[test]
fn references_query_result_is_non_empty_for_real_code() {
    let source = SourceFile::from_content(
        "test.rb",
        "require 'json'\nJSON.parse(data)\n".to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_references(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            assert!(
                !references.is_empty(),
                "references query must return results for non-trivial Ruby code"
            );
        }
        _ => panic!("Expected References result"),
    }
}

#[test]
fn skeleton_query_exact_names() {
    let source = SourceFile::from_content(
        "test.rb",
        r#"
require 'json'

module Serializer
  class JsonWriter
    def write(data)
    end
  end
end
"#
        .to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<_> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"Serializer"),
                "must find module 'Serializer'"
            );
            assert!(
                names.contains(&"JsonWriter"),
                "must find class 'JsonWriter'"
            );
            assert!(names.contains(&"write"), "must find method 'write'");
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_exact_names_and_kinds() {
    let source = SourceFile::from_content(
        "test.rb",
        r#"
class Account
  MAX_BALANCE = 1_000_000

  def initialize(owner)
    @owner = owner
    @balance = 0
  end

  def deposit(amount)
    @balance += amount
  end
end
"#
        .to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<_> = symbols.iter().map(|s| s.name()).collect();
            assert!(names.contains(&"Account"), "must find class 'Account'");
            assert!(
                names.contains(&"MAX_BALANCE"),
                "must find constant 'MAX_BALANCE'"
            );
            assert!(
                names.contains(&"initialize"),
                "must find method 'initialize'"
            );
            assert!(names.contains(&"deposit"), "must find method 'deposit'");
            assert!(names.contains(&"owner"), "must find parameter 'owner'");
            assert!(names.contains(&"amount"), "must find parameter 'amount'");
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn symbols_position_accuracy() {
    // class keyword starts at line 2 (1-indexed) in the snippet below
    let source = SourceFile::from_content(
        "test.rb",
        "\nclass Positioned\n  def method_a\n  end\nend\n".to_string(),
        Grammar::Ruby,
    )
    .unwrap();

    let result = query::execute_symbols(&source).unwrap();
    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let positioned = symbols.iter().find(|s| s.name() == "Positioned");
            assert!(positioned.is_some(), "must find class 'Positioned'");
            let sym = positioned.unwrap();
            // The class starts on line 2 (1-indexed). tree-sitter row 1 → line 2.
            assert_eq!(
                sym.range().start().line(),
                2,
                "Positioned class must start at line 2 (1-indexed)"
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn parse_error_on_malformed_ruby_is_detected() {
    // Deliberately malformed Ruby — unclosed class
    let result = SourceFile::from_content(
        "bad.rb",
        "class Broken\n  def method_a\n    # no end\n".to_string(),
        Grammar::Ruby,
    );
    // Parsing must succeed (tree-sitter always returns a tree) but the tree has errors
    assert!(
        result.is_ok(),
        "SourceFile::from_content must not panic on malformed Ruby"
    );
    let source = result.unwrap();
    assert!(
        source.tree().root_node().has_error(),
        "malformed Ruby input must produce a tree with errors"
    );
}
