//! act-lite ships exactly the grammars listed here; every other act grammar is gone.

use act_lite_parser::Grammar;
use std::collections::BTreeSet;

#[test]
fn grammar_set_is_exactly_the_lite_set() {
    let actual: BTreeSet<&str> = Grammar::all().iter().map(|g| g.name()).collect();
    let expected: BTreeSet<&str> = [
        "TypeScript",
        "TSX",
        "JavaScript",
        "JSX",
        "Python",
        "Rust",
        "Go",
        "Java",
        "Kotlin",
        "C",
        "C++",
        "C#",
        "Ruby",
        "PHP",
        "Swift",
        "Scala",
        "Dart",
        "Lua",
        "Elixir",
        "Bash",
        "SQL",
        "JSON",
        "JSONC",
        "YAML",
        "TOML",
        "HTML",
        "CSS",
        "SCSS",
        "Markdown",
        "Dockerfile",
        "HCL/Terraform",
        "XML",
        "GraphQL",
        "Protocol Buffers",
        "Make",
        "CMake",
        "Nix",
        "Zig",
        "R",
        "Julia",
    ]
    .into_iter()
    .collect();
    assert_eq!(actual, expected);
}

#[test]
fn dropped_extensions_are_unsupported() {
    for ext in ["cob", "vue", "hs", "sol", "f90", "cu", "svelte"] {
        assert!(
            Grammar::from_extension(ext).is_none(),
            "extension `{ext}` belongs to a dropped grammar but still routes to {:?}",
            Grammar::from_extension(ext)
        );
    }
}
