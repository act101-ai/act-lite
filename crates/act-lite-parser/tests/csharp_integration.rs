//! C# parsing integration tests.

use act_lite_parser::{Grammar, SourceFile, query};

fn create_cs_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.cs", content.to_string(), Grammar::CSharp).unwrap()
}

#[test]
fn parses_csharp_class() {
    let source = create_cs_source(
        r#"
        public class Calculator
        {
            private int _value;

            public Calculator(int initial)
            {
                _value = initial;
            }

            public int Add(int n) => _value += n;
        }
    "#,
    );
    assert_eq!(source.grammar(), Grammar::CSharp);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_record() {
    let source = create_cs_source(
        r#"
        public record Person(string FirstName, string LastName)
        {
            public string FullName => $"{FirstName} {LastName}";
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_record_struct() {
    let source = create_cs_source(r#"public readonly record struct Point(int X, int Y);"#);
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_interface() {
    let source = create_cs_source(
        r#"
        public interface IRepository<T>
        {
            Task<T?> GetByIdAsync(int id);
            Task<IEnumerable<T>> GetAllAsync();
            Task SaveAsync(T item);

            // Default implementation
            Task<bool> ExistsAsync(int id) =>
                GetByIdAsync(id).ContinueWith(t => t.Result != null);
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_struct() {
    let source = create_cs_source(
        r#"
        public readonly struct Coordinate
        {
            public double Latitude { get; init; }
            public double Longitude { get; init; }

            public double DistanceTo(Coordinate other)
            {
                var dx = Latitude - other.Latitude;
                var dy = Longitude - other.Longitude;
                return Math.Sqrt(dx * dx + dy * dy);
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_enum() {
    let source = create_cs_source(
        r#"
        public enum Status
        {
            Unknown = 0,
            Active = 1,
            Inactive = 2,
            [Obsolete("Use Inactive")]
            Deleted = 3
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_async_await() {
    let source = create_cs_source(
        r#"
        public class DataService
        {
            public async Task<string> FetchDataAsync(string url)
            {
                using var client = new HttpClient();
                var response = await client.GetAsync(url);
                return await response.Content.ReadAsStringAsync();
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_linq_query() {
    let source = create_cs_source(
        r#"
        public class QueryExample
        {
            public IEnumerable<string> FilterItems(List<string> items)
            {
                return from item in items
                       where item.Length > 3
                       orderby item
                       select item.ToUpper();
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_linq_method() {
    let source = create_cs_source(
        r#"
        public class MethodSyntax
        {
            public List<string> Process(List<string> items) =>
                items
                    .Where(x => !string.IsNullOrEmpty(x))
                    .Select(x => x.Trim())
                    .OrderBy(x => x)
                    .ToList();
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_pattern_matching() {
    let source = create_cs_source(
        r#"
        public class PatternExample
        {
            public string Describe(object? obj) => obj switch
            {
                null => "null",
                int i when i > 0 => $"positive int: {i}",
                int i => $"non-positive int: {i}",
                string s => $"string: {s}",
                { } x => $"object: {x}",
            };
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_nullable_types() {
    let source = create_cs_source(
        r#"
        #nullable enable

        public class NullableExample
        {
            public string? Name { get; set; }
            public int? Age { get; set; }

            public string GetDisplayName()
            {
                return Name ?? "Unknown";
            }

            public bool TryGetAge(out int age)
            {
                age = Age ?? 0;
                return Age.HasValue;
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_properties() {
    let source = create_cs_source(
        r#"
        public class PropertyExample
        {
            // Auto-property
            public int Id { get; set; }

            // Init-only property
            public string Name { get; init; } = "";

            // Expression-bodied property
            public bool IsValid => Id > 0;

            // Property with backing field
            private string _description = "";
            public string Description
            {
                get => _description;
                set => _description = value ?? "";
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_generics() {
    let source = create_cs_source(
        r#"
        public class Container<T> where T : class, new()
        {
            private T? _value;

            public TResult Transform<TResult>(Func<T?, TResult> transformer)
                where TResult : notnull
            {
                return transformer(_value);
            }

            public static Container<T> Create() => new();
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_attributes() {
    let source = create_cs_source(
        r#"
        [Serializable]
        [Obsolete("Use NewClass instead", error: false)]
        public class AttributeExample
        {
            [Required]
            [StringLength(100, MinimumLength = 1)]
            public string Name { get; set; } = "";

            [HttpGet("api/items/{id}")]
            [Authorize(Roles = "Admin")]
            public async Task<IActionResult> GetItem(int id) => Ok();
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_extension_methods() {
    let source = create_cs_source(
        r#"
        public static class StringExtensions
        {
            public static bool IsNullOrEmpty(this string? s) =>
                string.IsNullOrEmpty(s);

            public static string Truncate(this string s, int maxLength) =>
                s.Length <= maxLength ? s : s[..maxLength] + "...";

            public static T? FirstOrDefault<T>(this IEnumerable<T> source, Func<T, bool> predicate) =>
                source.Where(predicate).FirstOrDefault();
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_file_scoped_namespace() {
    let source = create_cs_source(
        r#"
        namespace MyApp.Services;

        using System;
        using System.Collections.Generic;

        public class MyService
        {
            public void DoWork() { }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_local_functions() {
    let source = create_cs_source(
        r#"
        public class LocalFunctionExample
        {
            public int Process(int[] numbers)
            {
                return Sum(numbers);

                static int Sum(int[] nums)
                {
                    var total = 0;
                    foreach (var n in nums)
                        total += n;
                    return total;
                }
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_tuples() {
    let source = create_cs_source(
        r#"
        public class TupleExample
        {
            public (int count, string first, string last) GetStats(List<string> items)
            {
                return (items.Count, items.FirstOrDefault() ?? "", items.LastOrDefault() ?? "");
            }

            public void Deconstruct()
            {
                var (count, first, last) = GetStats(new List<string>());
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csharp_delegates_and_events() {
    let source = create_cs_source(
        r#"
        public delegate void EventHandler<T>(object sender, T args);

        public class Publisher
        {
            public event EventHandler<string>? OnMessage;

            public void Send(string message)
            {
                OnMessage?.Invoke(this, message);
            }
        }
    "#,
    );
    assert!(!source.tree().root_node().has_error());
}

#[test]
fn parses_csx_script_files() {
    let source = SourceFile::from_content(
        "script.csx",
        r#"
        #r "nuget: Newtonsoft.Json, 13.0.1"

        using Newtonsoft.Json;

        var data = new { Name = "Test", Value = 42 };
        Console.WriteLine(JsonConvert.SerializeObject(data));
    "#
        .to_string(),
        Grammar::CSharp,
    )
    .unwrap();
    assert_eq!(source.grammar(), Grammar::CSharp);
}

#[test]
fn skeleton_query_finds_declarations() {
    let source = create_cs_source(
        r#"
        namespace MyApp;

        using System;

        public class Sample
        {
            public int Value { get; set; }
            public void Process() { }
        }

        public interface IService { }

        public enum Status { Active, Inactive }
    "#,
    );

    let result = query::execute_skeleton(&source).unwrap();
    // Verify skeleton query returns results
    match result {
        query::QueryResult::Skeleton(_) => {}
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_symbols() {
    let source = create_cs_source(
        r#"
        public class Sample
        {
            private readonly string _name;
            public int Value { get; set; }

            public Sample(string name, int value)
            {
                _name = name;
                Value = value;
            }

            public string GetName() => _name;
        }
    "#,
    );

    let result = query::execute_symbols(&source).unwrap();
    // Verify symbols query returns results
    match result {
        query::QueryResult::Symbols { .. } => {}
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_returns_expected_symbol_count_for_class() {
    let source = create_cs_source(
        r#"
namespace MyApp
{
    public class MyService
    {
        private string _name;
        public string Name { get; set; }
        public MyService(string name) { _name = name; }
        public void Process() { }
    }
}
"#,
    );
    let result = query::execute_skeleton(&source).unwrap();
    match result {
        query::QueryResult::Skeleton(skel) => {
            // Expect: 1 namespace, 1 class, 1 field, 1 property, 1 constructor, 1 method = 6 items
            assert!(
                skel.declarations.len() >= 6,
                "expected at least 6 skeleton items, got {}: {:?}",
                skel.declarations.len(),
                skel.declarations
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_returns_expected_definitions() {
    let source = create_cs_source(
        r#"
public class Calculator
{
    public int Add(int a, int b) => a + b;
    public int Subtract(int a, int b) => a - b;
}
"#,
    );
    let result = query::execute_symbols(&source).unwrap();
    match result {
        query::QueryResult::Symbols { symbols } => {
            let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"Calculator"),
                "should contain class name; got: {names:?}"
            );
            assert!(
                names.contains(&"Add"),
                "should contain method Add; got: {names:?}"
            );
            assert!(
                names.contains(&"Subtract"),
                "should contain method Subtract; got: {names:?}"
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn references_query_finds_identifiers_in_method_body() {
    let source = create_cs_source(
        r#"
public class Caller
{
    public void Run()
    {
        Helper();
    }
    private void Helper() { }
}
"#,
    );
    let result = query::execute_references(&source).unwrap();
    match result {
        query::QueryResult::References { references, .. } => {
            assert!(
                !references.is_empty(),
                "expected references in method body, got none"
            );
        }
        _ => panic!("Expected References result"),
    }
}
