//! act-lite ships exactly thirty grammars; every other act grammar is gone.

use act_lite_parser::Grammar;
use std::collections::BTreeSet;

#[test]
fn grammar_set_is_exactly_the_lite_thirty() {
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
        "YAML",
        "TOML",
        "HTML",
        "CSS",
        "SCSS",
        "Markdown",
        "Dockerfile",
        "HCL/Terraform",
    ]
    .into_iter()
    .collect();
    assert_eq!(actual, expected);
}

#[test]
fn dropped_extensions_are_unsupported() {
    for ext in [
        "cob", "vue", "hs", "sol", "f90", "cu", "zig", "svelte", "graphql",
    ] {
        assert!(
            Grammar::from_extension(ext).is_none(),
            "extension `{ext}` belongs to a dropped grammar but still routes to {:?}",
            Grammar::from_extension(ext)
        );
    }
}
