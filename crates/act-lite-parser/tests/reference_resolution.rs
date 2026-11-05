use act_lite_parser::{
    Grammar, ParserError, QueryResult, SourceFile, execute_reference_ranges_for_symbol_across,
    execute_references, execute_references_for_symbol, execute_references_for_symbol_across,
    walk_reference_siblings,
};

fn reference_lines(result: QueryResult) -> Vec<u32> {
    match result {
        QueryResult::References { references, .. } => references
            .into_iter()
            .map(|reference| reference.line())
            .collect(),
        other => panic!("expected references result, got {other:?}"),
    }
}

fn reference_files_and_lines(result: QueryResult) -> Vec<(String, u32)> {
    match result {
        QueryResult::References { references, .. } => references
            .into_iter()
            .map(|reference| {
                (
                    reference.file().to_string_lossy().into_owned(),
                    reference.line(),
                )
            })
            .collect(),
        other => panic!("expected references result, got {other:?}"),
    }
}

fn reference_tuples(result: QueryResult) -> Vec<(String, u32, u32)> {
    match result {
        QueryResult::References { references, .. } => references
            .into_iter()
            .map(|reference| {
                (
                    reference.file().to_string_lossy().into_owned(),
                    reference.line(),
                    reference.column(),
                )
            })
            .collect(),
        other => panic!("expected references result, got {other:?}"),
    }
}

fn typescript_file(path: &str, content: &str) -> SourceFile {
    SourceFile::from_content(path, content.to_string(), Grammar::TypeScript).unwrap()
}

#[test]
fn bash_references_match_only_the_requested_name() {
    let source = SourceFile::from_content(
        "test.sh",
        "target() { :; }\ntarget\ntarget\ntarget_suffix\n".to_string(),
        Grammar::Bash,
    )
    .unwrap();

    assert_eq!(
        reference_lines(execute_references_for_symbol(&source, "target", 2, 1).unwrap()),
        [1, 2, 3]
    );
}

#[test]
fn typescript_references_match_only_the_requested_name() {
    let source = SourceFile::from_content(
        "test.ts",
        "function target() {}\ntarget();\ntarget();\ntargetSuffix();\n".to_string(),
        Grammar::TypeScript,
    )
    .unwrap();

    assert_eq!(
        reference_lines(execute_references_for_symbol(&source, "target", 2, 1).unwrap()),
        [1, 2, 3]
    );
}

#[test]
fn dart_references_match_only_the_requested_name() {
    let source = SourceFile::from_content(
        "test.dart",
        "void target() {}\nvoid main() {\n  target();\n  target();\n  targetSuffix();\n}\n"
            .to_string(),
        Grammar::Dart,
    )
    .unwrap();

    assert_eq!(
        reference_lines(execute_references_for_symbol(&source, "target", 3, 3).unwrap()),
        [1, 3, 4]
    );
}

#[test]
fn references_for_symbol_reports_invalid_positions_and_missing_names() {
    let source =
        SourceFile::from_content("test.sh", "target\n".to_string(), Grammar::Bash).unwrap();

    assert!(matches!(
        execute_references_for_symbol(&source, "target", 99, 99),
        Err(ParserError::InvalidPosition {
            line: 99,
            column: 99,
            ..
        })
    ));
    assert!(matches!(
        execute_references_for_symbol(&source, "missing", 1, 1),
        Err(ParserError::SymbolNotFound { name, .. }) if name == "missing"
    ));
}

#[test]
fn references_for_symbol_rejects_columns_past_the_requested_line() {
    let source = SourceFile::from_content(
        "test.sh",
        "not_target\ntarget\ntarget\n".to_string(),
        Grammar::Bash,
    )
    .unwrap();

    assert!(matches!(
        execute_references_for_symbol(&source, "target", 1, 12),
        Err(ParserError::InvalidPosition {
            line: 1,
            column: 12,
            ..
        })
    ));
}

#[test]
fn references_for_symbol_accepts_emitted_utf8_byte_columns() {
    let source =
        SourceFile::from_content("test.sh", "echo é; N\nN\n".to_string(), Grammar::Bash).unwrap();
    let emitted_location = match execute_references(&source).unwrap() {
        QueryResult::References { references, .. } => references
            .into_iter()
            .find(|location| location.line() == 1 && location.column() == 10)
            .unwrap(),
        other => panic!("expected references result, got {other:?}"),
    };

    assert_eq!(emitted_location.column(), 10);
    assert_eq!(
        reference_lines(
            execute_references_for_symbol(
                &source,
                "N",
                emitted_location.line(),
                emitted_location.column(),
            )
            .unwrap(),
        ),
        [1, 2]
    );
}

