use act_lite_parser::Grammar;

#[test]
fn every_advertised_extension_resolves_to_its_own_grammar() {
    let mut unreachable = Vec::new();
    for grammar in Grammar::all() {
        for ext in grammar.language_support().extensions() {
            match Grammar::from_extension(ext) {
                Some(resolved) if resolved == grammar => {}
                other => unreachable.push(format!(
                    "{:?} advertises {:?} but from_extension({:?}) = {:?}",
                    grammar, ext, ext, other
                )),
            }
        }
    }
    assert!(
        unreachable.is_empty(),
        "grammars unreachable through their own extensions:\n  {}",
        unreachable.join("\n  ")
    );
}

/// Extracts `(extension, grammar_variant_name)` pairs from every arm of
/// `Grammar::from_extension`'s match by scanning its literal source text —
/// there is no reflection over match arms in Rust. Every arm in that function
/// follows the single-line shape `"ext1" | "ext2" | ... => Some(Grammar::Name),`
/// (verified by reading the function at the time this audit was written), so
/// this walks the function body line by line rather than parsing full Rust.
fn parse_from_extension_arms() -> Vec<(String, String)> {
    parse_arms_from(include_str!("../src/grammar.rs"))
}

fn grammar_by_debug_name(name: &str) -> Grammar {
    Grammar::all()
        .into_iter()
        .find(|g| format!("{g:?}") == name)
        .unwrap_or_else(|| panic!("no Grammar variant named {name:?}"))
}

const DOTLESS_CONVENTIONAL_NAMES: &[&str] = &["justfile"];

#[test]
fn every_from_extension_arm_is_advertised_by_its_grammar() {
    from_extension_arm_pin(&parse_from_extension_arms());
}

fn from_extension_arm_pin(pairs: &[(String, String)]) {
    let mut orphaned = Vec::new();
    for (ext, grammar_name) in pairs {
        if DOTLESS_CONVENTIONAL_NAMES.contains(&ext.as_str()) {
            continue;
        }
        let grammar = grammar_by_debug_name(grammar_name);
        if !grammar
            .language_support()
            .extensions()
            .contains(&ext.as_str())
        {
            orphaned.push(format!(
                "from_extension({ext:?}) resolves to {grammar:?}, but {grammar:?} does not \
                 advertise {ext:?} in extensions()"
            ));
        }
    }
    assert!(
        orphaned.is_empty(),
        "from_extension arms whose grammar does not advertise the extension:\n  {}",
        orphaned.join("\n  ")
    );
}

fn blob_sha(bytes: &[u8]) -> String {
    use std::io::Write;
    let mut child = std::process::Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args(["hash-object", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "git hash-object --stdin failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// The panic payload as a message, so a catch can be required to NAME its
/// defeat rather than merely fire.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        String::from("panicked with a non-string payload")
    }
}

/// One in-memory edit: the anchor is asserted to occur exactly once, so a
/// drifted grammar.rs is a loud refusal, not a silent green.
fn apply_edit(text: &str, anchor: &str, replacement: &str) -> String {
    let occurrences = text.match_indices(anchor).count();
    assert_eq!(
        occurrences, 1,
        "the neuter anchor {anchor:?} occurs {occurrences} times in grammar.rs, not 1 — \
         the pinned source moved under the battery; re-ground the neuter before \
         trusting any verdict below"
    );
    text.replace(anchor, replacement)
}

fn every_grammar_fragment_line_is_a_parsed_arm(src: &str) {
    let start = src
        .find("pub fn from_extension(ext: &str) -> Option<Self> {")
        .expect("from_extension signature not found — grammar.rs was restructured");
    let body = &src[start..];
    let end = body
        .find("\n            _ => None,\n        }\n    }\n}")
        .expect("from_extension's closing `_ => None` arm not found — grammar.rs was restructured");
    let body = &body[..end];
    let mut fragments = Vec::new();
    for (i, line) in body.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.contains("Some(Grammar::") && !line.contains("=>") {
            fragments.push(format!("line {}: {line}", i + 1));
        }
    }
    assert!(
        fragments.is_empty(),
        "from_extension arm fragments the line-based parser silently skips (a \
         grammar fragment on a line without its own `=>` — the arm was split \
         across lines, so the audit never read it):\n  {}",
        fragments.join("\n  ")
    );
}

