//! The README's language documentation: the list equals `status`, the
//! capability table is true row by row, each limitation and rule is stated,
//! and the examples for the added languages run on the built binary. Every
//! expected value is written here from the language rules, not read back from
//! the README.

mod langs_added;

use langs_added::{Mcp, cli, fixture};
use serde_json::{Value, json};
use std::path::Path;

fn read(rel: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel),
    )
    .unwrap()
}

/// The README with each run of whitespace collapsed to one space, so a
/// statement matches wherever the README wraps its lines.
fn readme() -> String {
    flat(&read("README.md"))
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The README's blank-line separated blocks, whitespace collapsed.
fn blocks() -> Vec<String> {
    read("README.md").split("\n\n").map(flat).collect()
}

fn must_state(statements: &[&str]) {
    let readme = readme();
    for statement in statements {
        assert!(readme.contains(statement), "README lacks: {statement}");
    }
}

const ADDED: [&str; 10] = [
    "JSONC",
    "XML",
    "GraphQL",
    "Protocol Buffers",
    "Make",
    "CMake",
    "Zig",
    "R",
    "Julia",
    "Nix",
];

/// The 23 languages the three analyzers support.
const ANALYZED: [&str; 23] = [
    "Bash",
    "C",
    "C#",
    "C++",
    "Dart",
    "Elixir",
    "Go",
    "Java",
    "JavaScript",
    "JSX",
    "Julia",
    "Kotlin",
    "Lua",
    "PHP",
    "Python",
    "R",
    "Ruby",
    "Rust",
    "Scala",
    "Swift",
    "TSX",
    "TypeScript",
    "Zig",
];

/// The 19 languages with a configured language server.
const SERVED: [&str; 19] = [
    "C",
    "C#",
    "C++",
    "Dart",
    "Elixir",
    "Go",
    "Java",
    "JavaScript",
    "JSX",
    "Kotlin",
    "PHP",
    "Python",
    "Ruby",
    "Rust",
    "Scala",
    "SQL",
    "Swift",
    "TSX",
    "TypeScript",
];

/// The language names `status` reports, from a running server.
fn status_languages() -> Vec<String> {
    let ws = tempfile::tempdir().unwrap();
    let mut mcp = Mcp::start(ws.path());
    let result = mcp.call("status", json!({})).unwrap();
    let status: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    let mut names: Vec<String> = status["languages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

/// The rows of the README table whose heading line starts with `header`, each
/// as its trimmed cells.
fn table(header: &str) -> Vec<Vec<String>> {
    let readme = read("README.md");
    let mut lines = readme.lines().skip_while(|l| !l.starts_with(header));
    assert!(lines.next().is_some(), "README has no table `{header}`");
    lines
        .skip(1)
        .take_while(|l| l.starts_with('|'))
        .skip(1)
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect()
        })
        .collect()
}

#[test]
fn the_language_list_is_the_status_list() {
    let blocks = blocks();
    let at = blocks
        .iter()
        .position(|b| b == "act-lite supports 40 languages:")
        .expect("README says `act-lite supports 40 languages:`");
    let mut listed: Vec<String> = blocks[at + 1]
        .trim_end_matches('.')
        .split(", ")
        .map(str::to_string)
        .collect();
    listed.sort();
    assert_eq!(listed, status_languages());
}

#[test]
fn the_capability_table_has_one_true_row_per_language() {
    let rows = table("| Language | `skeleton`, `symbols` |");
    let mut names: Vec<String> = rows.iter().map(|r| r[0].clone()).collect();
    names.sort();
    assert_eq!(names, status_languages());
    for row in &rows {
        let name = row[0].as_str();
        assert_eq!(row.len(), 7, "{name}: {row:?}");
        assert_eq!(row[1], "yes", "{name} skeleton and symbols");
        assert!(row[2].starts_with("tree-sitter"), "{name} definition");
        let served = SERVED.contains(&name);
        assert_eq!(
            row[3],
            if served {
                "language server"
            } else {
                "name-level"
            },
            "{name} references"
        );
        assert_eq!(
            row[4],
            if served {
                "language server"
            } else {
                "not available"
            },
            "{name} callers"
        );
        let analyzed = if ANALYZED.contains(&name) {
            "yes"
        } else {
            "not applicable"
        };
        assert_eq!(&row[5..], [analyzed; 2], "{name} analyzers");
    }
    let note = |name: &str| rows.iter().find(|r| r[0] == name).unwrap()[2].clone();
    assert!(note("JSONC").contains("keys without quotes"));
    assert!(note("JSON").contains("keys without quotes"));
    assert!(note("R").contains("parameters"));
    assert!(note("Nix").contains("parameters") && note("Nix").contains("dotted"));
    assert!(note("Julia").contains("dotted"));
    assert!(note("CMake").contains("case-sensitive"));
}

#[test]
fn file_names_and_extensions_of_the_added_languages() {
    let rows = table("| Language | File names | Extensions |");
    let names: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, ADDED);
    let expected: [(&str, &[&str], &[&str]); 10] = [
        (
            "JSONC",
            &[
                "tsconfig.json",
                "jsconfig.json",
                "tsconfig.*.json",
                "jsconfig.*.json",
                "devcontainer.json",
                ".devcontainer.json",
                ".eslintrc.json",
            ],
            &[".jsonc", ".code-workspace"],
        ),
        (
            "XML",
            &[],
            &[
                ".xml", ".xsd", ".xsl", ".xslt", ".svg", ".plist", ".csproj", ".fsproj", ".vbproj",
                ".props", ".targets", ".resx", ".xaml",
            ],
        ),
        ("GraphQL", &[], &[".graphql", ".graphqls", ".gql"]),
        ("Protocol Buffers", &[], &[".proto"]),
        (
            "Make",
            &["Makefile", "makefile", "GNUmakefile"],
            &[".mk", ".mak"],
        ),
        ("CMake", &["CMakeLists.txt"], &[".cmake"]),
        ("Zig", &[], &[".zig"]),
        ("R", &[".Rprofile"], &[".R", ".r"]),
        ("Julia", &[], &[".jl"]),
        ("Nix", &[], &[".nix"]),
    ];
    for (row, (name, files, extensions)) in rows.iter().zip(expected) {
        for item in files {
            assert!(row[1].contains(&format!("`{item}`")), "{name}: {item}");
        }
        for item in extensions {
            assert!(row[2].contains(&format!("`{item}`")), "{name}: {item}");
        }
    }
}