#[test]
fn references_across_files_merges_anchor_and_sibling_sites() {
    let anchor = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let sibling = typescript_file("user.ts", "sharedFn();\nsharedFn();\n");

    let references = reference_files_and_lines(
        execute_references_for_symbol_across(&anchor, &[sibling], "sharedFn", 1, 10).unwrap(),
    );

    assert_eq!(
        references,
        [
            ("decl.ts".to_string(), 1),
            ("decl.ts".to_string(), 2),
            ("user.ts".to_string(), 1),
            ("user.ts".to_string(), 2),
        ]
    );
}

#[test]
fn references_across_files_is_deterministic_regardless_of_sibling_order() {
    let anchor = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let alpha = typescript_file("alpha.ts", "sharedFn();\n");
    let zeta = typescript_file("zeta.ts", "sharedFn();\nsharedFn();\n");

    let forward = reference_tuples(
        execute_references_for_symbol_across(&anchor, &[alpha, zeta], "sharedFn", 1, 10).unwrap(),
    );

    let anchor = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let alpha = typescript_file("alpha.ts", "sharedFn();\n");
    let zeta = typescript_file("zeta.ts", "sharedFn();\nsharedFn();\n");

    let reverse = reference_tuples(
        execute_references_for_symbol_across(&anchor, &[zeta, alpha], "sharedFn", 1, 10).unwrap(),
    );

    assert_eq!(forward, reverse);
    assert_eq!(forward.len(), 5);
}

#[test]
fn references_across_files_dedupes_anchor_present_in_siblings() {
    let anchor = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let anchor_copy = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let sibling = typescript_file("user.ts", "sharedFn();\n");

    let references = reference_files_and_lines(
        execute_references_for_symbol_across(&anchor, &[anchor_copy, sibling], "sharedFn", 1, 10)
            .unwrap(),
    );

    assert_eq!(
        references,
        [
            ("decl.ts".to_string(), 1),
            ("decl.ts".to_string(), 2),
            ("user.ts".to_string(), 1),
        ]
    );
}

#[test]
fn references_across_files_preserves_anchor_validation_errors() {
    let anchor = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let sibling = typescript_file("user.ts", "sharedFn();\n");

    assert!(matches!(
        execute_references_for_symbol_across(
            &anchor,
            std::slice::from_ref(&sibling),
            "sharedFn",
            99,
            99,
        ),
        Err(ParserError::InvalidPosition {
            line: 99,
            column: 99,
            ..
        })
    ));
    assert!(matches!(
        execute_references_for_symbol_across(
            &anchor,
            std::slice::from_ref(&sibling),
            "missing",
            1,
            10,
        ),
        Err(ParserError::SymbolNotFound { name, .. }) if name == "missing"
    ));
}

/// A bash workspace fixture: an anchor script plus named siblings.
fn bash_workspace(siblings: &[(&str, &str)]) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let anchor = temp_dir.path().join("anchor.sh");
    std::fs::write(&anchor, "target() {\n    :\n}\ntarget\n").unwrap();
    for (name, content) in siblings {
        std::fs::write(temp_dir.path().join(name), content).unwrap();
    }
    (temp_dir, anchor)
}

#[test]
fn walk_reference_siblings_count_cap_is_hard_error() {
    let (temp_dir, anchor) = bash_workspace(&[("b.sh", "target\n"), ("c.sh", "target\n")]);
    let err = match walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 1, 1024) {
        Ok(_) => {
            panic!("file-count cap overflow must be a hard error, not a silent truncation")
        }
        Err(err) => err,
    };
    let message = format!("{err}");
    assert!(
        message.contains("cap of 1"),
        "error must name the cap: {message}"
    );
    assert!(
        message.contains("narrow the workspace"),
        "error must name the remedy: {message}"
    );
}

#[test]
fn walk_reference_siblings_size_cap_skips_and_discloses() {
    let (temp_dir, anchor) = bash_workspace(&[("big.sh", "target target target\n")]);
    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 4)
        .expect("size cap overflow must skip, not fail");
    assert!(walk.siblings.is_empty());
    assert_eq!(walk.skipped.len(), 1);
    assert!(walk.skipped[0].file.ends_with("big.sh"));
    assert!(
        walk.skipped[0].reason.contains('4'),
        "reason must name the cap: {}",
        walk.skipped[0].reason
    );
}