fn arm_pin_verdict(src: &str) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        from_extension_arm_pin(&parse_arms_from(src))
    }))
    .map_err(panic_message)
}

fn shape_canary_verdict(src: &str) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        every_grammar_fragment_line_is_a_parsed_arm(src)
    }))
    .map_err(panic_message)
}

fn parse_arms_from(src: &str) -> Vec<(String, String)> {
    let start = src
        .find("pub fn from_extension(ext: &str) -> Option<Self> {")
        .expect("from_extension signature not found — grammar.rs was restructured");
    let body = &src[start..];
    let end = body
        .find("\n            _ => None,\n        }\n    }\n}")
        .expect("from_extension's closing `_ => None` arm not found — grammar.rs was restructured");
    let body = &body[..end];

    let mut pairs = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let Some((exts_part, rest)) = line.split_once("=>") else {
            continue;
        };
        let Some(grammar_name) = rest
            .trim()
            .strip_prefix("Some(Grammar::")
            .and_then(|s| s.strip_suffix("),"))
        else {
            continue;
        };
        for ext in exts_part.split('|') {
            pairs.push((
                ext.trim().trim_matches('"').to_string(),
                grammar_name.to_string(),
            ));
        }
    }
    pairs
}

fn bitten_by(
    mutant: &str,
    history: &str,
    pristine_sha: &str,
    mutant_shas: &mut Vec<String>,
    verdict: fn(&str) -> Result<(), String>,
) -> String {
    let sha = blob_sha(mutant.as_bytes());
    assert_ne!(
        sha, pristine_sha,
        "the {history} neuter's mutation produced byte-identical text — it \
         never applied"
    );
    assert!(
        !mutant_shas.contains(&sha),
        "two neuters produced identical mutated bytes; these are not distinct \
         defeats"
    );
    mutant_shas.push(sha);
    match verdict(mutant) {
        Err(message) => message,
        Ok(()) => panic!(
            "the pin stayed GREEN under the {history} neuter — the battery \
             exists to prove that it cannot"
        ),
    }
}

