use act_lite_parser::Grammar;
use std::collections::BTreeSet;

/// The grammars without a `locals.scm` binding model, by `Grammar` variant
/// name. Shrink-only: add a `locals.scm` and delete the entry here in the
/// same commit.
const KNOWN_UNMODELED: &[&str] = &[
    "Astro",
    "Bash",
    "CMake",
    "Cpp",
    "CSharp",
    "Css",
    "Dart",
    "Dockerfile",
    "Elixir",
    "GraphQl",
    "HclTerraform",
    "Html",
    "Java",
    "Jsx",
    "Json",
    "Jsonc",
    "Julia",
    "Kotlin",
    "Lua",
    "Make",
    "Markdown",
    "Nix",
    "Php",
    "Protobuf",
    "Python",
    "R",
    "Ruby",
    "Scala",
    "Sql",
    "Scss",
    "Svelte",
    "Swift",
    "Toml",
    "Tsx",
    "Vue",
    "Xml",
    "Yaml",
    "Zig",
];

/// The measured unmodeled set equals the pin in both directions.
#[test]
fn locals_model_coverage_matches_the_pin() {
    let measured: BTreeSet<String> = Grammar::all()
        .into_iter()
        .filter(|grammar| grammar.language_support().locals_query().trim().is_empty())
        .map(|grammar| format!("{grammar:?}"))
        .collect();
    let pinned: BTreeSet<String> = KNOWN_UNMODELED.iter().map(|s| s.to_string()).collect();

    let new_unmodeled: Vec<_> = measured.difference(&pinned).collect();
    let fixed_but_pinned: Vec<_> = pinned.difference(&measured).collect();

    assert!(
        new_unmodeled.is_empty() && fixed_but_pinned.is_empty(),
        "grammars without a locals.scm binding model do not match the pin \
         ({} new unmodeled, {} pinned-but-modeled):\n{}\n{}\n\
         A grammar landing unmodeled keeps name-matched rename/inline/delete sites behind a \
         disclosure — file it on the `scope-blind-rename` row and add its locals.scm, or pin it \
        here knowingly; a grammar that GAINED a model must be removed from KNOWN_UNMODELED in \
         the commit that added it.",
        new_unmodeled.len(),
        fixed_but_pinned.len(),
        new_unmodeled
            .iter()
            .map(|g| format!("  NEW    \"{g}\","))
            .collect::<Vec<_>>()
            .join("\n"),
        fixed_but_pinned
            .iter()
            .map(|g| format!("  MODELED \"{g}\","))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
