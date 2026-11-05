use act_lite_parser::Grammar;
use std::collections::BTreeSet;
use tree_sitter::CaptureQuantifier;

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

fn is_kind_capture(name: &str) -> bool {
    KIND_CAPTURES.contains(&name) || name == "definition" || name.starts_with("definition.")
}

const KNOWN_NAMELESS: &[(&str, usize, &str)] = &[];

fn is_guaranteed(q: CaptureQuantifier) -> bool {
    matches!(q, CaptureQuantifier::One | CaptureQuantifier::OneOrMore)
}

fn is_possible(q: CaptureQuantifier) -> bool {
    !matches!(q, CaptureQuantifier::Zero)
}

#[test]
fn every_skeleton_declaration_pattern_guarantees_a_name() {
    let mut measured: BTreeSet<(String, usize, String)> = BTreeSet::new();

    for grammar in Grammar::all() {
        let support = grammar.language_support();
        let query = match tree_sitter::Query::new(&support.ts_language(), support.skeleton_query())
        {
            Ok(q) => q,
            // Compilation is `query_compile_audit`'s assertion, not this one.
            Err(_) => continue,
        };

        let capture_names = query.capture_names();
        let indices = |wanted: &[&str]| -> Vec<usize> {
            capture_names
                .iter()
                .enumerate()
                .filter(|(_, n)| wanted.contains(&(**n)))
                .map(|(i, _)| i)
                .collect()
        };
        let kind_captures: Vec<usize> = capture_names
            .iter()
            .enumerate()
            .filter(|(_, n)| is_kind_capture(n))
            .map(|(i, _)| i)
            .collect();
        let self_naming_captures = indices(SELF_NAMING_CAPTURES);
        let name_captures = indices(&["name"]);

        for pattern in 0..query.pattern_count() {
            let quantifiers = query.capture_quantifiers(pattern);

            // A self-naming capture sets the name from its own node.
            if self_naming_captures
                .iter()
                .any(|&c| is_possible(quantifiers[c]))
            {
                continue;
            }
            if !kind_captures.iter().any(|&c| is_possible(quantifiers[c])) {
                continue;
            }
            if name_captures.iter().any(|&c| is_guaranteed(quantifiers[c])) {
                continue;
            }

            let reason = if name_captures.iter().all(|&c| !is_possible(quantifiers[c])) {
                "binds no @name"
            } else {
                "binds @name only optionally"
            };
            measured.insert((format!("{grammar:?}"), pattern, reason.to_string()));
        }
    }

    let pinned: BTreeSet<(String, usize, String)> = KNOWN_NAMELESS
        .iter()
        .map(|(g, p, r)| ((*g).to_string(), *p, (*r).to_string()))
        .collect();

    let new_carriers: Vec<_> = measured.difference(&pinned).collect();
    let fixed_but_pinned: Vec<_> = pinned.difference(&measured).collect();

    let render = |label: &str, rows: &[&(String, usize, String)]| {
        rows.iter()
            .map(|(g, p, r)| format!("  {label} (\"{g}\", {p}, \"{r}\"),"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert!(
        new_carriers.is_empty() && fixed_but_pinned.is_empty(),
        "skeleton.scm patterns that can emit a nameless declaration do not match the pin \
         ({} unpinned carrier(s), {} pinned-but-fixed):\n{}\n{}\n\
         A carrier has no name to emit, so `execute_skeleton`'s blank-name skip drops its \
         declaration entirely; a pinned-but-fixed entry must be removed from KNOWN_NAMELESS in the commit \
         that fixed it.",
        new_carriers.len(),
        fixed_but_pinned.len(),
        render("NEW  ", &new_carriers),
        render("FIXED", &fixed_but_pinned),
    );
}

const MULTI_NAME_ALLOWED: &[(&str, usize)] = &[];

#[test]
fn multi_name_captures_are_explicitly_allow_listed() {
    let mut measured: BTreeSet<(String, usize)> = BTreeSet::new();

    for grammar in Grammar::all() {
        let support = grammar.language_support();
        let query = match tree_sitter::Query::new(&support.ts_language(), support.skeleton_query())
        {
            Ok(q) => q,
            Err(_) => continue,
        };

        let capture_names = query.capture_names();
        let name_captures: Vec<usize> = capture_names
            .iter()
            .enumerate()
            .filter(|(_, n)| **n == "name")
            .map(|(i, _)| i)
            .collect();
        if name_captures.is_empty() {
            continue;
        }

        for pattern in 0..query.pattern_count() {
            let quantifiers = query.capture_quantifiers(pattern);
            let can_bind_more_than_once = name_captures.iter().any(|&c| {
                matches!(
                    quantifiers[c],
                    CaptureQuantifier::OneOrMore | CaptureQuantifier::ZeroOrMore
                )
            });
            if can_bind_more_than_once {
                measured.insert((format!("{grammar:?}"), pattern));
            }
        }
    }

    let allowed: BTreeSet<(String, usize)> = MULTI_NAME_ALLOWED
        .iter()
        .map(|(g, p)| ((*g).to_string(), *p))
        .collect();

    let unlisted: Vec<_> = measured.difference(&allowed).collect();
    let stale: Vec<_> = allowed.difference(&measured).collect();

    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "patterns binding @name more than once must be explicitly allow-listed in \
         MULTI_NAME_ALLOWED ({} unlisted, {} stale entries):\n\
         unlisted: {:?}\nstale: {:?}\n\
         execute_skeleton concatenates every @name capture's text for these patterns — correct \
         only when deliberately designed that way (see MULTI_NAME_ALLOWED's doc comment). A new \
         unlisted pattern is very likely a bug, not a feature.",
        unlisted.len(),
        stale.len(),
        unlisted,
        stale,
    );
}