#[test]
fn each_addition_has_one_limitation_paragraph() {
    let blocks = blocks();
    let paragraph = |name: &str| -> String {
        let lead = format!("**{name}.**");
        let found: Vec<&String> = blocks.iter().filter(|b| b.starts_with(&lead)).collect();
        assert_eq!(found.len(), 1, "one paragraph starts with {lead}");
        found[0].clone()
    };
    let expected: [(&str, &[&str]); 10] = [
        (
            "JSONC",
            &[
                "`//` and `/* */` comments are comments",
                "A trailing comma before `}` or `]`",
                "`\"parse_error\": true`",
                "every key is still reported",
            ],
        ),
        (
            "XML",
            &[
                "no build-system semantics",
                "`${…}` properties are not expanded",
                "parent POMs, imports and includes are not followed",
                "binary property list (`bplist00`)",
                "not-UTF-8 error",
            ],
        ),
        (
            "GraphQL",
            &[
                "`extend schema @link(url: \"x\")`",
                "Apollo Federation header",
                "swallows the first definition after it",
                "`parse_error: true`",
                "anonymous operations",
            ],
        ),
        (
            "Protocol Buffers",
            &[
                "`syntax`, `option`, `reserved`",
                "`extend` blocks",
                "one identifier at a time",
            ],
        ),
        (
            "Make",
            &[
                "Recipes are not run and not parsed as shell",
                "variables are not expanded",
                "conditionals are not evaluated",
                "once per branch",
                "included files are not read",
                "`$(eval …)`",
            ],
        ),
        (
            "CMake",
            &[
                "Configuration is not run",
                "generator expressions and `if()` conditions are not evaluated",
                "`include()`d files and subdirectories are not read",
                "`ADD_TOOL(cli)` is not a reference to `function(add_tool …)`",
            ],
        ),
        (
            "Zig",
            &[
                "`struct {}`, `enum {}`, `union {}` or `opaque {}`",
                "`parse_error: true`",
                "`parse_failures`",
                "`comptime` is not evaluated",
                "`usingnamespace`",
            ],
        ),
        (
            "R",
            &[
                "`function(…) -> name`",
                "`assign()`",
                "`setClass`, `setRefClass` and `R6Class`",
            ],
        ),
        (
            "Julia",
            &[
                "multiple dispatch is not resolved",
                "`export` statements are not reported",
                "`@eval`",
                "`do` blocks",
            ],
        ),
        (
            "Nix",
            &[
                "structural, not evaluated",
                "`//`, `rec`, `with`, overlays, `lib.mkIf`",
                "a binding under `mkIf false { … }` is reported",
                "interpolated attribute name",
            ],
        ),
    ];
    for (name, statements) in expected {
        let text = paragraph(name);
        for statement in statements {
            assert!(
                text.contains(statement),
                "{name} paragraph lacks: {statement}"
            );
        }
    }
}