#[test]
fn walk_reference_siblings_skips_hidden_dirs_and_excludes_anchor() {
    let (temp_dir, anchor) = bash_workspace(&[("visible.sh", "target\n")]);
    let hidden = temp_dir.path().join(".hidden");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(hidden.join("secret.sh"), "target\n").unwrap();

    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 1024).unwrap();
    let paths: Vec<std::path::PathBuf> = walk
        .siblings
        .iter()
        .map(|sibling| sibling.path().to_path_buf())
        .collect();
    // Anchor excluded (it is scanned separately), hidden dir not descended.
    assert_eq!(paths, vec![temp_dir.path().join("visible.sh")]);
    assert!(walk.skipped.is_empty());
}

#[test]
fn walk_reference_siblings_discloses_unwalkable_root() {
    let (temp_dir, anchor) = bash_workspace(&[]);
    let missing_root = temp_dir.path().join("does-not-exist");
    let walk = walk_reference_siblings(&missing_root, &anchor, Grammar::Bash, 100, 1024)
        .expect("an unwalkable root must skip-and-disclose, not hard-error");
    assert!(walk.siblings.is_empty());
    assert_eq!(
        walk.skipped.len(),
        1,
        "root failure must be disclosed, never silently empty"
    );
    assert!(
        walk.skipped[0].file.ends_with("does-not-exist"),
        "disclosure must name the walked root: {:?}",
        walk.skipped[0].file
    );
}

// === The range-level twin: spans for editing callers (rename) ===

#[test]
fn reference_ranges_across_files_cover_full_identifier_spans() {
    let anchor = typescript_file(
        "anchor.ts",
        "export function sharedFn() {\n    return 1;\n}\n",
    );
    let sibling = typescript_file(
        "sibling.ts",
        "import { sharedFn } from \"./anchor\";\nsharedFn();\n",
    );

    let ranges =
        execute_reference_ranges_for_symbol_across(&anchor, &[sibling], "sharedFn", 1, 17).unwrap();

    // Both files, and every span is exactly the identifier (8 chars), not the
    // enclosing call or declaration — an edit-level caller rewrites the span.
    let spans: Vec<(String, u32, u32, u32)> = ranges
        .iter()
        .map(|r| {
            (
                r.start().file().to_string_lossy().into_owned(),
                r.start().line(),
                r.start().column(),
                r.end().column() - r.start().column(),
            )
        })
        .collect();
    assert!(
        spans.iter().any(|(f, ..)| f.ends_with("anchor.ts")),
        "anchor sites must be present: {spans:?}"
    );
    assert!(
        spans.iter().any(|(f, ..)| f.ends_with("sibling.ts")),
        "sibling sites must be present: {spans:?}"
    );
    for (_, _, _, len) in &spans {
        assert_eq!(
            *len, 8,
            "every span must cover exactly 'sharedFn': {spans:?}"
        );
    }
}

#[test]
fn reference_ranges_preserve_anchor_validation_errors() {
    let anchor = typescript_file("decl.ts", "function sharedFn() {}\nsharedFn();\n");
    let sibling = typescript_file("user.ts", "sharedFn();\n");

    assert!(matches!(
        execute_reference_ranges_for_symbol_across(
            &anchor,
            std::slice::from_ref(&sibling),
            "sharedFn",
            99,
            99,
        ),
        Err(ParserError::InvalidPosition {
            line: 99,
            column: 99,
            ..
        })
    ));
    assert!(matches!(
        execute_reference_ranges_for_symbol_across(
            &anchor,
            std::slice::from_ref(&sibling),
            "missing",
            1,
            10,
        ),
        Err(ParserError::SymbolNotFound { name, .. }) if name == "missing"
    ));
}

#[cfg(unix)]
#[test]
fn walk_reference_siblings_discloses_unreadable_sibling() {
    // chmod 000 on an owned file denies the read even for the owner, so the
    // walk's "cannot load" arm fires where the stat succeeded — the skip is
    // disclosed with the reason, never silently dropped.
    let (temp_dir, anchor) = bash_workspace(&[("locked.sh", "target\n")]);
    let locked = temp_dir.path().join("locked.sh");
    let mut perms = std::fs::metadata(&locked).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o000);
    std::fs::set_permissions(&locked, perms).unwrap();

    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 1024)
        .expect("an unreadable sibling must skip-and-disclose, not fail");
    assert!(walk.siblings.is_empty());
    assert_eq!(walk.skipped.len(), 1);
    assert!(walk.skipped[0].file.ends_with("locked.sh"));
    assert!(
        walk.skipped[0].reason.contains("cannot load"),
        "reason must name the load failure: {}",
        walk.skipped[0].reason
    );
}

