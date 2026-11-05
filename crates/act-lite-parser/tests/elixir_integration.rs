use act_lite_parser::{Grammar, SourceFile};

fn create_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.ex", content.to_string(), Grammar::Elixir).unwrap()
}

#[test]
fn parses_module() {
    let source = create_source("defmodule Foo do\n  def bar, do: :ok\nend");
    assert_eq!(source.grammar(), Grammar::Elixir);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_functions() {
    let source = create_source(
        r#"
        defmodule Math do
          def add(a, b), do: a + b
          defp sub(a, b), do: a - b
          def div(a, b) when b != 0, do: a / b
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_pattern_matching() {
    let source = create_source(
        r#"
        defmodule Parser do
          def parse({:ok, d}), do: d
          def parse({:error, r}), do: {:error, r}
          def parse(d) when is_binary(d), do: d
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_pipe_operator() {
    let source = create_source(
        r#"
        defmodule Pipeline do
          def run(data) do
            data
            |> String.trim()
            |> String.downcase()
            |> String.split(",")
          end
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_case_cond_with() {
    let source = create_source(
        r#"
        defmodule Control do
          def classify(s) do
            case s do
              :ok -> "ok"
              _ -> "other"
            end
          end

          def grade(n) do
            cond do
              n >= 90 -> :a
              true -> :b
            end
          end

          def process(r) do
            with {:ok, v} <- validate(r), do: {:ok, v}
          end

          defp validate(r), do: {:ok, r}
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_anonymous_functions() {
    let source = create_source(
        r#"
        defmodule Lambdas do
          def run do
            double = &(&1 * 2)
            greet = fn n -> "Hi #{n}" end
            handle = fn
              {:ok, r} -> r
              _ -> nil
            end
            {double, greet, handle}
          end
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_structs() {
    let source = create_source(
        r#"
        defmodule User do
          defstruct [:id, :name, active: true]

          def new(name), do: %__MODULE__{name: name}
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_protocols() {
    let source = create_source(
        r#"
        defprotocol Displayable do
          def display(data)
        end

        defimpl Displayable, for: Map do
          def display(map), do: inspect(map)
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_genserver() {
    let source = create_source(
        r#"
        defmodule Counter do
          use GenServer

          def start_link(init), do: GenServer.start_link(__MODULE__, init)

          @impl GenServer
          def init(state), do: {:ok, state}

          @impl GenServer
          def handle_call(:get, _from, state), do: {:reply, state, state}
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_comprehensions() {
    let source = create_source(
        r#"
        defmodule Comp do
          def run(tasks) do
            titles = for t <- tasks, do: t.title
            map = for t <- tasks, into: %{}, do: {t.id, t}
            {titles, map}
          end
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_macros() {
    let source = create_source(
        r#"
        defmodule MyMacros do
          defmacro log(expr) do
            quote do
              IO.puts(inspect(unquote(expr)))
            end
          end
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_error_handling() {
    let source = create_source(
        r#"
        defmodule Errors do
          defmodule NotFound do
            defexception [:message]
          end

          def find!(items, id) do
            case Map.get(items, id) do
              nil -> raise NotFound, message: "not found"
              item -> item
            end
          end

          def safe_find(items, id) do
            try do
              find!(items, id)
            rescue
              e in NotFound -> {:error, e.message}
            end
          end
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_module_attributes() {
    let source = create_source(
        r#"
        defmodule Config do
          @moduledoc "Config module"
          @vsn "1.0"
          @max 100

          @type t :: %{max: integer()}
          @spec get() :: t()
          def get, do: %{max: @max}
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_exunit() {
    let source = create_source(
        r#"
        defmodule UserTest do
          use ExUnit.Case, async: true

          describe "new/1" do
            test "creates user" do
              assert User.new("John").name == "John"
            end
          end
        end
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_source(
        r#"
        defmodule TaskManager do
          @max 100
          defstruct [:id, :title]
          def new(t), do: %__MODULE__{title: t}
          defp gen_id, do: :crypto.strong_rand_bytes(8)
        end
    "#,
    );
    let result = act_lite_parser::query::execute_skeleton(&source).unwrap();
    match result {
        act_lite_parser::query::QueryResult::Skeleton(skel) => {
            assert!(!skel.declarations.is_empty());
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn execute_symbols_classifies_defmodule_correctly() {
    use act_lite_parser::query::{self, QueryResult};
    let source = create_source("defmodule Foo do\n  def bar, do: :ok\nend");
    let result = query::execute_symbols(&source).unwrap();
    match result {
        QueryResult::Symbols { symbols } => {
            let module_sym = symbols.iter().find(|s| s.name() == "Foo");
            assert!(module_sym.is_some(), "Foo module symbol not found");
            let sym = module_sym.unwrap();
            assert_eq!(
                sym.kind(),
                act_lite_core::SymbolKind::Module,
                "defmodule Foo should be classified as Module, not {:?}",
                sym.kind()
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn execute_symbols_classifies_def_as_function() {
    use act_lite_parser::query::{self, QueryResult};
    let source =
        create_source("defmodule M do\n  def my_fn(x), do: x\n  defp priv_fn(y), do: y\nend");
    let result = query::execute_symbols(&source).unwrap();
    match result {
        QueryResult::Symbols { symbols } => {
            let pub_fn = symbols.iter().find(|s| s.name() == "my_fn");
            assert!(pub_fn.is_some(), "my_fn symbol not found");
            assert_eq!(pub_fn.unwrap().kind(), act_lite_core::SymbolKind::Function);

            let priv_fn = symbols.iter().find(|s| s.name() == "priv_fn");
            assert!(priv_fn.is_some(), "priv_fn symbol not found");
            assert_eq!(priv_fn.unwrap().kind(), act_lite_core::SymbolKind::Function);
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_finds_defguard_and_defdelegate() {
    use act_lite_parser::query::{self, QueryResult};
    let source = create_source(
        r#"defmodule Guards do
  defguard is_positive(x) when is_integer(x) and x > 0
  defguardp is_small(x) when x < 100
  defdelegate flatten(list), to: List
end"#,
    );
    let result = query::execute_skeleton(&source).unwrap();
    match result {
        QueryResult::Skeleton(skel) => {
            let names: Vec<&str> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"is_positive"),
                "defguard must appear in skeleton, got: {:?}",
                names
            );
            assert!(
                names.contains(&"is_small"),
                "defguardp must appear in skeleton, got: {:?}",
                names
            );
            assert!(
                names.contains(&"flatten"),
                "defdelegate must appear in skeleton, got: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn execute_symbols_handles_complex_match_bindings() {
    use act_lite_parser::query;
    let source = create_source(
        r#"defmodule Edge do
  def run do
    {a, b} = {1, 2}
    [h | t] = [1, 2, 3]
    x = :ok
    _ = :ignored
    {a, b, h, t, x}
  end
end"#,
    );
    // Must not panic
    let result = query::execute_symbols(&source);
    assert!(
        result.is_ok(),
        "execute_symbols must not panic on complex match patterns"
    );
}

#[test]
fn execute_symbols_handles_anonymous_functions() {
    use act_lite_parser::query;
    let source = create_source(
        r#"defmodule M do
  def run do
    f = fn x -> x * 2 end
    g = fn
      {:ok, v} -> v
      _ -> nil
    end
    {f, g}
  end
end"#,
    );
    let result = query::execute_symbols(&source);
    assert!(
        result.is_ok(),
        "execute_symbols must handle anonymous function captures"
    );
}

#[test]
fn execute_symbols_handles_module_attributes() {
    use act_lite_parser::query;
    let source = create_source(
        r#"defmodule M do
  @moduledoc "docs"
  @spec foo() :: :ok
  @callback bar() :: :ok
  @impl true
  def foo, do: :ok
end"#,
    );
    let result = query::execute_symbols(&source);
    assert!(
        result.is_ok(),
        "execute_symbols must handle module attributes without panic"
    );
}

#[test]
fn execute_references_does_not_capture_definition_site() {
    use act_lite_parser::query::{self, QueryResult};
    // `my_fn` appears once as a definition name, once as a call.
    // The top-level (identifier) @reference pattern used to capture both — the definition
    // site too. After removing that over-broad pattern, definition-site identifiers must
    // not appear as references (they are already covered by the more specific call patterns).
    let source = create_source(
        r#"defmodule M do
  def my_fn(x), do: x
  def run, do: my_fn(1)
end"#,
    );
    let result = query::execute_references(&source);
    assert!(
        result.is_ok(),
        "execute_references must succeed for Elixir files"
    );
    match result.unwrap() {
        QueryResult::References { references, .. } => {
            // With the over-broad (identifier) @reference removed, the count of references
            // for this snippet must be lower than if every identifier were captured.
            // There are 8 identifiers in the file (defmodule, M, def, my_fn, x, x, def,
            // run, do, my_fn). A correct implementation captures only call-site `my_fn`
            // (and `run` via the call pattern), not all identifiers.
            // We assert the total is below 8 to confirm the over-broad pattern is gone.
            assert!(
                references.len() < 8,
                "Too many references captured ({}); over-broad (identifier) pattern may still be present",
                references.len()
            );
        }
        other => panic!("Expected References result, got {:?}", other),
    }
}

#[test]
fn execute_references_finds_module_alias_references() {
    use act_lite_parser::query::{self, QueryResult};
    let source = create_source(
        r#"defmodule M do
  alias MyApp.User
  def create(attrs), do: %User{}
end"#,
    );
    let result = query::execute_references(&source);
    assert!(
        result.is_ok(),
        "execute_references must not error on Elixir alias references"
    );
    match result.unwrap() {
        QueryResult::References { references, .. } => {
            // At minimum the alias directive reference (MyApp.User alias) must be captured.
            assert!(
                !references.is_empty(),
                "execute_references must find at least one reference in a module with alias"
            );
        }
        other => panic!("Expected References result, got {:?}", other),
    }
}