#[test]
fn reference_walks_and_evidence_are_stated() {
    must_state(&[
        "The `references` walk skips every path component that starts with `.`, so `.eslintrc.json`, `.devcontainer.json`, `.devcontainer/…` and `.Rprofile` are never listed from another file; `definition` and `repo-outline` see them.",
        "The `references` and `definition` walks read the files of one language, except JSON and JSONC: from a `.jsonc` or `.code-workspace` file they read JSON and JSONC files; from a JSON file, or from a JSONC file recognized by its name (such as `tsconfig.json`), they read JSON files and the JSONC files recognized by name only.",
        "For the ten added languages every `references` answer is a name match (`\"source\": \"tree-sitter\"`), and none of them has a language server: `callers`, `callees` and `implementations` fail for them, and `status` lists no server for them.",
        "MCP `skeleton` omits `import` entries for every language; `act-lite query skeleton` and `symbols-batch` list them.",
        "`repo-outline` counts every recognized file against `--max-files`, so the files of the added languages (`.svg`, `.xml`, `Makefile`, and others) use part of the budget: a truncated outline of a repository that has them lists fewer of its other files.",
        "`status` lists the 40 languages",
    ]);
    assert!(
        !readme().contains("every same-language file"),
        "the walk sentence must name the JSON and JSONC exception"
    );
}

#[test]
fn test_files_and_embedded_tests_of_the_added_languages_are_stated() {
    must_state(&[
        "| Rust, CSS, SCSS, HTML, JSON, YAML, TOML, Markdown, SQL, Dockerfile, JSONC, XML, GraphQL, Protocol Buffers, Make, CMake, Zig, R, Julia, Nix | the directory rule only |",
        "R `testthat` reads test files only from `tests/testthat/`, which the directory rule covers; `R/test_x.R` is package source.",
        "| Zig | A `test` declaration (`test \"name\" { … }`, `test name { … }`, `test { … }`) |",
        "| Julia | `@test` and `@testset` in source files | Not detected. Not removed. |",
        "Files of the added languages are recognized files: they appear in `excluded.*` and in its counts, and `coverage.unrecognized_files` falls. Findings of the other languages do not change.",
    ]);
}

#[test]
fn the_analyzer_statements_follow_zig_r_and_julia() {
    must_state(&[
        "The three analyzers support 23 programming languages: Bash, C, C++, C#, Dart, Elixir, Go, Java, JavaScript, JSX, Julia, Kotlin, Lua, PHP, Python, R, Ruby, Rust, Scala, Swift, TSX, TypeScript and Zig.",
    ]);
    must_state(&[
        "They do not support the 17 data, schema and markup formats: CMake, CSS, Dockerfile, GraphQL, HCL/Terraform, HTML, JSON, JSONC, Make, Markdown, Nix, Protocol Buffers, SCSS, SQL, TOML, XML and YAML.",
        "Zig functions are `fn` declarations with a body; a `test` block is a function only with `--include-tests`.",
        "R functions are `function(…)` and `\\(…)` definitions, named by the `<-`, `=` or `<<-` assignment whose right side they are, else `null`.",
        "Julia functions are long-form and short-form definitions (`f(x) = …`, also with a `::T` return type or a `where` clause), macros, anonymous `function (x) … end` and `x -> …` functions, and `do` blocks; `function f end` is not a function.",
        "Zig `try` and `orelse`, R `ifelse()`, `switch()` and `tryCatch()` calls, and Julia comprehension `for` and `if` clauses are not decision points.",
        "Zig `string`, `multiline_string`, `character`, `integer`, `float`",
        "R `string`, `integer`, `float`, `complex`",
        "Julia `string_literal`, `prefixed_string_literal`, `command_literal`, `prefixed_command_literal`, `character_literal`, `integer_literal`, `float_literal`",
    ]);
}

#[test]
fn the_examples_for_the_added_languages_run() {
    let ws = tempfile::tempdir().unwrap();
    for rel in [
        "make/Makefile",
        "graphql/schema.graphql",
        "protobuf/shop.proto",
    ] {
        let source = fixture(rel);
        std::fs::copy(&source, ws.path().join(source.file_name().unwrap())).unwrap();
    }
    let examples: Vec<String> = read("README.md")
        .lines()
        .filter(|l| l.starts_with("act-lite query "))
        .filter(|l| {
            ["Makefile", "schema.graphql", "shop.proto"]
                .iter()
                .any(|f| l.contains(f))
        })
        .map(|l| l.split('#').next().unwrap().trim().to_string())
        .collect();
    assert_eq!(examples.len(), 3, "{examples:?}");
    for example in &examples {
        let args: Vec<&str> = example.split_whitespace().skip(1).collect();
        let output = cli(ws.path(), &args);
        assert!(
            output.status.success(),
            "{example}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        if args[1] == "references" {
            assert_eq!(result["source"], "tree-sitter", "{example}: {result}");
            assert!(
                result["references"].as_array().unwrap().len() >= 2,
                "{result}"
            );
        } else {
            assert!(!result.to_string().is_empty());
        }
    }
}

#[test]
fn the_plugin_and_marketplace_descriptions_say_forty_languages() {
    for rel in [
        "plugin/.claude-plugin/plugin.json",
        ".claude-plugin/marketplace.json",
    ] {
        let text = read(rel);
        assert!(text.contains("across 40 languages"), "{rel}");
        assert!(!text.contains("30 languages"), "{rel}");
    }
}
