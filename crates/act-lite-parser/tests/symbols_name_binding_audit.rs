use act_lite_parser::Grammar;
use std::collections::BTreeSet;
use tree_sitter::CaptureQuantifier;

const KNOWN_NAMELESS: &[(&str, usize, &str)] = &[];

fn is_guaranteed(q: CaptureQuantifier) -> bool {
    matches!(q, CaptureQuantifier::One | CaptureQuantifier::OneOrMore)
}

fn is_possible(q: CaptureQuantifier) -> bool {
    !matches!(q, CaptureQuantifier::Zero)
}

#[test]
fn every_definition_pattern_guarantees_a_name() {
    let mut measured: BTreeSet<(String, usize, String)> = BTreeSet::new();

    for grammar in Grammar::all() {
        let support = grammar.language_support();
        let query = match tree_sitter::Query::new(&support.ts_language(), support.symbols_query()) {
            Ok(q) => q,
            // Compilation is `query_compile_audit`'s assertion, not this one.
            Err(_) => continue,
        };

        let capture_names = query.capture_names();
        let definition_captures: Vec<usize> = capture_names
            .iter()
            .enumerate()
            .filter(|(_, n)| **n == "definition" || n.starts_with("definition."))
            .map(|(i, _)| i)
            .collect();
        let name_captures: Vec<usize> = capture_names
            .iter()
            .enumerate()
            .filter(|(_, n)| **n == "name")
            .map(|(i, _)| i)
            .collect();

        for pattern in 0..query.pattern_count() {
            let quantifiers = query.capture_quantifiers(pattern);

            let emits_definition = definition_captures
                .iter()
                .any(|&c| is_possible(quantifiers[c]));
            if !emits_definition {
                continue;
            }

            let guarantees_name = name_captures.iter().any(|&c| is_guaranteed(quantifiers[c]));
            if guarantees_name {
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
            .map(|(g, p, r)| format!("  {label} {g} pattern #{p}: {r}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert!(
        new_carriers.is_empty() && fixed_but_pinned.is_empty(),
        "symbols.scm patterns that can emit a nameless symbol do not match the pin \
         ({} unpinned carrier(s), {} pinned-but-fixed):\n{}\n{}\n\
         A carrier emits `{{\"name\": \"\"}}`; a pinned-but-fixed entry \
         must be removed from KNOWN_NAMELESS in the commit that fixed it.",
        new_carriers.len(),
        fixed_but_pinned.len(),
        render("NEW    ", &new_carriers),
        render("FIXED  ", &fixed_but_pinned),
    );
}
