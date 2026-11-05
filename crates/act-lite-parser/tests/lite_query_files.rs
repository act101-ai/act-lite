//! act-lite ships navigation queries only: no analyzer query files.

use std::path::Path;

const ALLOWED: &[&str] = &[
    "skeleton.scm",
    "symbols.scm",
    "references.scm",
    "locals.scm",
];

#[test]
fn only_navigation_query_files_ship() {
    let queries = Path::new(env!("CARGO_MANIFEST_DIR")).join("queries");
    let mut stray = Vec::new();
    for entry in walkdir::WalkDir::new(&queries) {
        let entry = entry.unwrap_or_else(|e| panic!("cannot walk {}: {e}", queries.display()));
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !ALLOWED.contains(&name.as_ref()) {
            stray.push(entry.path().display().to_string());
        }
    }
    stray.sort();
    assert!(
        stray.is_empty(),
        "query files outside {ALLOWED:?}:\n  {}",
        stray.join("\n  ")
    );
}
