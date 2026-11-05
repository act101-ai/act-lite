//! Workspace management and file tracking.

use crate::{ParserError, SourceFile, error::Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Workspace representing the project root.
///
/// Tracks open files, manages file state, and coordinates parsing.
pub struct Workspace {
    root: PathBuf,
    files: HashMap<PathBuf, SourceFile>,
}

impl Workspace {
    /// Creates a new workspace at the given root directory.
    ///
    /// Returns an error if the root doesn't exist or isn't a directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();

        if !root.exists() {
            return Err(ParserError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("workspace root not found: {}", root.display()),
            )));
        }

        if !root.is_dir() {
            return Err(ParserError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("workspace root is not a directory: {}", root.display()),
            )));
        }

        let root = if root.is_absolute() {
            root.to_path_buf()
        } else {
            let joined = std::env::current_dir().map_err(ParserError::Io)?.join(root);
            joined
                .canonicalize()
                .unwrap_or_else(|_| collapse_lexically(&joined))
        };

        Ok(Self {
            root,
            files: HashMap::new(),
        })
    }

    /// Returns the workspace root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Converts a path to absolute, normalizing it relative to the workspace root.
    /// Handles both absolute and relative paths correctly.
    fn normalize_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else if path.components().next() == Some(std::path::Component::ParentDir) {
            std::env::current_dir().unwrap_or_default().join(path)
        } else {
            self.root.join(path)
        };
        collapse_lexically(&joined)
    }

    /// Validates that a file path is within the workspace root directory.
    /// This ensures that Lua files and all files are correctly recognized as part of the workspace.
    ///
    /// Returns true if the normalized path is within the workspace root.
    fn is_file_in_workspace(&self, abs_path: &Path) -> bool {
        // Use canonicalize for accurate path comparison, but fall back to path comparison
        // if canonicalization fails (e.g., for non-existent files)
        match (abs_path.canonicalize(), self.root.canonicalize()) {
            (Ok(file), Ok(root)) => {
                // Check if file path starts with workspace root path
                file.starts_with(&root)
            }
            _ => {
                // Fallback: compare paths directly using component analysis
                abs_path.strip_prefix(&self.root).is_ok()
            }
        }
    }

    /// Opens and parses a file, adding it to the workspace.
    ///
    /// If the file is already open, returns the existing `SourceFile`.
    /// The file must be within the workspace root directory.
    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<&SourceFile> {
        let abs_path = self.normalize_path(path);

        // Validate that the file is within the workspace
        if !self.is_file_in_workspace(&abs_path) {
            return Err(ParserError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "file is outside workspace: {} (workspace: {})",
                    abs_path.display(),
                    self.root.display()
                ),
            )));
        }

        if !self.files.contains_key(&abs_path) {
            let source = SourceFile::load(&abs_path)?;
            self.files.insert(abs_path.clone(), source);
        }

        let source = self
            .files
            .get(&abs_path)
            .expect("file just inserted or already present");
        act_lite_core::touch::record(
            &abs_path,
            source.content().len() as u64,
            source.grammar().name(),
        );
        Ok(source)
    }

    fn lookup_key(&self, path: &Path) -> Option<PathBuf> {
        let cwd_anchored = self.normalize_path(path);
        if path.components().next() != Some(std::path::Component::ParentDir) {
            return Some(cwd_anchored);
        }
        let root_anchored = collapse_lexically(&self.root.join(path));
        if root_anchored != cwd_anchored
            && self.files.contains_key(&root_anchored)
            && self.files.contains_key(&cwd_anchored)
        {
            return None;
        }
        Some(cwd_anchored)
    }

    /// Returns a reference to an open file.
    ///
    /// Returns `None` if the file is not open.
    pub fn get_file(&self, path: impl AsRef<Path>) -> Option<&SourceFile> {
        let abs_path = self.lookup_key(path.as_ref())?;
        let file = self.files.get(&abs_path);
        if let Some(sf) = file {
            act_lite_core::touch::record(&abs_path, sf.content().len() as u64, sf.grammar().name());
        }
        file
    }

    pub fn resolved_path(&self, path: impl AsRef<Path>) -> Option<std::path::PathBuf> {
        let abs_path = self.lookup_key(path.as_ref())?;
        self.files.contains_key(&abs_path).then_some(abs_path)
    }

    /// Returns a mutable reference to an open file.
    ///
    /// Returns `None` if the file is not open.
    pub fn get_file_mut(&mut self, path: impl AsRef<Path>) -> Option<&mut SourceFile> {
        let abs_path = self.normalize_path(path);
        // Record from an immutable probe first (size before mutation is the
        // naive read cost), then hand back the mutable reference.
        if let Some(sf) = self.files.get(&abs_path) {
            act_lite_core::touch::record(&abs_path, sf.content().len() as u64, sf.grammar().name());
        }
        self.files.get_mut(&abs_path)
    }

    /// Closes a file, removing it from the workspace.
    pub fn close_file(&mut self, path: impl AsRef<Path>) -> bool {
        let Some(abs_path) = self.lookup_key(path.as_ref()) else {
            return false;
        };
        self.files.remove(&abs_path).is_some()
    }

    /// Returns the number of open files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Returns an iterator over all open file paths.
    pub fn open_files(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }

    /// Returns an iterator over all open files.
    pub fn files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.values()
    }
}

