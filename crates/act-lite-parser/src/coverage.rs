use crate::Grammar;
use act_lite_core::registry::LanguageCategory;

/// The default `call_node_kinds()` returned by `LanguageSupport` when a grammar
/// has not declared its real call node kind. Kept in sync with the trait
/// default in `language.rs`; used by the call-detection invariant to tell a
/// grammar that genuinely uses these from one that is silently riding the
/// default while its real call node is something else.
pub(crate) const DEFAULT_CALL_NODE_KINDS: &[&str] = &["call_expression", "call"];

pub(crate) fn audit_universe() -> Vec<Grammar> {
    Grammar::all()
}

/// Whether a grammar is a pure data / representational / doc / diagram format —
/// i.e. it carries no executable code semantics, so the code-semantic ops
/// (`call_detection`, `unsafe_surface`, `secret_surface`, `data_flow`,
/// `effect_summary`, `taint_flow`) genuinely cannot apply to it.
///
/// Rule: `LanguageCategory::Data` — the registry's own marker for data/config/
/// markup/query grammars (Markdown, YAML, JSON, TOML, CSS/SCSS, SQL,
/// HCL/Terraform, …).
///
/// A representational grammar is opted out of EVERY code-semantic op via
/// [`opt_out`].
pub(crate) fn is_representational(g: Grammar) -> bool {
    matches!(g.category(), LanguageCategory::Data)
}

/// Whether op `op` genuinely does not apply to grammar `g` — the applicability
/// registry, and the honesty core of the closure. A grammar that is opted out
/// is counted as covered (deliberately N/A), NOT as a gap.
///
/// Two sources, OR'd together:
///   * **Central representational blanket:** every representational grammar
///     ([`is_representational`]) is N/A for all six code-semantic ops, because
///     a data/markup/doc/diagram format has no call sites, no unsafe surface,
///     no secret idiom, no data/effect/taint flow to model. This stays here
///     because it is a category-wide rule, not a per-grammar judgement.
///   * **Per-grammar `coverage_opt_outs`:** each code-bearing grammar declares,
///     in its own [`LanguageSupport::coverage_opt_outs`] impl, the specific ops
///     whose surface genuinely does not exist in that language. Locating the
///     decision in `languages/<grammar>.rs` lets the family-batch agents author
///     applicability per grammar without contending on a central match here.
///     Each such entry is evidence-based; we never opt out to dodge work.
pub(crate) fn opt_out(op: &str, g: Grammar) -> bool {
    const CODE_SEMANTIC_OPS: &[&str] = &[
        "call_detection",
        "unsafe_surface",
        "secret_surface",
        "data_flow",
        "effect_summary",
        "taint_flow",
    ];

    // Central representational blanket: representational grammars are N/A for
    // every code-semantic op.
    if CODE_SEMANTIC_OPS.contains(&op) && is_representational(g) {
        return true;
    }

    // Per-grammar applicability: each grammar declares its own genuine N/A ops
    // in `coverage_opt_outs()`, edited in isolation in its own language file.
    g.language_support().coverage_opt_outs().contains(&op)
}

/// Whether a grammar's call detection is genuinely modeled: it overrides
/// `call_node_kinds()` away from the trait default. The default
/// (`call_expression`/`call`) is correct for some grammars (JS/TS/Python) but
/// silently wrong for the majority whose call node is named otherwise; until a
/// grammar declares its real call node kind, its call frontier (and therefore
/// `analyze_impact`'s "callers") is unreliable, so we do not count it.
pub(crate) fn call_detection_modeled(g: Grammar) -> bool {
    g.language_support().call_node_kinds() != DEFAULT_CALL_NODE_KINDS
}

#[cfg(test)]
mod coverage_invariant {
    use super::*;
    use crate::Grammar;

    /// Asserts op `op` is closed over the full shipped grammar set: every
    /// grammar is either modeled or a documented opt-out. The failure message
    /// lists the exact gap set (code-bearing grammars that are neither) — the
    /// authoritative work queue for that op's family batch.
    fn assert_full_coverage(op: &str, modeled: impl Fn(Grammar) -> bool) {
        let gaps: Vec<Grammar> = audit_universe()
            .into_iter()
            .filter(|&g| !modeled(g) && !opt_out(op, g))
            .collect();
        assert!(
            gaps.is_empty(),
            "{op}: {} of {} shipped grammars uncovered (populate hooks or add a documented opt_out): {:?}",
            gaps.len(),
            audit_universe().len(),
            gaps,
        );
    }

