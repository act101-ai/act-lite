//! Parser wrapper with grammar caching.

use crate::{Grammar, ParserError, SyntaxTree, error::Result};
use std::path::Path;
use std::time::{Duration, Instant};
use tree_sitter::Parser as TsParser;

const DEFAULT_PARSE_TIMEOUT: Duration = Duration::from_secs(30);

/// Parser with grammar caching and convenience methods.
///
/// Each instance maintains its own tree-sitter parser state.
/// Grammars are loaded lazily on first use.
pub struct Parser {
    inner: TsParser,
    current_grammar: Option<Grammar>,
    /// Wall-clock cap per parse. `None` disables the cap (unbounded).
    timeout: Option<Duration>,
}

impl Parser {
    /// Creates a new parser instance.
    pub fn new() -> Self {
        Self {
            inner: TsParser::new(),
            current_grammar: None,
            timeout: Some(DEFAULT_PARSE_TIMEOUT),
        }
    }

    /// Sets the per-parse wall-clock deadline. A parse that exceeds it is
    /// cancelled and returns [`ParserError::ParseTimeout`] instead of hanging.
    /// Pass `None` to disable the cap (only for known-trusted input).
    pub fn set_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.timeout = timeout;
        self
    }

    /// Parses source code with the specified grammar.
    ///
    /// Returns a `SyntaxTree` even if the source has syntax errors
    /// (tree-sitter is error-tolerant). Returns [`ParserError::ParseTimeout`]
    /// if parsing exceeds the configured deadline (see [`Parser::set_timeout`]).
    pub fn parse(&mut self, source: &str, grammar: Grammar) -> Result<SyntaxTree> {
        self.set_grammar(grammar)?;
        let tree = self.parse_bounded(source, Path::new("<source>"))?;
        Ok(SyntaxTree::new(tree, source.to_string()))
    }

    /// Parses `source` with `grammar` and one included range, `content`: the
    /// tree of that range alone, whose nodes carry their positions in
    /// `source`. Returns a bare tree, so the text is not copied.
    pub fn parse_region(
        &mut self,
        source: &str,
        grammar: Grammar,
        content: std::ops::Range<usize>,
    ) -> Result<tree_sitter::Tree> {
        let range = crate::regions::LineStarts::new(source).ts_range(content);
        self.parse_included(source, grammar, range)
    }

    /// [`Parser::parse_region`] with the range's points already known.
    pub(crate) fn parse_included(
        &mut self,
        source: &str,
        grammar: Grammar,
        range: tree_sitter::Range,
    ) -> Result<tree_sitter::Tree> {
        if range.start_byte > range.end_byte || range.end_byte > source.len() {
            return Err(ParserError::ParseFailed {
                file: "<source>".into(),
                reason: format!(
                    "region {}..{} is outside the {}-byte text",
                    range.start_byte,
                    range.end_byte,
                    source.len()
                ),
            });
        }
        self.set_grammar(grammar)?;
        self.inner
            .set_included_ranges(&[range])
            .map_err(|e| ParserError::ParseFailed {
                file: "<source>".into(),
                reason: format!("invalid included range: {e}"),
            })?;
        let tree = self.parse_bounded(source, Path::new("<source>"));
        // An empty list restores the parse of the whole text.
        let _ = self.inner.set_included_ranges(&[]);
        tree
    }

    /// Parses the whole of `source` and returns a bare tree, so the caller
    /// can keep the one copy of the text.
    pub(crate) fn parse_tree(
        &mut self,
        source: &str,
        grammar: Grammar,
    ) -> Result<tree_sitter::Tree> {
        self.set_grammar(grammar)?;
        self.parse_bounded(source, Path::new("<source>"))
    }

    /// Runs the tree-sitter parse under the configured deadline. A deadline
    /// breach cancels parsing (via the progress callback) so the call always
    /// terminates.
    fn parse_bounded(&mut self, source: &str, file: &Path) -> Result<tree_sitter::Tree> {
        let bytes = source.as_bytes();
        let deadline = self.timeout.map(|t| Instant::now() + t);

        // Returning `true` from the progress callback cancels the parse. The
        // callback fires periodically during parsing, so a runaway parse is
        // interrupted at the next checkpoint after the deadline.
        let mut over_deadline =
            |_state: &tree_sitter::ParseState| deadline.is_some_and(|d| Instant::now() >= d);
        let options = tree_sitter::ParseOptions::new().progress_callback(&mut over_deadline);

        let mut input = |i: usize, _: tree_sitter::Point| {
            if i < bytes.len() {
                &bytes[i..]
            } else {
                // Empty tail slice (same `&[u8]` type, borrows `bytes`) signals EOF.
                &bytes[bytes.len()..]
            }
        };

        let tree = self
            .inner
            .parse_with_options(&mut input, None, Some(options));
        if tree.is_none() {
            // A halted parse is resumed by the next call unless the parser is
            // reset, so the next text would get this text's partial tree.
            self.inner.reset();
        }

        match tree {
            Some(t) => Ok(t),
            // A `None` after the deadline elapsed is a cancellation, not a
            // genuine parser failure.
            None if deadline.is_some_and(|d| Instant::now() >= d) => {
                Err(ParserError::ParseTimeout {
                    file: file.to_path_buf(),
                    timeout_ms: self.timeout.map(|t| t.as_millis() as u64).unwrap_or(0),
                })
            }
            None => Err(ParserError::ParseFailed {
                file: file.to_path_buf(),
                reason: "tree-sitter parser returned None".to_string(),
            }),
        }
    }

    /// Parses a file, detecting grammar from extension.
    pub fn parse_file(&mut self, path: &Path) -> Result<SyntaxTree> {
        // Read bytes first to distinguish I/O errors from UTF-8 errors
        let bytes = std::fs::read(path).map_err(ParserError::Io)?;
        let source = String::from_utf8(bytes).map_err(|_| ParserError::InvalidUtf8 {
            file: path.to_path_buf(),
        })?;

        let extension = path.extension().and_then(|e| e.to_str()).ok_or_else(|| {
            ParserError::UnsupportedLanguage {
                extension: path.display().to_string(),
            }
        })?;

        let grammar =
            Grammar::from_extension(extension).ok_or_else(|| ParserError::UnsupportedLanguage {
                extension: extension.to_string(),
            })?;

        self.set_grammar(grammar)?;

        let tree = self.parse_bounded(&source, path)?;

        Ok(SyntaxTree::new(tree, source))
    }

    /// Sets the parser's language grammar.
    ///
    /// This is called automatically by `parse` and `parse_file`.
    fn set_grammar(&mut self, grammar: Grammar) -> Result<()> {
        if self.current_grammar != Some(grammar) {
            let language = grammar.language_support().ts_language();

            self.inner
                .set_language(&language)
                .map_err(|e| ParserError::ParseFailed {
                    file: "<unknown>".into(),
                    reason: format!("failed to set language: {}", e),
                })?;

            self.current_grammar = Some(grammar);
        }

        Ok(())
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_parse_after_a_timeout_starts_from_the_new_text() {
        // A large region times out; the next region of the same grammar must
        // not resume the halted parse.
        let big: String = (0..20_000)
            .map(|i| format!("function f{i}() {{ return {i}; }}\n"))
            .collect();
        let small = "let a = 1;\n";
        let text = format!("{big}{small}");
        let mut parser = Parser::new();
        parser.set_timeout(Some(Duration::from_nanos(1)));
        assert!(matches!(
            parser.parse_region(&text, Grammar::JavaScript, 0..big.len()),
            Err(ParserError::ParseTimeout { .. })
        ));
        parser.set_timeout(None);
        let after = parser
            .parse_region(&text, Grammar::JavaScript, big.len()..text.len())
            .unwrap();
        let fresh = Parser::new()
            .parse_region(&text, Grammar::JavaScript, big.len()..text.len())
            .unwrap();
        assert_eq!(after.root_node().to_sexp(), fresh.root_node().to_sexp());
        assert_eq!(
            after.root_node().byte_range(),
            fresh.root_node().byte_range()
        );
    }
    use super::*;

    #[test]
    fn parse_typescript() {
        let mut parser = Parser::new();
        let tree = parser.parse("const x = 1;", Grammar::TypeScript).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn parse_invalid_typescript() {
        let mut parser = Parser::new();
        // Error-tolerant: should still produce a tree
        let tree = parser.parse("const x = ;", Grammar::TypeScript).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(tree.root_node().has_error());
    }

    #[test]
    fn parse_multiple_grammars() {
        let mut parser = Parser::new();
        let tree1 = parser.parse("const x = 1;", Grammar::TypeScript).unwrap();
        assert_eq!(tree1.root_node().kind(), "program");

        // Switching grammar should work
        let tree2 = parser.parse("const y = 2;", Grammar::TypeScript).unwrap();
        assert_eq!(tree2.root_node().kind(), "program");
    }

    #[test]
    fn parse_file_not_found() {
        let mut parser = Parser::new();
        let result = parser.parse_file(Path::new("/nonexistent/file.ts"));
        assert!(result.is_err());
        if let Err(crate::ParserError::Io(_)) = result {
            // Expected I/O error
        } else {
            panic!("Expected I/O error");
        }
    }

    #[test]
    fn parse_file_unsupported_extension() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("xyz");
        writeln!(file, "some content").unwrap();
        std::fs::rename(file.path(), &path).unwrap();

        let mut parser = Parser::new();
        let result = parser.parse_file(&path);
        assert!(result.is_err());
        if let Err(crate::ParserError::UnsupportedLanguage { extension }) = result {
            assert_eq!(extension, "xyz");
        } else {
            panic!("Expected UnsupportedLanguage error");
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn parse_file_invalid_utf8() {
        use tempfile::NamedTempFile;

        let file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("ts");
        // Write invalid UTF-8 bytes
        std::fs::write(&path, [0xFF, 0xFE, 0xFD]).unwrap();

        let mut parser = Parser::new();
        let result = parser.parse_file(&path);
        assert!(result.is_err());
        if let Err(crate::ParserError::InvalidUtf8 { .. }) = result {
            // Expected
        } else {
            panic!("Expected InvalidUtf8 error");
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn parse_file_success() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.ts");
        std::fs::write(&path, "const x: number = 42;").unwrap();

        let mut parser = Parser::new();
        let tree = parser.parse_file(&path).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parse_file_no_extension() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("noext");
        std::fs::write(&path, "some content").unwrap();

        let mut parser = Parser::new();
        let result = parser.parse_file(&path);
        assert!(result.is_err());
        if let Err(crate::ParserError::UnsupportedLanguage { extension }) = result {
            // The extension field contains the full path for no-extension case
            assert!(extension.contains("noext"));
        } else {
            panic!("Expected UnsupportedLanguage error");
        }
    }

    #[test]
    fn parser_default() {
        let parser = Parser::default();
        assert!(parser.current_grammar.is_none());
    }

    #[test]
    fn parse_empty_source() {
        let mut parser = Parser::new();
        let tree = parser.parse("", Grammar::TypeScript).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert_eq!(tree.root_node().child_count(), 0);
    }

    #[test]
    fn parse_complex_typescript() {
        let mut parser = Parser::new();
        let source = r#"
            interface User {
                name: string;
                age: number;
            }

            class UserService {
                private users: User[] = [];

                addUser(user: User): void {
                    this.users.push(user);
                }

                getUsers(): User[] {
                    return this.users;
                }
            }

            const service = new UserService();
        "#;

        let tree = parser.parse(source, Grammar::TypeScript).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parse_reuses_grammar() {
        let mut parser = Parser::new();

        // First parse sets grammar
        parser.parse("const a = 1;", Grammar::TypeScript).unwrap();
        assert_eq!(parser.current_grammar, Some(Grammar::TypeScript));

        // Second parse should reuse the same grammar (no change)
        parser.parse("const b = 2;", Grammar::TypeScript).unwrap();
        assert_eq!(parser.current_grammar, Some(Grammar::TypeScript));
    }

    #[test]
    fn parse_file_with_complex_content() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("complex.ts");
        let content = r#"
            // Complex TypeScript file
            export async function fetchData<T>(url: string): Promise<T> {
                const response = await fetch(url);
                return response.json();
            }

            type Optional<T> = T | undefined;

            export const config = {
                apiUrl: 'https://api.example.com',
                timeout: 5000,
            } as const;
        "#;
        std::fs::write(&path, content).unwrap();

        let mut parser = Parser::new();
        let tree = parser.parse_file(&path).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parse_javascript() {
        let mut parser = Parser::new();
        let tree = parser.parse("const x = 1;", Grammar::JavaScript).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn parse_csharp() {
        let mut parser = Parser::new();
        let tree = parser.parse("class Test {}", Grammar::CSharp).unwrap();
        assert_eq!(tree.root_node().kind(), "compilation_unit");
    }

    #[test]
    fn parse_python() {
        let mut parser = Parser::new();
        let tree = parser.parse("def test(): pass", Grammar::Python).unwrap();
        assert_eq!(tree.root_node().kind(), "module");
    }

    #[test]
    fn parse_rust() {
        let mut parser = Parser::new();
        let tree = parser.parse("fn main() {}", Grammar::Rust).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn parse_go() {
        let mut parser = Parser::new();
        let tree = parser.parse("package main", Grammar::Go).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn parse_elixir() {
        let mut parser = Parser::new();
        let tree = parser
            .parse("defmodule Test do end", Grammar::Elixir)
            .unwrap();
        assert_eq!(tree.root_node().kind(), "source");
    }

    #[test]
    fn parse_java() {
        let mut parser = Parser::new();
        let tree = parser.parse("class Test {}", Grammar::Java).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn parse_kotlin() {
        let mut parser = Parser::new();
        let tree = parser.parse("fun test() {}", Grammar::Kotlin).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn parse_c() {
        let mut parser = Parser::new();
        let tree = parser
            .parse("int main() { return 0; }", Grammar::C)
            .unwrap();
        assert_eq!(tree.root_node().kind(), "translation_unit");
    }

    #[test]
    fn parse_cpp() {
        let mut parser = Parser::new();
        let tree = parser
            .parse("int main() { return 0; }", Grammar::Cpp)
            .unwrap();
        assert_eq!(tree.root_node().kind(), "translation_unit");
    }

    #[test]
    fn parse_ruby() {
        let mut parser = Parser::new();
        let tree = parser.parse("def test; end", Grammar::Ruby).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn parse_grammar_switch() {
        let mut parser = Parser::new();

        // Parse TypeScript
        parser.parse("const x = 1;", Grammar::TypeScript).unwrap();
        assert_eq!(parser.current_grammar, Some(Grammar::TypeScript));

        // Switch to Python
        parser.parse("def test(): pass", Grammar::Python).unwrap();
        assert_eq!(parser.current_grammar, Some(Grammar::Python));

        // Switch to Rust
        parser.parse("fn main() {}", Grammar::Rust).unwrap();
        assert_eq!(parser.current_grammar, Some(Grammar::Rust));
    }
}