#[cfg(unix)]
#[test]
fn walk_reference_siblings_discloses_symlinked_file_and_directory() {
    let (temp_dir, anchor) = bash_workspace(&[("plain.sh", "target\n")]);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("real.sh"), "target\n").unwrap();
    std::fs::write(outside.path().join("notes.txt"), "target\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("real.sh"),
        temp_dir.path().join("link.sh"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), temp_dir.path().join("linkdir")).unwrap();
    // A symlink to another grammar's file is irrelevant to this walk.
    std::os::unix::fs::symlink(
        outside.path().join("notes.txt"),
        temp_dir.path().join("notes.txt"),
    )
    .unwrap();

    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 1024).unwrap();
    let siblings: Vec<std::path::PathBuf> = walk
        .siblings
        .iter()
        .map(|sibling| sibling.path().to_path_buf())
        .collect();
    assert_eq!(siblings, vec![temp_dir.path().join("plain.sh")]);
    let skipped: Vec<(std::path::PathBuf, String)> = walk
        .skipped
        .iter()
        .map(|skip| (skip.file.clone(), skip.reason.clone()))
        .collect();
    assert_eq!(
        skipped,
        vec![
            (
                temp_dir.path().join("link.sh"),
                format!(
                    "symlink not followed: {}",
                    temp_dir.path().join("link.sh").display()
                )
            ),
            (
                temp_dir.path().join("linkdir"),
                format!(
                    "symlink not followed: {}",
                    temp_dir.path().join("linkdir").display()
                )
            ),
        ]
    );
}

#[cfg(unix)]
#[test]
fn walk_reference_siblings_follows_a_symlinked_root() {
    // The walked ROOT itself may be spelled through a symlink (macOS
    // `/var` → `/private/var`); that is the workspace, not an omitted entry.
    let (temp_dir, anchor) = bash_workspace(&[("plain.sh", "target\n")]);
    let holder = tempfile::tempdir().unwrap();
    let root_link = holder.path().join("root");
    std::os::unix::fs::symlink(temp_dir.path(), &root_link).unwrap();
    let walk = walk_reference_siblings(&root_link, &anchor, Grammar::Bash, 100, 1024).unwrap();
    assert!(
        walk.skipped.is_empty(),
        "skipped: {:?}",
        walk.skipped.iter().map(|s| &s.file).collect::<Vec<_>>()
    );
    assert_eq!(walk.siblings.len(), 1);
}

/// Restores a directory's permissions on drop so the tempdir can clean up.
#[cfg(unix)]
struct RestoreMode(std::path::PathBuf);

#[cfg(unix)]
impl Drop for RestoreMode {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755))
        {
            eprintln!(
                "could not restore permissions on {}: {err}",
                self.0.display()
            );
        }
    }
}

#[cfg(unix)]
fn running_as_root() -> bool {
    // Mode 000 is still readable by root, so the unreadable case cannot be built.
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0")
        .unwrap_or(false)
}

#[cfg(unix)]
#[test]
fn walk_reference_siblings_discloses_symlink_with_unreadable_target() {
    use std::os::unix::fs::PermissionsExt;
    let (temp_dir, anchor) = bash_workspace(&[]);
    let outside = tempfile::tempdir().unwrap();
    let locked = outside.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(locked.join("real.sh"), "target\n").unwrap();
    // No grammar extension on the link name: only the target could tell.
    let link = temp_dir.path().join("data");
    std::os::unix::fs::symlink(locked.join("real.sh"), &link).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestoreMode(locked.clone());
    if running_as_root() {
        return;
    }

    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 1024).unwrap();
    assert!(walk.siblings.is_empty());
    assert_eq!(
        walk.skipped.len(),
        1,
        "skipped: {:?}",
        walk.skipped
            .iter()
            .map(|s| (&s.file, &s.reason))
            .collect::<Vec<_>>()
    );
    assert_eq!(walk.skipped[0].file, link);
    assert!(
        walk.skipped[0]
            .reason
            .starts_with("symlink target unreadable: "),
        "reason: {}",
        walk.skipped[0].reason
    );
}

