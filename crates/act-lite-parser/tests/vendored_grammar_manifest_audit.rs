use std::path::Path;

/// The grammars vendored under the repo-root `vendor/`.
const VENDORED_GRAMMARS: &[&str] = &["dart", "scss", "sequel", "kotlin"];

/// The lines of one section (`[package]`, `[dependencies]`, ...) of a
/// manifest, up to the next section header. Every consumer of this scan
/// relies on the repo's manifests keeping keys unindented at column 0 —
/// the shape TOML requires of these tables anyway.
fn section_lines<'a>(manifest: &'a str, header: &str) -> Vec<&'a str> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != header)
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .collect()
}

/// The `[dependencies]` line declaring `tree-sitter-<name>`, if any. The
/// ` = ` suffix keeps `tree-sitter-v` from matching `tree-sitter-vbnet`.
fn dependency_line<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("tree-sitter-{name} = ");
    section_lines(manifest, "[dependencies]")
        .into_iter()
        .find(|line| line.starts_with(&prefix))
}

/// The `[dependencies]` or `[dev-dependencies]` line declaring
/// `tree-sitter-<name>`, if any — the same match as `dependency_line`,
/// across both sections a member manifest may declare grammars in.
fn dependency_line_any_section<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("tree-sitter-{name} = ");
    section_lines(manifest, "[dependencies]")
        .into_iter()
        .chain(section_lines(manifest, "[dev-dependencies]"))
        .find(|line| line.starts_with(&prefix))
}

/// The value of a quoted `key = "value"` field inside a single-line table
/// dependency entry, e.g. the `path` of `{ path = "...", version = "..." }`.
fn quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("{key} = \"");
    let start = line.find(&marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[test]
fn root_manifest_carries_no_patch_crates_io_section() {
    let root = include_str!("../../../Cargo.toml");
    assert!(
        !root.lines().any(|line| line.trim().starts_with("[patch")),
        "the root Cargo.toml carries a patch section — it was retired by \
         path-declaring the vendored grammars in the crate manifests; fix the \
         declarations there instead of re-patching crates.io. The predicate matches \
         any spelling ([patch.crates-io], [patch.\"crates-io\"], \
         [patch.\"https://crates.io\"])"
    );
}

#[test]
fn every_vendored_grammar_is_path_declared_and_resolves() {
    let manifest = include_str!("../Cargo.toml");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    for name in VENDORED_GRAMMARS {
        let Some(line) = dependency_line(manifest, name) else {
            violations.push(format!(
                "tree-sitter-{name} is not declared in [dependencies]"
            ));
            continue;
        };
        let Some(path) = quoted_value(line, "path") else {
            violations.push(format!(
                "tree-sitter-{name} is declared without a path entry: {line}"
            ));
            continue;
        };
        let resolved = manifest_dir.join(path);
        if !resolved.is_dir() {
            violations.push(format!(
                "tree-sitter-{name} path {path} resolves to {resolved:?}, which is not \
                 an existing directory"
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "vendored grammar declarations drifted from the path idiom:\n  {}",
        violations.join("\n  ")
    );
}

/// The pin must hold in EVERY member manifest, not just act-lite-parser's:
/// another member may declare a grammar by `path`, and a registry-form
/// regression there (`tree-sitter-scss = "1.0.0"`) would resolve silently
/// through crates.io — no patch block remains to catch it — while an
/// act-lite-parser-only pin stayed green. The manifests are read from the file
/// system at runtime, so a crate added later is covered the day it lands.
/// The same scan carries the broadened `[patch` predicate, so no member
/// manifest may reinstate the redirect under any cargo spelling.
#[test]
fn every_member_manifest_path_declares_vendored_grammars_and_carries_no_patch() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("act-lite-parser sits directly under crates/")
        .to_path_buf();
    let mut manifests: Vec<_> = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", crates_dir.display()))
        .map(|entry| entry.expect("readable entry under crates/").path())
        .filter(|path| path.is_dir())
        .map(|dir| dir.join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();
    let mut violations = Vec::new();
    for manifest_path in &manifests {
        let manifest = std::fs::read_to_string(manifest_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));
        if let Some(line) = manifest.lines().find(|l| l.trim().starts_with("[patch")) {
            violations.push(format!(
                "{} carries a patch section — the crates.io redirect is retired; \
                 declare vendored grammars by path instead: {line}",
                manifest_path.display()
            ));
        }
        for name in VENDORED_GRAMMARS {
            if let Some(line) = dependency_line_any_section(&manifest, name)
                && quoted_value(line, "path").is_none()
            {
                violations.push(format!(
                    "{} declares tree-sitter-{name} without a path entry — the \
                     vendored tree is bypassed and crates.io resolves silently: {line}",
                    manifest_path.display()
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "member manifests drifted from the vendored-grammar path idiom:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn act_lite_parser_is_not_publishable() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        section_lines(manifest, "[package]")
            .iter()
            .any(|line| line.trim() == "publish = false"),
        "act-lite-parser's [package] carries no `publish = false` — no publish step \
         exists"
    );
}
