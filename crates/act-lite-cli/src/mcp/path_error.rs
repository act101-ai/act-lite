//! Smart path-error enrichment for MCP tool error messages.
//!
//! When an agent calls a tool with a path that does not exist, instead of
//! returning a bare "not found" error we walk up to the nearest ancestor
//! directory that exists within the workspace and list its siblings, giving
//! the agent the context it needs to correct the path.

use std::path::{Path, PathBuf};

/// Walk up `path` until a directory that exists inside `workspace_root` is found,
/// and return its canonical path.
///
/// Returns `None` if no ancestor within the workspace root exists.
pub(crate) fn find_nearest_existing_ancestor(
    path: &Path,
    workspace_root: &Path,
) -> Option<PathBuf> {
    let canonical_root = workspace_root.canonicalize().ok()?;

    let mut current = path.to_path_buf();
    loop {
        if current.is_dir() {
            // `..` and symlinks can lead a lexically inside path outside.
            let canonical = current.canonicalize().ok()?;
            return canonical.starts_with(&canonical_root).then_some(canonical);
        }
        current = current.parent()?.to_path_buf();
    }
}

/// Build a multi-line enriched error message for a missing path.
///
/// * `user_input`      — the raw string the caller provided (shown in the first line).
/// * `resolved_path`   — the absolute path after resolution.
/// * `workspace_root`  — used for relative display and ancestor walk.
pub(crate) fn enrich_not_found_error(
    user_input: &str,
    resolved_path: &Path,
    workspace_root: &Path,
) -> String {
    let header = format!("The path '{}' does not exist.", user_input);

    let Some(deepest_dir) = find_nearest_existing_ancestor(resolved_path, workspace_root) else {
        return format!(
            "{}\n\nNo ancestor directory could be found within the workspace.",
            header
        );
    };

    // Compare and list on canonical paths: `deepest_dir` is canonical and inside
    // `root`, so neither it nor its parent is outside the workspace.
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let nearest_display = deepest_dir
        .strip_prefix(&root)
        .unwrap_or(&deepest_dir)
        .display()
        .to_string();

    // Choose what to list:
    // - If the deepest existing dir IS the direct parent of the missing path
    //   (i.e. the containing directory exists but only the final name is wrong),
    //   list that directory's contents so the agent can spot the correct filename.
    // - Otherwise the agent mis-rooted further up, so list deepest_dir's parent
    //   to show siblings of the closest existing anchor.
    // - If deepest_dir IS the workspace root, just list the root's contents.
    let direct_parent = resolved_path.parent().and_then(|p| p.canonicalize().ok());
    let listing_dir = if direct_parent.as_deref() == Some(deepest_dir.as_path()) {
        // Containing dir exists — show its contents (correct dir, wrong filename)
        deepest_dir.clone()
    } else if deepest_dir == root {
        root.clone()
    } else {
        deepest_dir.parent().unwrap_or(&root).to_path_buf()
    };

    let mut entries: Vec<String> = match std::fs::read_dir(&listing_dir) {
        Ok(iter) => iter
            .flatten()
            .map(|e| {
                e.path()
                    .strip_prefix(&root)
                    .unwrap_or(&e.path())
                    .display()
                    .to_string()
            })
            .collect(),
        Err(_) => vec![],
    };
    entries.sort();

    if entries.is_empty() {
        format!(
            "{}\n\nNearest root: {}\n\n(directory is empty)",
            header, nearest_display
        )
    } else {
        format!(
            "{}\n\nNearest root: {}\n\n{}",
            header,
            nearest_display,
            entries.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_dir(root: &Path, rel: &str) -> PathBuf {
        let p = root.join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn find_ancestor_walks_up_to_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();
        make_dir(&ws, "a/b");
        // a/b exists; a/b/c/d does not
        let result = find_nearest_existing_ancestor(&ws.join("a/b/c/d"), &ws);
        assert_eq!(result, Some(ws.join("a/b").canonicalize().unwrap()));
    }

    #[test]
    fn find_ancestor_stops_at_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("project");
        fs::create_dir_all(&ws).unwrap();
        // Use a completely separate path that does NOT start with ws to verify
        // the ancestor walk stops at the workspace boundary.
        // /tmp itself exists but is outside ws, so should return None.
        let outside = Path::new("/tmp");
        let result = find_nearest_existing_ancestor(outside, &ws);
        assert_eq!(result, None);
    }

    #[test]
    fn find_ancestor_workspace_root_missing() {
        let result = find_nearest_existing_ancestor(
            Path::new("/nonexistent/workspace/src/foo.rs"),
            Path::new("/nonexistent/workspace"),
        );
        assert_eq!(result, None);
    }

    #[test]
    fn find_ancestor_path_is_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();
        let result = find_nearest_existing_ancestor(&ws, &ws);
        assert_eq!(result, Some(ws.canonicalize().unwrap()));
    }

    #[test]
    fn enrich_lists_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();
        make_dir(&ws, "src/foo");
        make_dir(&ws, "src/bar");
        make_dir(&ws, "src/baz");
        // asking for src/foo/nonexistent/file.rs — deepest existing is src/foo, listing src/
        let msg = enrich_not_found_error(
            "src/foo/nonexistent/file.rs",
            &ws.join("src/foo/nonexistent/file.rs"),
            &ws,
        );
        assert!(
            msg.contains("Nearest root:"),
            "missing Nearest root: in\n{msg}"
        );
        assert!(
            msg.contains("src/bar") || msg.contains("bar"),
            "siblings not listed in\n{msg}"
        );
        assert!(
            msg.contains("src/baz") || msg.contains("baz"),
            "siblings not listed in\n{msg}"
        );
    }

    #[test]
    fn enrich_no_ancestor_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        // path outside workspace entirely
        let msg = enrich_not_found_error(
            "/tmp/outside/file.rs",
            Path::new("/tmp/outside/file.rs"),
            &ws,
        );
        assert!(
            msg.contains("No ancestor directory"),
            "expected fallback message, got:\n{msg}"
        );
    }

    #[test]
    fn enrich_deepest_is_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();
        make_dir(&ws, "alpha");
        make_dir(&ws, "beta");
        // path under ws root, nothing inside exists
        let msg =
            enrich_not_found_error("nonexistent/file.rs", &ws.join("nonexistent/file.rs"), &ws);
        assert!(
            msg.contains("Nearest root:"),
            "missing Nearest root: in\n{msg}"
        );
        assert!(
            msg.contains("alpha") || msg.contains("beta"),
            "workspace children not listed in\n{msg}"
        );
    }

    #[test]
    fn enrich_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();
        // Workspace root has NO children. Ask for a nonexistent path directly
        // under the root so deepest_dir == ws and listing_dir == ws (empty).
        let msg =
            enrich_not_found_error("nonexistent/file.rs", &ws.join("nonexistent/file.rs"), &ws);
        assert!(
            msg.contains("(directory is empty)"),
            "expected empty dir message, got:\n{msg}"
        );
    }

    #[test]
    fn enrich_correct_dir_wrong_filename_lists_dir_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();
        make_dir(&ws, "src");
        // Create real files in src/ so the listing is non-empty.
        fs::write(ws.join("src/existing.rs"), "fn foo() {}").unwrap();
        fs::write(ws.join("src/another.rs"), "fn bar() {}").unwrap();
        // Ask for a nonexistent filename in an existing directory.
        // direct parent = src (exists) = deepest_dir, so we list src/ contents.
        let msg = enrich_not_found_error("src/wrong_name.rs", &ws.join("src/wrong_name.rs"), &ws);
        assert!(
            msg.contains("Nearest root:"),
            "missing Nearest root: in\n{msg}"
        );
        assert!(
            msg.contains("src/existing.rs") || msg.contains("existing.rs"),
            "should list existing.rs in\n{msg}"
        );
        assert!(
            msg.contains("src/another.rs") || msg.contains("another.rs"),
            "should list another.rs in\n{msg}"
        );
    }
}