/// The battery proper: every neuter of the from-extension audit must be
/// CAUGHT — by the pin failing on it, or (for the pin's disclosed blind
/// spots) by the shape-lock canary — and the catch must NAME the defeat. All
/// surgery is in-memory on the read grammar.rs text; nothing is ever
/// written.
#[test]
fn from_extension_arm_pin_bites_on_its_neuters() {
    let pristine = include_str!("../src/grammar.rs");
    // GREEN controls: the pin and the shape canary pass the live bytes, and
    // the mutated-source parse reproduces the live parse byte-for-byte (the
    // factoring is the same instrument).
    assert_eq!(parse_arms_from(pristine), parse_from_extension_arms());
    arm_pin_verdict(pristine).expect("the pin must pass the live grammar.rs bytes");
    shape_canary_verdict(pristine).expect("the shape canary must pass the live grammar.rs bytes");
    let pristine_sha = blob_sha(pristine.as_bytes());
    let mut mutant_shas: Vec<String> = Vec::new();
    let terminator = "\n            _ => None,\n        }\n    }\n}";

    // Neuter 1 — orphan arm planted: an extension no grammar advertises,
    // resolving to a real grammar. The pin's own direction: the arm exists
    // but its grammar does not advertise the extension.
    let message = bitten_by(
        &apply_edit(
            pristine,
            terminator,
            "\n            \"f1562_orphan_probe\" => Some(Grammar::Go),\
             \n            _ => None,\n        }\n    }\n}",
        ),
        "orphan arm planted (an extension no grammar advertises)",
        &pristine_sha,
        &mut mutant_shas,
        arm_pin_verdict,
    );
    assert!(
        message.contains("f1562_orphan_probe"),
        "the catch did not name the orphan extension:\n{message}"
    );

    // Neuter 2 — variant swap: the first parsed arm's grammar respelled to a
    // different grammar that (verified at run time) does not advertise the
    // arm's extension. Same catch direction, reached by mutating a real arm
    // rather than planting one.
    {
        let (ext, name) = parse_arms_from(pristine)[0].clone();
        let grammar = grammar_by_debug_name(&name);
        let other = Grammar::all()
            .into_iter()
            .find(|g| *g != grammar && !g.language_support().extensions().contains(&ext.as_str()))
            .unwrap_or_else(|| panic!("no grammar avoids advertising {ext:?}; re-ground"));
        let arm_line = pristine
            .lines()
            .find(|line| {
                line.contains(&format!("\"{ext}\""))
                    && line.contains(&format!("Some(Grammar::{name}),"))
            })
            .expect("the first parsed arm's source line")
            .to_string();
        let swapped = apply_edit(
            pristine,
            arm_line.trim(),
            &arm_line.trim().replace(
                &format!("Some(Grammar::{name}),"),
                &format!("Some(Grammar::{other:?}),"),
            ),
        );
        let message = bitten_by(
            &swapped,
            "variant swap (a real arm resolves to a grammar that does not advertise it)",
            &pristine_sha,
            &mut mutant_shas,
            arm_pin_verdict,
        );
        assert!(
            message.contains(&format!("{ext:?}")),
            "the catch did not name the mis-routed extension:\n{message}"
        );
    }

    // Neuter 3 — arm deleted (the pin's disclosed blind spot): the whole arm
    // line removed. The parse simply has fewer pairs, so the pin is GREEN —
    // the executable disclosure. The closure is the sibling runtime test
    // `every_advertised_extension_resolves_to_its_own_grammar`, whose
    // criterion (`from_extension(ext) == the advertising grammar`) breaks on
    // the live fn the moment the arm leaves the source; the control below
    // names that criterion on the live runtime for this exact extension.
    {
        let (ext, name) = parse_arms_from(pristine)[0].clone();
        let grammar = grammar_by_debug_name(&name);
        let arm_line = pristine
            .lines()
            .find(|line| {
                line.contains(&format!("\"{ext}\""))
                    && line.contains(&format!("Some(Grammar::{name}),"))
            })
            .expect("the first parsed arm's source line")
            .to_string();
        let deleted = apply_edit(pristine, &format!("{}\n", arm_line.trim()), "");
        assert!(
            arm_pin_verdict(&deleted).is_ok(),
            "the pin caught the arm-deleted neuter — its parse changed since this \
             battery spelled the blind spot; re-ground the neuter"
        );
        assert_eq!(
            Grammar::from_extension(&ext),
            Some(grammar),
            "the runtime control: the live from_extension resolves {ext:?} to its \
             advertising grammar — deleting the arm breaks exactly this, which is \
             the sibling test's criterion, not this pin's"
        );
        let sha = blob_sha(deleted.as_bytes());
        assert_ne!(sha, pristine_sha, "the neuter never applied");
        assert!(!mutant_shas.contains(&sha));
        mutant_shas.push(sha);
    }

    // Neuter 4 — arm split across lines (the shape-blindness the census
    // flagged): the arm's `Some(Grammar::…)` fragment moved to its own line.
    // The line-based parser skips both halves, so the pin is GREEN — the
    // executable disclosure — and the shape-lock canary refuses the silent
    // skip.
    {
        let (ext, name) = parse_arms_from(pristine)[0].clone();
        let arm_line = pristine
            .lines()
            .find(|line| {
                line.contains(&format!("\"{ext}\""))
                    && line.contains(&format!("Some(Grammar::{name}),"))
            })
            .expect("the first parsed arm's source line")
            .to_string();
        let split_arm = format!(
            "            \"{}\" =>\n                Some(Grammar::{}),",
            ext, name
        );
        let split = apply_edit(pristine, arm_line.trim(), &split_arm);
        assert!(
            arm_pin_verdict(&split).is_ok(),
            "the pin caught the split-arm neuter — its parse changed since this \
             battery spelled the blind spot; re-ground the neuter"
        );
        let message = bitten_by(
            &split,
            "arm split across lines (the parser silently skips both halves)",
            &pristine_sha,
            &mut mutant_shas,
            shape_canary_verdict,
        );
        assert!(
            message.contains("silently skips"),
            "the shape canary's catch did not name the silent skip:\n{message}"
        );
    }

    assert_eq!(
        mutant_shas.len(),
        4,
        "the battery's floor: four distinct defeats"
    );
    let live =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/grammar.rs")).unwrap();
    assert_eq!(
        blob_sha(live.as_bytes()),
        pristine_sha,
        "the live grammar.rs changed while the battery ran — the battery must \
         never write; re-read and re-ground"
    );
}
