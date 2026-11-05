//! The README's language documentation: the list equals `status`, the
//! capability table is true row by row, each limitation and rule is stated,
//! and the examples for the added languages run on the built binary. Every
//! expected value is written here from the language rules, not read back from
//! the README.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

fn read(rel: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel),
    )
    .unwrap()
}

/// The README with `\r\n` line ends read as `\n`, so a Windows checkout reads
/// the same text.
fn readme_source() -> String {
    read("README.md").replace("\r\n", "\n")
}

/// The README with each run of whitespace collapsed to one space, so a
/// statement matches wherever the README wraps its lines.
fn readme() -> String {
    flat(&readme_source())
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The README's blocks, separated by one or more blank lines, whitespace
/// collapsed.
fn blocks() -> Vec<String> {
    let mut blocks = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    let source = readme_source();
    for line in source.lines().chain([""]) {
        if line.trim().is_empty() {
            if !block.is_empty() {
                blocks.push(flat(&block.join("\n")));
                block.clear();
            }
        } else {
            block.push(line);
        }
    }
    blocks
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
    let readme = readme_source();
    let mut lines = readme.lines().skip_while(|l| !l.starts_with(header));
    assert!(lines.next().is_some(), "README has no table `{header}`");
    lines
        .skip(1)
        .take_while(|l| l.starts_with('|'))
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
        assert_eq!(row.len(), 8, "{name}: {row:?}");
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
        assert_eq!(&row[5..], [analyzed; 3], "{name} analyzers");
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
                "binding under `mkIf false { … }` is reported",
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
    let examples: Vec<(String, String)> = readme_source()
        .lines()
        .filter(|l| l.starts_with("act-lite query "))
        .filter(|l| {
            ["Makefile", "schema.graphql", "shop.proto"]
                .iter()
                .any(|f| l.contains(f))
        })
        .map(|l| {
            let (command, comment) = l.split_once('#').expect("each example has a comment");
            (command.trim().to_string(), comment.trim().to_string())
        })
        .collect();
    assert_eq!(examples.len(), 3, "{examples:?}");
    for (example, comment) in &examples {
        let args: Vec<&str> = example.split_whitespace().skip(1).collect();
        let result = cli_json(ws.path(), &args);
        match args[1] {
            "skeleton" => {
                assert_eq!(comment, "targets, variables, includes", "{example}");
                let found = kind_names(&result, "declarations");
                let kinds: BTreeSet<&str> = found.iter().map(|(k, _)| k.as_str()).collect();
                assert_eq!(
                    kinds,
                    BTreeSet::from(["function", "variable", "include"]),
                    "{result}"
                );
                for entry in [
                    ("function", "all"),
                    ("variable", "CC"),
                    ("include", "common.mk"),
                ] {
                    assert!(found.contains(&pair(entry)), "{entry:?}: {result}");
                }
            }
            "symbols" => {
                assert_eq!(comment, "types, fields, enum values", "{example}");
                let found = kind_names(&result, "symbols");
                for entry in [("class", "User"), ("field", "id"), ("enum_member", "ADMIN")] {
                    assert!(found.contains(&pair(entry)), "{entry:?}: {result}");
                }
            }
            "references" => {
                assert_eq!(comment, "name matches", "{example}");
                assert_eq!(result["source"], "tree-sitter", "{example}: {result}");
                let sites: Vec<(String, u64, u64)> = result["references"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| {
                        let file = Path::new(r["file"].as_str().unwrap());
                        (
                            file.file_name().unwrap().to_string_lossy().to_string(),
                            r["line"].as_u64().unwrap(),
                            r["column"].as_u64().unwrap(),
                        )
                    })
                    .collect();
                let at = |line, column| ("shop.proto".to_string(), line, column);
                assert_eq!(
                    sites,
                    [at(5, 9), at(15, 11), at(15, 27), at(16, 20), at(16, 43)],
                    "{result}"
                );
            }
            other => panic!("unexpected example `{other}`: {example}"),
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

#[test]
fn hidden_paths_are_seen_by_definition_and_outline_not_by_references() {
    let ws = tempfile::tempdir().unwrap();
    for (rel, content) in [
        ("package.json", "{\"name\": 1}\n"),
        (".eslintrc.json", "{\"name\": 2}\n"),
        (".devcontainer/devcontainer.json", "{\"a\": 1}\n"),
        (".Rprofile", "x <- 1\nonly_here <- 2\n"),
        ("use.R", "only_here + 1\n"),
    ] {
        let path = ws.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let output = cli(
        ws.path(),
        &[
            "query",
            "references",
            "name",
            "--file",
            "package.json",
            "--line",
            "1",
            "--column",
            "3",
        ],
    );
    let references: Value = serde_json::from_slice(&output.stdout).unwrap();
    let files: Vec<&str> = references["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["file"].as_str().unwrap())
        .collect();
    assert_eq!(files.len(), 1, "{references}");
    assert!(files[0].ends_with("package.json"), "{references}");
    let output = cli(
        ws.path(),
        &[
            "query",
            "definition",
            "only_here",
            "--file",
            "use.R",
            "--line",
            "1",
            "--column",
            "1",
        ],
    );
    let definition: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        definition["definition"]["file"]
            .as_str()
            .unwrap()
            .ends_with(".Rprofile"),
        "{definition}"
    );
    let outline = cli_json(ws.path(), &["query", "repo-outline"]);
    let paths: Vec<&str> = outline["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    for hidden in [
        ".eslintrc.json",
        ".devcontainer/devcontainer.json",
        ".Rprofile",
    ] {
        assert!(paths.contains(&hidden), "{hidden} missing: {paths:?}");
    }
}

#[test]
fn imports_are_in_query_skeleton_and_not_in_mcp_skeleton() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("zig/point.zig"), ws.path().join("point.zig")).unwrap();
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "point.zig"]);
    let kinds = |value: &Value| -> Vec<String> {
        value["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["kind"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(
        kinds(&skeleton).contains(&"import".to_string()),
        "{skeleton}"
    );
    let mut mcp = Mcp::start(ws.path());
    let result = mcp.call("skeleton", json!({"file": "point.zig"})).unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Point"), "{text}");
    assert!(!text.to_lowercase().contains("import"), "{text}");
}

/// The `(kind, name)` pairs of the entries under `key` of a `query` result.
fn kind_names(result: &Value, key: &str) -> BTreeSet<(String, String)> {
    result[key]
        .as_array()
        .unwrap_or_else(|| panic!("no `{key}` in {result}"))
        .iter()
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_string(),
                e["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pair((kind, name): (&str, &str)) -> (String, String) {
    (kind.to_string(), name.to_string())
}

fn pairs(entries: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    entries.iter().copied().map(pair).collect()
}

/// A temporary workspace holding each `(path, content)` of `files`.
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

/// One small file for each language `status` lists: `(language, path, content)`.
const ONE_FILE_EACH: [(&str, &str, &str); 40] = [
    ("Bash", "a.sh", "f() { :; }\n"),
    ("C", "a.c", "int f(void) { return 0; }\n"),
    ("C#", "a.cs", "class A { void F() {} }\n"),
    ("C++", "a.cpp", "int f() { return 0; }\n"),
    ("CMake", "CMakeLists.txt", "set(A 1)\n"),
    ("CSS", "a.css", "a { color: red; }\n"),
    ("Dart", "a.dart", "void f() {}\n"),
    ("Dockerfile", "Dockerfile", "FROM x\n"),
    ("Elixir", "a.ex", "defmodule A do\nend\n"),
    ("Go", "a.go", "package a\n"),
    ("GraphQL", "a.graphql", "type A { a: Int }\n"),
    ("HCL/Terraform", "a.tf", "a = 1\n"),
    ("HTML", "a.html", "<p></p>\n"),
    ("Java", "A.java", "class A {}\n"),
    ("JavaScript", "a.js", "function f() {}\n"),
    ("JSX", "a.jsx", "function f() {}\n"),
    ("JSON", "a.json", "{\"a\": 1}\n"),
    ("JSONC", "a.jsonc", "{\"a\": 1}\n"),
    ("Julia", "a.jl", "f(x) = x\n"),
    ("Kotlin", "a.kt", "fun f() {}\n"),
    ("Lua", "a.lua", "local a = 1\n"),
    ("Make", "Makefile", "a = 1\n"),
    ("Markdown", "a.md", "# a\n"),
    ("Nix", "a.nix", "{ a = 1; }\n"),
    ("PHP", "a.php", "<?php function f() {}\n"),
    ("Protocol Buffers", "a.proto", "syntax = \"proto3\";\n"),
    ("Python", "a.py", "def f(): pass\n"),
    ("R", "a.R", "f <- function() 1\n"),
    ("Ruby", "a.rb", "def f; end\n"),
    ("Rust", "a.rs", "fn f() {}\n"),
    ("Scala", "a.scala", "object A\n"),
    ("SCSS", "a.scss", "a { b: c; }\n"),
    ("SQL", "a.sql", "SELECT 1;\n"),
    ("Swift", "a.swift", "func f() {}\n"),
    ("TOML", "a.toml", "a = 1\n"),
    ("TSX", "a.tsx", "function f() {}\n"),
    ("TypeScript", "a.ts", "function f() {}\n"),
    ("XML", "a.xml", "<a/>\n"),
    ("YAML", "a.yaml", "a: 1\n"),
    ("Zig", "a.zig", "fn f() void {}\n"),
];

/// The count and the names of the README list that follows `lead` and ends
/// at `end`: `lead` is followed by the count, then `noun`, then the names.
fn analyzer_list(readme: &str, lead: &str, noun: &str, end: &str) -> (usize, BTreeSet<String>) {
    let rest = &readme[readme
        .find(lead)
        .unwrap_or_else(|| panic!("README lacks: {lead}"))
        + lead.len()..];
    let (count, rest) = rest
        .split_once(noun)
        .unwrap_or_else(|| panic!("README lacks: {noun}"));
    let (list, _) = rest
        .split_once(end)
        .unwrap_or_else(|| panic!("README lacks: {end}"));
    let names = list
        .replace(" and ", ", ")
        .split(", ")
        .map(str::to_string)
        .collect();
    (count.parse().unwrap(), names)
}

#[test]
fn the_analyzer_language_lists_are_what_the_analyzers_report() {
    let files: Vec<(&str, &str)> = ONE_FILE_EACH.iter().map(|(_, p, c)| (*p, *c)).collect();
    let ws = workspace(&files);
    let mut languages: Vec<String> = ONE_FILE_EACH
        .iter()
        .map(|(l, _, _)| l.to_string())
        .collect();
    languages.sort();
    assert_eq!(
        languages,
        status_languages(),
        "one file for each language of `status`"
    );

    let patterns = cli_json(ws.path(), &["analyze", "patterns"]);
    let coverage = patterns["coverage"]["languages"].as_array().unwrap();
    assert_eq!(coverage.len(), 40, "{patterns}");
    let reported = |supported: bool| -> BTreeSet<String> {
        coverage
            .iter()
            .filter(|l| l["supported"] == supported)
            .inspect(|l| assert_eq!(l["files"], 1, "{l}"))
            .map(|l| l["language"].as_str().unwrap().to_string())
            .collect()
    };
    let (supported, unsupported) = (reported(true), reported(false));
    assert_eq!((supported.len(), unsupported.len()), (23, 17), "{patterns}");

    let readme = readme();
    let (count, listed) = analyzer_list(
        &readme,
        "The three analyzers support ",
        " programming languages: ",
        ". They do not support",
    );
    assert_eq!((count, listed.len()), (23, 23), "{listed:?}");
    assert_eq!(listed, supported);
    let (count, listed) = analyzer_list(
        &readme,
        "They do not support the ",
        " data, schema and markup formats: ",
        ". Those files",
    );
    assert_eq!((count, listed.len()), (17, 17), "{listed:?}");
    assert_eq!(listed, unsupported);
    assert_eq!(supported, ANALYZED.iter().map(|l| l.to_string()).collect());

    let mut mcp = Mcp::start(ws.path());
    let result = mcp.call("analyze_hotspots", json!({})).unwrap();
    let hotspots: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(hotspots["coverage"], patterns["coverage"]);
}

#[test]
fn the_design_statements_of_the_added_languages_are_stated() {
    must_state(&[
        "An `element` in the skeleton is an element, and in the symbols an element name is a `class` and an attribute name a `field`.",
        "Types, interfaces, inputs, enums, unions, scalars, directives, named operations and fragments are declarations; fields and enum values are symbols.",
        "Not reported: `schema { … }`, anonymous operations, field arguments, operation variables and selections.",
        "Messages, enums, services and `rpc` methods are declarations; fields and enum values are symbols; `import` and `package` are skeleton entries.",
        "Targets are functions and variable assignments are variables; GNU special targets (`.PHONY`) and target-specific assignments are not reported.",
        "`include()`d files and subdirectories are not read, and a call such as `add_tool(cli)` is a use, not a target.",
        "The built-in commands are recognized in any letter case, but name matches are case-sensitive: `ADD_TOOL(cli)` is not a reference to `function(add_tool …)`.",
        "navigation results carry `parse_error: true`, and an analyzer lists the file in `parse_failures` and says `complete: false`.",
        "`comptime` is not evaluated: a type built by a call (`const T = List(u8)`) is a `constant`, not a `struct`.",
        "`test name { … }` (a doctest) is a use of `name`, not a declaration.",
        "A function is an assignment whose value is a `function` or `\\(…)` definition.",
        "Not modeled: `function(…) -> name`, assignments inside function bodies other than functions, `assign()`, and classes made by calls (`setClass`, `setRefClass` and `R6Class`).",
        "Each method of a function is a separate entry, and multiple dispatch is not resolved.",
        "`imports = [ ./a.nix ]` is a binding whose list is not followed.",
        "The kind comes from the syntax of the bound expression.",
        "R functions are `function(…)` and `\\(…)` definitions, named by the `<-`, `=` or `<<-` assignment whose right side they are, else `null`.",
        "Julia functions are long-form and short-form definitions (`f(x) = …`, also with a `::T` return type or a `where` clause), macros, anonymous `function (x) … end` and `x -> …` functions, and `do` blocks;",
    ]);
}

#[test]
fn r_and_julia_functions_are_named_as_the_analyzer_paragraph_states() {
    let ws = workspace(&[
        (
            "a.R",
            "g = function(x) x\nh <<- function() 2\nk <- \\(x) x\nlapply(1, function(y) y)\n",
        ),
        ("b.jl", "macro twice(ex) ex end\n"),
    ]);
    let hotspots = cli_json(ws.path(), &["analyze", "hotspots"]);
    let mut functions: Vec<(String, Option<String>)> = hotspots["findings"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| {
            let file = f["file"].as_str().unwrap().to_string();
            f["top_functions"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |t| (file.clone(), t["function"].as_str().map(str::to_string)))
        })
        .collect();
    functions.sort();
    let at = |file: &str, name: Option<&str>| (file.to_string(), name.map(str::to_string));
    assert_eq!(
        functions,
        [
            at("a.R", None),
            at("a.R", Some("g")),
            at("a.R", Some("h")),
            at("a.R", Some("k")),
            at("b.jl", Some("twice")),
        ],
        "{hotspots}"
    );
}

#[test]
fn proto2_groups_xml_prologs_and_zig_test_names_are_stated_and_hold() {
    must_state(&[
        "A proto2 `group` is reported as a `struct` (its message) and a `field` (the group name as written).",
        "Text content, comments, processing instructions, the XML declaration, `DOCTYPE` and internal DTD declarations are not reported.",
        "`test \"name\" { … }` is a `function` in `skeleton` and `symbols`, named by the string content (`test \"point len\"` is named `point len`);",
    ]);
    let ws = workspace(&[
        (
            "g.proto",
            "syntax = \"proto2\";\nmessage G {\n  optional group Result = 1 {}\n}\n",
        ),
        (
            "a.xml",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE r [<!ELEMENT r ANY>]>\n<!-- c -->\n<?pi x?>\n<r a=\"1\">text</r>\n",
        ),
        (
            "t.zig",
            "const name = 1;\ntest name { }\ntest \"point len\" { }\n",
        ),
    ]);
    let symbols = |file: &str| cli_json(ws.path(), &["query", "symbols", file]);
    let skeleton = |file: &str| cli_json(ws.path(), &["query", "skeleton", file]);

    let group = symbols("g.proto");
    assert_eq!(
        kind_names(&group, "symbols"),
        pairs(&[("struct", "G"), ("struct", "Result"), ("field", "Result")]),
        "{group}"
    );

    let xml = symbols("a.xml");
    assert_eq!(xml["parse_error"], Value::Null, "{xml}");
    assert_eq!(
        kind_names(&xml, "symbols"),
        pairs(&[("class", "r"), ("field", "a")]),
        "{xml}"
    );
    let xml = skeleton("a.xml");
    assert_eq!(
        kind_names(&xml, "declarations"),
        pairs(&[("element", "r")]),
        "{xml}"
    );

    let want = pairs(&[("constant", "name"), ("function", "point len")]);
    let zig = symbols("t.zig");
    assert_eq!(kind_names(&zig, "symbols"), want, "{zig}");
    let zig = skeleton("t.zig");
    assert_eq!(kind_names(&zig, "declarations"), want, "{zig}");
}