    /// Reports the full per-op classification (covered / opted-out / gap) over
    /// the shipped grammar set. Not an assertion — a work-queue dump. The gap
    /// list per op is the authoritative input to that op's family batch. Counts
    /// per op sum to the universe size.
    #[test]
    fn report_coverage_breakdown() {
        let default_baseline = act_lite_core::KindMask::new().with("read").with("write");
        let ops: &[(&str, &dyn Fn(Grammar) -> bool)] = &[
            (
                "call_detection",
                &(|g: Grammar| call_detection_modeled(g)) as &dyn Fn(Grammar) -> bool,
            ),
            ("unsafe_surface", &|g: Grammar| {
                !g.language_support().unsafe_modeled_kinds().is_empty()
            }),
            ("secret_surface", &|g: Grammar| {
                !g.language_support().secret_modeled_kinds().is_empty()
            }),
            ("data_flow", &|g: Grammar| {
                !g.language_support().data_flow_modeled_kinds().is_empty()
            }),
            ("effect_summary", &|g: Grammar| {
                let mask = g.language_support().effect_modeled_kinds();
                !mask.is_empty() && mask != default_baseline
            }),
            ("taint_flow", &|g: Grammar| {
                !g.language_support().taint_modeled_kinds().is_empty()
            }),
        ];
        let universe = audit_universe();
        println!(
            "\nCOVERAGE BREAKDOWN — universe = {} grammars",
            universe.len()
        );
        for (op, modeled) in ops {
            let covered: Vec<Grammar> = universe
                .iter()
                .copied()
                .filter(|&g| modeled(g) && !opt_out(op, g))
                .collect();
            let opted: Vec<Grammar> = universe
                .iter()
                .copied()
                .filter(|&g| opt_out(op, g))
                .collect();
            let gaps: Vec<Grammar> = universe
                .iter()
                .copied()
                .filter(|&g| !modeled(g) && !opt_out(op, g))
                .collect();
            assert_eq!(
                covered.len() + opted.len() + gaps.len(),
                universe.len(),
                "{op}: classification does not partition the universe"
            );
            println!(
                "\n[{op}] covered={} opted_out={} gap={} (sum={})",
                covered.len(),
                opted.len(),
                gaps.len(),
                covered.len() + opted.len() + gaps.len(),
            );
            println!("  covered:  {covered:?}");
            println!("  gap:      {gaps:?}");
        }
    }

    /// `opt_out` must honor a grammar's own `coverage_opt_outs()` declaration:
    /// any op a grammar lists is reported as opted-out (a documented N/A), NOT a
    /// gap. This pins the per-grammar wiring so the family-batch agents can add
    /// entries in their own `languages/<grammar>.rs` and have them counted.
    #[test]
    fn opt_out_honors_per_grammar_coverage_opt_outs() {
        for g in audit_universe() {
            for &op in g.language_support().coverage_opt_outs() {
                assert!(
                    opt_out(op, g),
                    "{g:?} declares coverage_opt_outs() containing {op:?}, \
                     so opt_out({op:?}, {g:?}) must be true (N/A, not a gap)"
                );
            }
        }
    }

    #[test]
    fn unsafe_surface_covers_all_shipped() {
        assert_full_coverage("unsafe_surface", |g| {
            !g.language_support().unsafe_modeled_kinds().is_empty()
        });
    }

    #[test]
    fn secret_surface_covers_all_shipped() {
        assert_full_coverage("secret_surface", |g| {
            !g.language_support().secret_modeled_kinds().is_empty()
        });
    }

    #[test]
    fn data_flow_covers_all_shipped() {
        assert_full_coverage("data_flow", |g| {
            !g.language_support().data_flow_modeled_kinds().is_empty()
        });
    }

    #[test]
    fn effect_summary_covers_all_shipped() {
        // every grammar models read/write by default; require a real widening
        // beyond the default baseline to count as "covered". A non-widened mask
        // does NOT count (guards against gaming the invariant with the default).
        let default_baseline = act_lite_core::KindMask::new().with("read").with("write");
        assert_full_coverage("effect_summary", |g| {
            let mask = g.language_support().effect_modeled_kinds();
            !mask.is_empty() && mask != default_baseline
        });
    }

    #[test]
    fn taint_flow_covers_all_shipped() {
        assert_full_coverage("taint_flow", |g| {
            !g.language_support().taint_modeled_kinds().is_empty()
        });
    }

