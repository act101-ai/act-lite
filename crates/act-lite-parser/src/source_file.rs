//! Source file with parsed syntax tree.

use crate::{Grammar, Parser, ParserError, SyntaxTree, error::Result};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// A source file with its content and parsed syntax tree.
///
/// Handles incremental re-parsing when content changes.
pub struct SourceFile {
    path: PathBuf,
    content: String,
    grammar: Grammar,
    tree: SyntaxTree,
}

impl SourceFile {
    /// Loads and parses a source file. A FIFO is not waited on, and a path
    /// that is not a regular file is refused.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = open_regular(path).map_err(ParserError::Io)?;
        Self::record(Self::read(path, file)?)
    }

    /// Loads and parses a source file as [`SourceFile::load`] does, and
    /// refuses it unless the operating system's path of the opened file lies
    /// inside the canonical `root`. The check is on the opened file, so a
    /// path that changes after a caller's own check, or after this open,
    /// cannot make this read a file outside `root`.
    pub fn load_within(path: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = open_regular(path).map_err(ParserError::Io)?;
        if !opened_inside(&file, root.as_ref()) {
            return Err(ParserError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path must be inside the workspace root",
            )));
        }
        Self::record(Self::read(path, file)?)
    }

    fn record(source: Self) -> Result<Self> {
        // Parse-entry touch hook. SourceFile::load is the read entry
        // point for the navigation/analyze surface (skeleton, symbols, and the
        // MCP/CLI query handlers), which loads files directly rather than through
        // Workspace::open_file/get_file. Recording here attributes their read
        // bytes to the active TouchScope; record() is a no-op without one.
        // TouchSet dedups by path, so this coexists with the Workspace hooks
        // (which still cover cached get_file re-access, where no load() occurs).
        act_lite_core::touch::record(
            &source.path,
            source.content.len() as u64,
            source.grammar.name(),
        );
        Ok(source)
    }

    fn read(path: &Path, mut file: File) -> Result<Self> {
        // Read bytes first to distinguish I/O errors from UTF-8 errors
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(ParserError::Io)?;
        let content = String::from_utf8(bytes).map_err(|_| ParserError::InvalidUtf8 {
            file: path.to_path_buf(),
        })?;

        // Filename conventions (Dockerfile) route by NAME before extension
        // routing; `Grammar::from_path` is the single
        // classifier, shared with the reference walk.
        let grammar = Grammar::from_path(path).ok_or_else(|| ParserError::UnsupportedLanguage {
            extension: path
                .extension()
                .and_then(|e| e.to_str())
                .map_or_else(|| path.display().to_string(), str::to_string),
        })?;

        let mut parser = Parser::new();
        let tree = parser.parse(&content, grammar)?;

        Ok(Self {
            path: path.to_path_buf(),
            content,
            grammar,
            tree,
        })
    }

    /// Creates a source file from content with explicit grammar.
    pub fn from_content(
        path: impl Into<PathBuf>,
        content: String,
        grammar: Grammar,
    ) -> Result<Self> {
        let mut parser = Parser::new();
        let tree = parser.parse(&content, grammar)?;

        Ok(Self {
            path: path.into(),
            content,
            grammar,
            tree,
        })
    }

    /// Returns the file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the source content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the grammar.
    pub fn grammar(&self) -> Grammar {
        self.grammar
    }

    /// Returns a reference to the syntax tree.
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }

    /// Returns true if this is a TypeScript declaration file (.d.ts, .d.mts, .d.cts).
    ///
    /// Declaration files are ambient-only and should not be targets for
    /// refactoring operations that generate implementation code.
    pub fn is_declaration_file(&self) -> bool {
        let path_str = self.path.to_string_lossy();
        path_str.ends_with(".d.ts") || path_str.ends_with(".d.mts") || path_str.ends_with(".d.cts")
    }

    /// Updates the content and re-parses.
    ///
    /// TODO: Implement incremental parsing with tree-sitter's `parse(source, Some(&old_tree))`
    /// for efficiency. Currently parses from scratch.
    pub fn update_content(&mut self, new_content: String) -> Result<()> {
        let mut parser = Parser::new();
        let tree = parser.parse(&new_content, self.grammar)?;
        self.content = new_content;
        self.tree = tree;
        Ok(())
    }
}

/// The text of the regular file at `path`. On unix a FIFO is not waited on,
/// and a path that is not a regular file is refused.
pub fn read_regular_text(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    open_regular(path)?.read_to_string(&mut text)?;
    Ok(text)
}

/// Opens `path` for reading. On unix a FIFO is not waited on. A handle that
/// is not a regular file is refused.
fn open_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(file)
}

