use act_lite_parser::Grammar;
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::CaptureQuantifier;

/// The 33 kind captures `execute_skeleton` turns into a `Declaration`
/// (`query/mod.rs`, the `"function" | "class" | …` arm). Copied from
/// `skeleton_name_binding_audit.rs`, which mirrors the same arm.
/// `@definition` and `@definition.<kind>` are handled in
/// `is_declaration_producing` instead — the dotted family takes any suffix and
/// cannot be enumerated here.
const KIND_CAPTURES: &[&str] = &[
    "function",
    "class",
    "interface",
    "trait",
    "type_alias",
    "enum",
    "method",
    "namespace",
    "struct",
    "delegate",
    "field",
    "constructor",
    "event",
    "property",
    "variable",
    "docstring",
    "impl",
    "module",
    "macro",
    "union",
    "typedef",
    "concept",
    "using",
    "static_initializer",
    "record",
    "annotation",
    "enum_constant",
    "package",
    "indexer",
    "constant",
    "element",
    "block",
    "tag",
];

/// Captures that supply the name from their own node text
/// (`query/mod.rs`, the `"import" | "export" | …` arm).
const SELF_NAMING_CAPTURES: &[&str] = &["import", "export", "include", "use", "require"];

fn is_declaration_producing(name: &str) -> bool {
    KIND_CAPTURES.contains(&name)
        || SELF_NAMING_CAPTURES.contains(&name)
        || name == "definition"
        || name.starts_with("definition.")
}

const KNOWN_INERT: &[(&str, usize, &str)] = &[];

const KNOWN_EMPTY_GRAMMARS: &[&str] = &[];

fn is_possible(q: CaptureQuantifier) -> bool {
    !matches!(q, CaptureQuantifier::Zero)
}

/// Per-grammar measurement: the inert patterns, and whether the grammar has any
/// declaration-producing pattern at all.
struct GrammarMeasurement {
    inert: Vec<(usize, String)>,
    has_any_declaration_pattern: bool,
}

/// Walks every `Grammar::all()` variant's compiled `skeleton.scm` and reports,
/// per grammar, which patterns can never produce a declaration.
///
/// Grammars whose skeleton query fails to compile are skipped — compilation is
/// `query_compile_audit`'s assertion, not this one.
fn measure() -> BTreeMap<String, GrammarMeasurement> {
    let mut measured = BTreeMap::new();

    for grammar in Grammar::all() {
        let support = grammar.language_support();
        let query = match tree_sitter::Query::new(&support.ts_language(), support.skeleton_query())
        {
            Ok(q) => q,
            Err(_) => continue,
        };

        let capture_names = query.capture_names();
        let mut inert = Vec::new();
        let mut has_any_declaration_pattern = false;

        for pattern in 0..query.pattern_count() {
            let quantifiers = query.capture_quantifiers(pattern);
            let possible: Vec<&str> = capture_names
                .iter()
                .enumerate()
                .filter(|(i, _)| is_possible(quantifiers[*i]))
                .map(|(_, n)| *n)
                .collect();

            // A pattern with no possible capture at all binds nothing for this
            // audit to reason about (and cannot be a `skeleton.scm` bug of this
            // class); it is neither inert nor declaration-producing.
            if possible.is_empty() {
                continue;
            }
            if possible.iter().any(|n| is_declaration_producing(n)) {
                has_any_declaration_pattern = true;
            } else {
                inert.push((pattern, possible.join("+")));
            }
        }

        measured.insert(
            format!("{grammar:?}"),
            GrammarMeasurement {
                inert,
                has_any_declaration_pattern,
            },
        );
    }

    measured
}

#[test]
fn every_skeleton_pattern_can_produce_a_declaration() {
    let measured: BTreeSet<(String, usize, String)> = measure()
        .into_iter()
        .flat_map(|(grammar, m)| {
            m.inert
                .into_iter()
                .map(move |(pattern, captures)| (grammar.clone(), pattern, captures))
        })
        .collect();

    let pinned: BTreeSet<(String, usize, String)> = KNOWN_INERT
        .iter()
        .map(|(g, p, c)| ((*g).to_string(), *p, (*c).to_string()))
        .collect();

    let unpinned: Vec<_> = measured.difference(&pinned).collect();
    let fixed_but_pinned: Vec<_> = pinned.difference(&measured).collect();

    let render = |label: &str, rows: &[&(String, usize, String)]| {
        rows.iter()
            .map(|(g, p, c)| format!("  {label} (\"{g}\", {p}, \"{c}\"),"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert!(
        unpinned.is_empty() && fixed_but_pinned.is_empty(),
        "skeleton.scm patterns that can never produce a declaration do not match the pin \
         ({} unpinned inert, {} pinned-but-fixed; {} inert total measured):\n{}\n{}\n\
         An inert pattern binds no capture `execute_skeleton` acts on (the 33 kind captures, the \
         5 self-naming captures, `@definition`, and `@definition.<kind>`), so it emits nothing \
         for any input. A pinned-but-fixed entry must be removed from KNOWN_INERT in the commit \
         that fixed it.",
        unpinned.len(),
        fixed_but_pinned.len(),
        measured.len(),
        render("NEW  ", &unpinned),
        render("FIXED", &fixed_but_pinned),
    );
}

#[test]
fn every_grammar_can_produce_at_least_one_declaration() {
    let measured: BTreeSet<String> = measure()
        .into_iter()
        .filter(|(_, m)| !m.has_any_declaration_pattern)
        .map(|(grammar, _)| grammar)
        .collect();

    let pinned: BTreeSet<String> = KNOWN_EMPTY_GRAMMARS
        .iter()
        .map(|g| (*g).to_string())
        .collect();

    let unpinned: Vec<_> = measured.difference(&pinned).collect();
    let fixed_but_pinned: Vec<_> = pinned.difference(&measured).collect();

    let render = |label: &str, rows: &[&String]| {
        rows.iter()
            .map(|g| format!("  {label} \"{g}\","))
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert!(
        unpinned.is_empty() && fixed_but_pinned.is_empty(),
        "grammars whose skeleton.scm can never produce a declaration do not match the pin \
         ({} unpinned, {} pinned-but-fixed; {} total measured):\n{}\n{}\n\
         A pinned-but-fixed entry must be removed from KNOWN_EMPTY_GRAMMARS in the commit that \
         fixed it.",
        unpinned.len(),
        fixed_but_pinned.len(),
        measured.len(),
        render("NEW  ", &unpinned),
        render("FIXED", &fixed_but_pinned),
    );
}