#[cfg(unix)]
#[test]
fn walk_reference_siblings_discloses_dangling_symlink() {
    let (temp_dir, anchor) = bash_workspace(&[]);
    let link = temp_dir.path().join("stale");
    std::os::unix::fs::symlink(temp_dir.path().join("missing-target"), &link).unwrap();

    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 1024).unwrap();
    assert_eq!(walk.skipped.len(), 1);
    assert_eq!(walk.skipped[0].file, link);
    assert!(
        walk.skipped[0]
            .reason
            .starts_with("symlink target unreadable: "),
        "reason: {}",
        walk.skipped[0].reason
    );
}

#[cfg(unix)]
#[test]
fn walk_reference_siblings_omits_symlink_proven_irrelevant() {
    // Guard: stat succeeds, the target is a regular file, and the name is
    // not same-grammar — the only omitted shape.
    let (temp_dir, anchor) = bash_workspace(&[]);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("README"), "target\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("README"),
        temp_dir.path().join("README"),
    )
    .unwrap();

    let walk = walk_reference_siblings(temp_dir.path(), &anchor, Grammar::Bash, 100, 1024).unwrap();
    assert!(walk.siblings.is_empty());
    assert!(walk.skipped.is_empty());
}

/// A workspace whose root anchor is `anchor_name`, plus a regular
/// `sub/<anchor_name>` and (unix) a symlinked `sub2/<anchor_name>`.
fn filename_routed_workspace(
    anchor_name: &str,
    content: &str,
) -> (tempfile::TempDir, tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let anchor = temp_dir.path().join(anchor_name);
    std::fs::write(&anchor, content).unwrap();
    std::fs::create_dir(temp_dir.path().join("sub")).unwrap();
    std::fs::write(temp_dir.path().join("sub").join(anchor_name), content).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join(anchor_name), content).unwrap();
    #[cfg(unix)]
    {
        std::fs::create_dir(temp_dir.path().join("sub2")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join(anchor_name),
            temp_dir.path().join("sub2").join(anchor_name),
        )
        .unwrap();
    }
    (temp_dir, outside, anchor)
}

fn assert_filename_routed_walk(anchor_name: &str, content: &str, grammar: Grammar) {
    let (temp_dir, _outside, anchor) = filename_routed_workspace(anchor_name, content);
    assert_eq!(SourceFile::load(&anchor).unwrap().grammar(), grammar);
    let walk = walk_reference_siblings(temp_dir.path(), &anchor, grammar, 100, 1 << 20).unwrap();
    let siblings: Vec<std::path::PathBuf> = walk
        .siblings
        .iter()
        .map(|sibling| sibling.path().to_path_buf())
        .collect();
    assert_eq!(
        siblings,
        vec![temp_dir.path().join("sub").join(anchor_name)],
        "the regular filename-routed sibling must be walked"
    );
    #[cfg(unix)]
    {
        let link = temp_dir.path().join("sub2").join(anchor_name);
        let skipped: Vec<(std::path::PathBuf, String)> = walk
            .skipped
            .iter()
            .map(|skip| (skip.file.clone(), skip.reason.clone()))
            .collect();
        assert_eq!(
            skipped,
            vec![(
                link.clone(),
                format!("symlink not followed: {}", link.display())
            )],
            "the symlinked filename-routed sibling must be disclosed"
        );
    }
}

#[test]
fn walk_reference_siblings_routes_dockerfile_by_filename() {
    assert_filename_routed_walk(
        "Dockerfile",
        "FROM alpine\nRUN echo hi\n",
        Grammar::Dockerfile,
    );
}

#[test]
fn grammar_from_path_agrees_with_the_loader() {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in [
        ("CMakeLists.txt", "project(demo)\n"),
        ("Dockerfile", "FROM alpine\n"),
        ("BUILD", "cc_library(name = \"x\")\n"),
        ("q.sqlite.sql", "SELECT 1;\n"),
        ("main.rs", "fn main() {}\n"),
        ("notes.txt", "plain\n"),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let loaded = SourceFile::load(&path).ok().map(|source| source.grammar());
        assert_eq!(Grammar::from_path(&path), loaded, "{name}");
    }
}

fn hcl_lines(content: &str, name: &str, line: u32, column: u32) -> Vec<u32> {
    let source =
        SourceFile::from_content("main.tf", content.to_string(), Grammar::HclTerraform).unwrap();
    reference_lines(execute_references_for_symbol(&source, name, line, column).unwrap())
}

#[test]
fn hcl_references_exclude_resource_type_labels() {
    let content = "resource \"aws_s3_bucket\" \"b\" {}\nresource \"aws_s3_bucket\" \"c\" {}\n";
    assert_eq!(
        hcl_lines(content, "aws_s3_bucket", 1, 11),
        Vec::<u32>::new()
    );
}