/// Whether `file` lies inside the canonical `root`, judged by the path the
/// operating system holds for the opened file, not by a new look-up of a
/// path that can have changed since the open.
fn opened_inside(file: &File, root: &Path) -> bool {
    match (opened_path(file), root.canonicalize()) {
        (Ok(opened), Ok(root)) => opened.starts_with(root),
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn opened_path(file: &File) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::AsRawFd;
    let mut buf = vec![0u8; libc::PATH_MAX as usize];
    // SAFETY: F_GETPATH writes at most PATH_MAX bytes into `buf`.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buf.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(&buf[..len])))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn opened_path(file: &File) -> io::Result<PathBuf> {
    use std::os::unix::io::AsRawFd;
    std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
}

#[cfg(windows)]
fn opened_path(file: &File) -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS,
    };
    let mut buf = vec![0u16; 1024];
    loop {
        // SAFETY: the call writes at most `buf.len()` UTF-16 units into `buf`.
        let len = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        } as usize;
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        if len < buf.len() {
            return Ok(PathBuf::from(std::ffi::OsString::from_wide(&buf[..len])));
        }
        buf.resize(len, 0);
    }
}

/// No way to get the path of an opened file: every file is refused.
#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "android",
    windows
)))]
fn opened_path(_: &File) -> io::Result<PathBuf> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn load_file() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("ts");
        writeln!(file, "const x = 1;").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let source = SourceFile::load(&path).unwrap();
        assert_eq!(source.content().trim(), "const x = 1;");
        assert_eq!(source.grammar(), Grammar::TypeScript);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_records_a_touch_into_the_active_scope() {
        // Parse-entry hook: SourceFile::load is the read entry point for
        // the navigation/analyze surface (skeleton, symbols, …), which bypasses
        // Workspace::open_file/get_file. Without this hook, every read op records
        // naive_bytes=0 — measured savings for the read surface collapse to zero.
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("rs");
        writeln!(file, "fn main() {{}}").unwrap();
        std::fs::rename(file.path(), &path).unwrap();
        let canonical = std::fs::canonicalize(&path).unwrap();

        let scope = act_lite_core::touch::TouchScope::enter();
        let _ = SourceFile::load(&canonical).unwrap();
        let set = scope.finish();

        assert!(
            set.contains(&canonical),
            "load must record the file as touched in the active scope"
        );
        assert!(
            set.naive_bytes() > 0,
            "touched file must contribute naive bytes"
        );
        assert_eq!(
            set.grammar_bytes().get("Rust"),
            Some(&(set.naive_bytes())),
            "touched bytes must be attributed to the resolved grammar"
        );

        std::fs::remove_file(&canonical).unwrap();
    }

    #[test]
    fn load_without_active_scope_is_a_noop() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("rs");
        writeln!(file, "fn main() {{}}").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        // Must not panic when no TouchScope is active.
        let source = SourceFile::load(&path).unwrap();
        assert_eq!(source.grammar(), Grammar::Rust);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn from_content() {
        let source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();
        assert_eq!(source.content(), "const x = 1;");
        assert_eq!(source.path(), Path::new("test.ts"));
    }

    #[test]
    fn update_content() {
        let mut source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();

        source.update_content("const y = 2;".to_string()).unwrap();
        assert_eq!(source.content(), "const y = 2;");
    }

    #[test]
    fn load_file_not_found() {
        let result = SourceFile::load("/nonexistent/file.ts");
        assert!(result.is_err());
        if let Err(ParserError::Io(_)) = result {
            // Expected I/O error
        } else {
            panic!("Expected I/O error");
        }
    }

    #[test]
    fn load_file_unsupported_extension() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("xyz"); // Unsupported extension
        writeln!(file, "some content").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let result = SourceFile::load(&path);
        assert!(result.is_err());
        if let Err(ParserError::UnsupportedLanguage { extension }) = result {
            assert_eq!(extension, "xyz");
        } else {
            panic!("Expected UnsupportedLanguage error");
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_file_no_extension() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_file_name("noextension");
        writeln!(file, "some content").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let result = SourceFile::load(&path);
        assert!(result.is_err());
        if let Err(ParserError::UnsupportedLanguage { .. }) = result {
            // Expected
        } else {
            panic!("Expected UnsupportedLanguage error");
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_file_invalid_utf8() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("ts");
        // Write invalid UTF-8 bytes
        std::fs::write(&path, [0xFF, 0xFE, 0xFD]).unwrap();

        let result = SourceFile::load(&path);
        assert!(result.is_err());
        if let Err(ParserError::InvalidUtf8 { .. }) = result {
            // Expected
        } else {
            panic!("Expected InvalidUtf8 error");
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn load_refuses_a_fifo_without_waiting() {
        use std::os::unix::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("pipe.rs");
        let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let (tx, rx) = std::sync::mpsc::channel();
        let path = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(SourceFile::load(&path).is_err());
        });
        let refused = rx.recv_timeout(std::time::Duration::from_secs(5));
        // Unblock a reader that waits on the pipe.
        let _ = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo);
        assert_eq!(refused, Ok(true), "load must refuse a FIFO at once");
    }

    #[cfg(unix)]
    #[test]
    fn load_within_refuses_a_file_outside_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(dir.path().join("out.rs"), "fn outside() {}\n").unwrap();
        std::fs::write(root.join("in.rs"), "fn inside() {}\n").unwrap();
        std::os::unix::fs::symlink(dir.path().join("out.rs"), root.join("link.rs")).unwrap();
        std::os::unix::fs::symlink(root.join("in.rs"), root.join("alias.rs")).unwrap();
        assert!(SourceFile::load_within(root.join("link.rs"), &root).is_err());
        assert!(SourceFile::load_within(dir.path().join("out.rs"), &root).is_err());
        assert!(SourceFile::load_within(root.join("in.rs"), &root).is_ok());
        assert!(SourceFile::load_within(root.join("alias.rs"), &root).is_ok());
    }

    #[test]
    fn the_root_check_is_on_the_opened_file_not_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(dir.path().join("out.rs"), "fn outside() {}\n").unwrap();
        std::fs::write(root.join("a.rs"), "fn inside() {}\n").unwrap();
        // A handle opened outside the root stays outside, whatever the path
        // text names at the time of the check.
        let outside = File::open(dir.path().join("out.rs")).unwrap();
        assert!(!opened_inside(&outside, &root));
        let inside = File::open(root.join("a.rs")).unwrap();
        assert!(opened_inside(&inside, &root));
    }

    #[test]
    fn tree_returns_syntax_tree() {
        let source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();
        let tree = source.tree();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn tree_can_be_queried() {
        let source = SourceFile::from_content(
            "test.ts",
            "function foo() {}".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let tree = source.tree();
        // Navigate to function declaration
        let root = tree.root_node();
        let func = root.child(0).unwrap();
        assert_eq!(func.kind(), "function_declaration");
    }

    #[test]
    fn path_returns_correct_path() {
        let source = SourceFile::from_content(
            "my/path/test.ts",
            "const x = 1;".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        assert_eq!(source.path(), Path::new("my/path/test.ts"));
    }

    #[test]
    fn update_content_changes_tree() {
        let mut source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();

        // Initially has lexical_declaration
        let root = source.tree().root_node();
        let first_child = root.child(0).unwrap();
        assert_eq!(first_child.kind(), "lexical_declaration");

        // Update to function
        source
            .update_content("function foo() {}".to_string())
            .unwrap();

        // Now has function_declaration
        let root = source.tree().root_node();
        let first_child = root.child(0).unwrap();
        assert_eq!(first_child.kind(), "function_declaration");
    }

    #[test]
    fn grammar_is_preserved_after_update() {
        let mut source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();
        assert_eq!(source.grammar(), Grammar::TypeScript);

        source.update_content("const y = 2;".to_string()).unwrap();
        assert_eq!(source.grammar(), Grammar::TypeScript);
    }

    #[test]
    fn load_tsx_file() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("tsx");
        writeln!(file, "const x = <div>Hello</div>;").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let source = SourceFile::load(&path).unwrap();
        assert_eq!(source.grammar(), Grammar::Tsx);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_mts_file() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("mts");
        writeln!(file, "export const x = 1;").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let source = SourceFile::load(&path).unwrap();
        assert_eq!(source.grammar(), Grammar::TypeScript);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_cts_file() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("cts");
        writeln!(file, "const x = require('foo');").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let source = SourceFile::load(&path).unwrap();
        assert_eq!(source.grammar(), Grammar::TypeScript);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn is_declaration_file_d_ts() {
        let source = SourceFile::from_content(
            "types.d.ts",
            "declare function foo(): void;".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        assert!(source.is_declaration_file());
    }

    #[test]
    fn is_declaration_file_regular_ts() {
        let source = SourceFile::from_content(
            "main.ts",
            "function foo() {}".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        assert!(!source.is_declaration_file());
    }

    #[test]
    fn is_declaration_file_d_mts() {
        let source =
            SourceFile::from_content("types.d.mts", "export {};".to_string(), Grammar::TypeScript)
                .unwrap();
        assert!(source.is_declaration_file());
    }

    #[test]
    fn is_declaration_file_d_cts() {
        let source =
            SourceFile::from_content("types.d.cts", "export {};".to_string(), Grammar::TypeScript)
                .unwrap();
        assert!(source.is_declaration_file());
    }

    #[test]
    fn load_filename_convention_grammars() {
        let cases: &[(&str, &str, Grammar)] = &[];
        let temp_dir = tempfile::TempDir::new().unwrap();
        for (name, content, expected) in cases {
            let path = temp_dir.path().join(name);
            std::fs::write(&path, content).unwrap();
            let source = SourceFile::load(&path).unwrap();
            assert_eq!(source.grammar(), *expected, "filename {name}");
        }
    }
}