fn collapse_lexically(joined: &Path) -> PathBuf {
    let mut normalized = std::path::PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            other => normalized.push(other),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_workspace() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();
        assert_eq!(workspace.root(), dir.path());
        assert_eq!(workspace.file_count(), 0);
    }

    #[test]
    fn touch_open_file_records_path_bytes_and_grammar() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.ts");
        std::fs::write(&file, "export const x = 1;\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();

        let scope = act_lite_core::touch::TouchScope::enter();
        let expected_grammar = {
            let sf = ws.open_file(&file).unwrap();
            sf.grammar().name().to_string()
        };
        let set = scope.finish();

        assert_eq!(set.len(), 1);
        assert_eq!(set.naive_bytes(), 20); // exact content length
        assert_eq!(
            set.grammar_bytes().get(&expected_grammar),
            Some(&20),
            "touch must bin bytes under the file's grammar name"
        );
    }

    #[test]
    fn touch_get_file_and_get_file_mut_record_on_hit_only() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.ts");
        std::fs::write(&file, "export const x = 1;\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();
        ws.open_file(&file).unwrap(); // open OUTSIDE any scope

        let scope = act_lite_core::touch::TouchScope::enter();
        assert!(ws.get_file(&file).is_some());
        assert!(ws.get_file_mut(&file).is_some());
        assert!(ws.get_file(dir.path().join("missing.ts")).is_none()); // miss: no record
        let set = scope.finish();

        assert_eq!(set.len(), 1, "hit recorded once (dedup), miss not recorded");
    }

    #[test]
    fn touch_unscoped_workspace_access_records_nothing_and_does_not_panic() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.ts");
        std::fs::write(&file, "export const x = 1;\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();
        ws.open_file(&file).unwrap(); // no scope active: must simply work
        let scope = act_lite_core::touch::TouchScope::enter();
        let set = scope.finish();
        assert!(set.is_empty());
    }

    #[test]
    fn new_workspace_not_found() {
        let result = Workspace::new("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn open_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        let source = workspace.open_file("test.ts").unwrap();
        assert_eq!(source.content(), "const x = 1;");
        assert_eq!(workspace.file_count(), 1);
    }

    #[test]
    fn open_file_twice() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("test.ts").unwrap();
        workspace.open_file("test.ts").unwrap();
        assert_eq!(workspace.file_count(), 1); // Should not duplicate
    }

    #[test]
    fn get_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("test.ts").unwrap();

        let source = workspace.get_file("test.ts").unwrap();
        assert_eq!(source.content(), "const x = 1;");
    }

    #[test]
    fn close_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("test.ts").unwrap();
        assert_eq!(workspace.file_count(), 1);

        let closed = workspace.close_file("test.ts");
        assert!(closed);
        assert_eq!(workspace.file_count(), 0);
    }

    #[test]
    fn workspace_root_is_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("not_a_dir.txt");
        std::fs::write(&file_path, "content").unwrap();

        let result = Workspace::new(&file_path);
        assert!(result.is_err());
        // Check error message contains expected text
        let err_str = format!("{}", result.err().unwrap());
        assert!(err_str.contains("not a directory"));
    }

    #[test]
    fn get_file_mut() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("test.ts").unwrap();

        let source = workspace.get_file_mut("test.ts").unwrap();
        assert_eq!(source.content(), "const x = 1;");
    }

    #[test]
    fn get_file_mut_not_open() {
        let dir = TempDir::new().unwrap();
        let mut workspace = Workspace::new(dir.path()).unwrap();

        let result = workspace.get_file_mut("nonexistent.ts");
        assert!(result.is_none());
    }

    #[test]
    fn open_file_absolute_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        let source = workspace.open_file(&file_path).unwrap(); // Absolute path
        assert_eq!(source.content(), "const x = 1;");
    }

    #[test]
    fn get_file_absolute_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file(&file_path).unwrap();

        // Get using absolute path
        let source = workspace.get_file(&file_path).unwrap();
        assert_eq!(source.content(), "const x = 1;");
    }

    #[test]
    fn get_file_mut_absolute_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file(&file_path).unwrap();

        // Get mut using absolute path
        let source = workspace.get_file_mut(&file_path).unwrap();
        assert_eq!(source.content(), "const x = 1;");
    }

    #[test]
    fn close_file_absolute_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file(&file_path).unwrap();
        assert_eq!(workspace.file_count(), 1);

        // Close using absolute path
        let closed = workspace.close_file(&file_path);
        assert!(closed);
        assert_eq!(workspace.file_count(), 0);
    }

    #[test]
    fn close_file_not_open() {
        let dir = TempDir::new().unwrap();
        let mut workspace = Workspace::new(dir.path()).unwrap();

        let closed = workspace.close_file("nonexistent.ts");
        assert!(!closed);
    }

    #[test]
    fn get_file_not_open() {
        let dir = TempDir::new().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();

        let result = workspace.get_file("nonexistent.ts");
        assert!(result.is_none());
    }

    #[test]
    fn open_files_iterator() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.ts"), "const a = 1;").unwrap();
        std::fs::write(dir.path().join("b.ts"), "const b = 2;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("a.ts").unwrap();
        workspace.open_file("b.ts").unwrap();

        let paths: Vec<_> = workspace.open_files().collect();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn files_iterator() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.ts"), "const a = 1;").unwrap();
        std::fs::write(dir.path().join("b.ts"), "const b = 2;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("a.ts").unwrap();
        workspace.open_file("b.ts").unwrap();

        let files: Vec<_> = workspace.files().collect();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn open_file_not_found() {
        let dir = TempDir::new().unwrap();
        let mut workspace = Workspace::new(dir.path()).unwrap();

        let result = workspace.open_file("nonexistent.ts");
        assert!(result.is_err());
    }

    #[test]
    fn workspace_with_multiple_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("one.ts"), "const one = 1;").unwrap();
        std::fs::write(dir.path().join("two.ts"), "const two = 2;").unwrap();
        std::fs::write(dir.path().join("three.ts"), "const three = 3;").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();
        workspace.open_file("one.ts").unwrap();
        workspace.open_file("two.ts").unwrap();
        workspace.open_file("three.ts").unwrap();

        assert_eq!(workspace.file_count(), 3);

        // Close one
        workspace.close_file("two.ts");
        assert_eq!(workspace.file_count(), 2);

        // Verify remaining files
        assert!(workspace.get_file("one.ts").is_some());
        assert!(workspace.get_file("two.ts").is_none());
        assert!(workspace.get_file("three.ts").is_some());
    }

    #[test]
    fn file_outside_workspace_rejected() {
        let workspace_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("outside.ts");
        std::fs::write(&outside_file, "const x = 1;").unwrap();

        let mut workspace = Workspace::new(workspace_dir.path()).unwrap();

        // Try to open a file outside the workspace using absolute path
        let result = workspace.open_file(&outside_file);
        assert!(result.is_err());
        assert_eq!(workspace.file_count(), 0); // File should not be added
    }

    #[test]
    fn lua_files_in_workspace_recognized() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.lua"), "print('hello')").unwrap();
        std::fs::write(dir.path().join("module.lua"), "local M = {}").unwrap();

        let mut workspace = Workspace::new(dir.path()).unwrap();

        // Lua files should be recognized and opened correctly
        let main = workspace.open_file("main.lua").unwrap();
        assert_eq!(main.content(), "print('hello')");

        let module = workspace.open_file("module.lua").unwrap();
        assert_eq!(module.content(), "local M = {}");

        assert_eq!(workspace.file_count(), 2);

        // Files should be retrievable
        assert!(workspace.get_file("main.lua").is_some());
        assert!(workspace.get_file("module.lua").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_dotdot_root_guards_the_resolved_directory() {
        let cwd = std::env::current_dir().unwrap();
        let holder = cwd
            .parent()
            .expect("test cwd is one level deep")
            .to_path_buf();
        let t = tempfile::tempdir_in(&holder).unwrap();
        let external = t.path().join("external");
        std::fs::create_dir_all(external.join("sub")).unwrap();
        std::fs::write(external.join("a.ts"), "export const x = 1;\n").unwrap();
        std::fs::write(t.path().join("sibling.ts"), "export const y = 2;\n").unwrap();
        std::os::unix::fs::symlink(external.join("sub"), t.path().join("link")).unwrap();

        // The CWD-relative spelling of `T/link/..`.
        let mut tcomp = t.path().components();
        let mut ccomp = cwd.components();
        loop {
            match (tcomp.clone().next(), ccomp.clone().next()) {
                (Some(a), Some(b)) if a == b => {
                    tcomp.next();
                    ccomp.next();
                }
                _ => break,
            }
        }
        let mut root_spelling = PathBuf::new();
        for _ in 0..ccomp.count() {
            root_spelling.push("..");
        }
        for comp in tcomp {
            root_spelling.push(comp);
        }
        root_spelling.push("link");
        root_spelling.push("..");

        let mut ws = Workspace::new(&root_spelling).expect("the root exists through the symlink");
        ws.open_file("a.ts")
            .expect("the root is the RESOLVED directory: external/a.ts opens");
        let err = match ws.open_file(t.path().join("sibling.ts")) {
            Ok(_) => panic!("a file outside the resolved root refuses containment"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("outside workspace"),
            "the refusal names containment: {err}"
        );
    }
}