#[test]
fn hcl_references_exclude_labels_of_blocks_that_declare_nothing() {
    let content = "variable \"region\" {}\nprovider \"region\" {}\n";
    assert_eq!(hcl_lines(content, "region", 1, 11), [1]);
}

#[test]
fn hcl_references_include_the_name_label_of_declaring_blocks() {
    let content = "resource \"t\" \"logs\" {}\ndata \"t\" \"logs\" {}\noutput \"logs\" {}\nmodule \"logs\" {}\n";
    assert_eq!(hcl_lines(content, "logs", 1, 16), [1, 2, 3, 4]);
}

#[test]
fn sql_references_exclude_the_schema_of_a_declared_name() {
    let content =
        "CREATE TABLE app.users (id INT);\nCREATE VIEW app.v AS SELECT id FROM app.users;\n";
    let source = SourceFile::from_content("a.sql", content.to_string(), Grammar::Sql).unwrap();
    let found = reference_tuples(execute_references_for_symbol(&source, "app", 1, 14).unwrap());
    let positions: Vec<(u32, u32)> = found.iter().map(|(_, l, c)| (*l, *c)).collect();
    assert!(!positions.contains(&(1, 14)), "{positions:?}");
    assert!(!positions.contains(&(2, 13)), "{positions:?}");
    let found = reference_tuples(execute_references_for_symbol(&source, "users", 1, 18).unwrap());
    let positions: Vec<(u32, u32)> = found.iter().map(|(_, l, c)| (*l, *c)).collect();
    assert!(positions.contains(&(1, 18)), "{positions:?}");
}

fn reference_sites(
    grammar: Grammar,
    path: &str,
    content: &str,
    name: &str,
    line: u32,
    column: u32,
) -> Vec<(u32, u32)> {
    let source = SourceFile::from_content(path, content.to_string(), grammar).unwrap();
    reference_tuples(execute_references_for_symbol(&source, name, line, column).unwrap())
        .into_iter()
        .map(|(_, line, column)| (line, column))
        .collect()
}

#[test]
fn references_include_more_declaration_kinds() {
    // Swift protocol: declaring name and the conformance; a same-name let is not a site.
    let swift = "protocol Shape {}\nclass Circle: Shape {}\nlet Shape = 1\n";
    assert_eq!(
        reference_sites(Grammar::Swift, "a.swift", swift, "Shape", 1, 10),
        [(1, 10), (2, 15)]
    );

    // Java @interface: declaring name and the annotation use; a same-name field is not a site.
    let java = "@interface Marker {}\n@Marker class A {}\nclass B { int Marker; }\n";
    assert_eq!(
        reference_sites(Grammar::Java, "A.java", java, "Marker", 1, 12),
        [(1, 12), (2, 2)]
    );

    // PHP enum: declaring name and the parameter type; a same-name variable is not a site.
    let php = "<?php\nenum Suit {}\nfunction f(Suit $s) {}\n$Suit = 1;\n";
    assert_eq!(
        reference_sites(Grammar::Php, "a.php", php, "Suit", 2, 6),
        [(2, 6), (3, 12)]
    );

    // C# constructor: the class name and the constructor name; a same-name parameter is not a site.
    let ctor = "class Box {\n    public Box() {}\n    void M(int Box) {}\n}\n";
    assert_eq!(
        reference_sites(Grammar::CSharp, "a.cs", ctor, "Box", 1, 7),
        [(1, 7), (2, 12)]
    );

    // C# delegate: declaring name; a same-name field is not a site.
    let delegate = "delegate void Handler(int x);\nclass C { int Handler; }\n";
    assert_eq!(
        reference_sites(Grammar::CSharp, "b.cs", delegate, "Handler", 1, 15),
        [(1, 15)]
    );

    // Scala val: declaring name; a same-name parameter is not a site.
    let scala = "val answer = 1\ndef g(answer: Int) = 0\n";
    assert_eq!(
        reference_sites(Grammar::Scala, "a.scala", scala, "answer", 1, 5),
        [(1, 5)]
    );
}

#[test]
fn php_function_references_exclude_same_name_variable() {
    let content =
        "<?php\nfunction add($a) { return $a; }\n$add = 1;\necho $add;\n$x = $add;\nadd(1);\n";
    assert_eq!(
        reference_sites(Grammar::Php, "a.php", content, "add", 2, 10),
        [(2, 10), (6, 1)]
    );
}