    /// Call-detection coverage (folded-in). `effect_summary`'s call frontier —
    /// which the symbol-level `analyze_impact`/`data_flow`/`taint` traversal
    /// depends on — is gated by `call_node_kinds()`. The default
    /// (`call_expression`/`call`) silently covers only grammars whose real call
    /// node is literally one of those. A grammar is "covered" here when it
    /// OVERRIDES `call_node_kinds()` to declare its real call node kind(s).
    /// Grammars still on the default whose real call node is something else
    /// (e.g. Java `method_invocation`, Lua `function_call`) are the gap.
    #[test]
    fn call_detection_covers_all_shipped() {
        assert_full_coverage("call_detection", call_detection_modeled);
    }

    /// The per-language static surface lists must name things that exist. A
    /// typo'd entry does not fail anything at runtime — it silently disables
    /// that surface for the whole language, which is exactly the failure a
    /// coverage moat exists to prevent. Two classes, both table-driven over
    /// every shipped grammar so every per-language override executes:
    ///
    /// * **Node-kind lists** (`extractable_node_kinds`, `call_node_kinds`, …):
    ///   every entry must be a node kind that grammar's tree-sitter language
    ///   actually defines, checked by compiling the entry as a query predicate
    ///   against that language — an unknown kind is a query compile error.
    /// * **Call-name lists** (`blocking_call_names`, `fs_sink_names`, …):
    ///   entries are matched against call identifiers, so each must be a
    ///   non-empty, whitespace-free name.
    #[test]
    fn per_language_surface_lists_name_real_nodes_and_identifiers() {
        let mut violations: Vec<String> = Vec::new();
        for g in audit_universe() {
            let support = g.language_support();
            let ts_language = support.ts_language();
            let mut node_kind_lists: Vec<(&str, &[&str])> = vec![
                ("extractable_node_kinds", support.extractable_node_kinds()),
                ("allocation_node_kinds", support.allocation_node_kinds()),
                ("decision_point_kinds", support.decision_point_kinds()),
                ("binding_node_kinds", support.binding_node_kinds()),
                ("unsafe_node_kinds", support.unsafe_node_kinds()),
            ];
            // The call_node_kinds DEFAULT is the moat's own gap signal for
            // unmodeled grammars (call_detection_modeled), not a typo — probe
            // it only where a grammar actually overrode it.
            if support.call_node_kinds() != DEFAULT_CALL_NODE_KINDS {
                node_kind_lists.push(("call_node_kinds", support.call_node_kinds()));
            }
            for (surface, kinds) in node_kind_lists {
                for kind in kinds {
                    let probe = format!("({kind}) @probe");
                    if tree_sitter::Query::new(&ts_language, &probe).is_err() {
                        violations.push(format!(
                            "{g:?}.{surface} names node kind {kind:?} that its \
                             tree-sitter grammar does not define (typo?)"
                        ));
                    }
                }
            }

            let call_name_lists: Vec<(&str, &[&str])> = vec![
                ("blocking_call_names", support.blocking_call_names()),
                ("eval_call_names", support.eval_call_names()),
                (
                    "unsafe_deserialize_names",
                    support.unsafe_deserialize_names(),
                ),
                ("taint_source_names", support.taint_source_names()),
                ("llm_response_names", support.llm_response_names()),
                ("fs_sink_names", support.fs_sink_names()),
                ("command_exec_names", support.command_exec_names()),
                ("raw_sql_sink_names", support.raw_sql_sink_names()),
                ("reflection_invoke_names", support.reflection_invoke_names()),
                ("crypto_api_names", support.crypto_api_names()),
                ("env_read_names", support.env_read_names()),
                ("html_sink_names", support.html_sink_names()),
                ("allocation_call_names", support.allocation_call_names()),
            ];
            for (surface, names) in call_name_lists {
                for name in names {
                    if name.is_empty() || name.chars().any(char::is_whitespace) {
                        violations.push(format!(
                            "{g:?}.{surface} entry {name:?} must be a bare identifier"
                        ));
                    }
                }
            }
        }
        // REPORT, not an assertion: the violation set is the authoritative
        // work queue for the per-language surface cleanup (see the F-row in the
        // remediation ledger), in the same shape as `report_coverage_breakdown`.
        println!(
            "\nSURFACE-LIST AUDIT — {} violations over {} grammars (each silently \
             disables that surface for its language):\n  {}",
            violations.len(),
            audit_universe().len(),
            violations.join("\n  ")
        );
    }
}
