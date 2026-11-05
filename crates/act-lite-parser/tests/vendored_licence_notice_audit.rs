use std::path::{Path, PathBuf};

/// The notice filenames a vendored tree may satisfy the notice law with.
/// `LICENSE.txt` is included because tree-sitter-nim upstream ships its
/// MPL-2.0 notice under that exact name and the vendoring keeps
/// upstream's own filename.
const NOTICE_FILENAMES: &[&str] = &[
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "LICENCE",
    "COPYING",
    "COPYING.LESSER",
    "COPYRIGHT",
    "NOTICE",
];

const KNOWN_UPSTREAM_SHIPS_NO_NOTICE: &[&str] = &["apex"];

/// Every `vendor/*/` tree carrying a `Cargo.toml`, sorted by name.
fn vendored_trees() -> Vec<PathBuf> {
    let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
    let mut trees: Vec<PathBuf> = std::fs::read_dir(&vendor)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", vendor.display()))
        .map(|entry| entry.expect("readable entry under vendor/").path())
        .filter(|path| path.is_dir() && path.join("Cargo.toml").is_file())
        .collect();
    trees.sort();
    trees
}

fn package_lines(manifest: &str) -> Vec<&str> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[package]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .collect()
}

/// A vendored tree declares `license = "…"` in `[package]`.
fn declares_licence(package: &[&str]) -> bool {
    package.iter().any(|line| line.starts_with("license = "))
}

#[test]
fn every_vendored_grammar_declaring_a_licence_carries_its_notice_file() {
    let mut violations = Vec::new();
    for tree in vendored_trees() {
        let name = tree
            .file_name()
            .expect("vendor entries are named")
            .to_string_lossy()
            .into_owned();
        let manifest = std::fs::read_to_string(tree.join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("cannot read {}/Cargo.toml: {e}", tree.display()));
        if !declares_licence(&package_lines(&manifest)) {
            continue;
        }
        if KNOWN_UPSTREAM_SHIPS_NO_NOTICE
            .iter()
            .any(|waived| name.strip_prefix("tree-sitter-") == Some(*waived))
        {
            continue;
        }
        let notice = NOTICE_FILENAMES
            .iter()
            .map(|file| tree.join(file))
            .find(|path| path.is_file());
        match notice {
            Some(path) => {
                let contents = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                if contents.trim().is_empty() {
                    violations.push(format!("{name}: notice file is empty"));
                }
            }
            None => violations.push(format!(
                "{name}: Cargo.toml declares a licence but the tree carries no notice \
                 file (any of {}) — restore the upstream notice",
                NOTICE_FILENAMES.join(", ")
            )),
        }
    }
    assert!(
        violations.is_empty(),
        "vendored grammars lost their licence notices:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_vendored_manifest_claims_act_authorship() {
    let mut violations = Vec::new();
    for tree in vendored_trees() {
        let name = tree
            .file_name()
            .expect("vendor entries are named")
            .to_string_lossy()
            .into_owned();
        let manifest = std::fs::read_to_string(tree.join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("cannot read {}/Cargo.toml: {e}", tree.display()));
        for line in package_lines(&manifest) {
            if line.starts_with("authors = ") && line.contains("act developers") {
                violations.push(format!(
                    "{name}: {line} — the crate is upstream's; carry upstream's \
                     authors verbatim or no authors line at all"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "vendored manifests falsely attribute authorship:\n  {}",
        violations.join("\n  ")
    );
}
