//! Reading the files in scope: size bound, skipped files, parse failures and
//! the removal of embedded test code, one file at a time.

use super::classify::open_no_follow;
use super::discovery::{Candidate, Skip};
use super::embedded::{blank, test_regions};
use super::envelope::{AnalyzeError, ExcludedEntry, ParseFailure, SkipKind, SkippedFile};
use act_lite_core::CompactRange;
use act_lite_parser::{Grammar, SourceFile};
use std::fs;
use std::io::Read;
use tokio_util::sync::CancellationToken;
use tree_sitter::Node;

/// The largest file that is read, in bytes.
pub const MAX_FILE_BYTES: u64 = act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES;

/// A file read and parsed. With `include_tests` false the content has its
/// test regions blanked; every location is the file's own.
pub struct Parsed {
    pub rel: String,
    pub source: SourceFile,
}

/// The result of reading one file.
pub enum ReadResult {
    Parsed {
        parsed: Parsed,
        parse_failures: Vec<ParseFailure>,
        test_exclusions: Vec<ExcludedEntry>,
    },
    Skipped(Skip),
    /// A file whose own attribute `#![cfg(test)]` makes it test code.
    TestFile(ExcludedEntry),
}

/// Reads and parses one file.
pub fn read_file(file: &Candidate, include_tests: bool) -> Result<ReadResult, AnalyzeError> {
    let content = match read_content(file) {
        Ok(content) => content,
        Err(skip) => return Ok(ReadResult::Skipped(skip)),
    };
    let mut source = parse(file, content)?;
    let mut test_exclusions = Vec::new();
    if !include_tests {
        let found = test_regions(&source);
        if found.whole_file {
            return Ok(ReadResult::TestFile(ExcludedEntry {
                path: file.rel.clone(),
                range: None,
            }));
        }
        if !found.regions.is_empty() {
            test_exclusions = found
                .regions
                .iter()
                .map(|region| ExcludedEntry {
                    path: file.rel.clone(),
                    range: Some(region.range.clone()),
                })
                .collect();
            source = parse(file, blank(source.content(), &found.regions))?;
        }
    }
    let parse_failures = first_error(&source)
        .map(|range| ParseFailure {
            file: file.rel.clone(),
            range,
        })
        .into_iter()
        .collect();
    Ok(ReadResult::Parsed {
        parsed: Parsed {
            rel: file.rel.clone(),
            source,
        },
        parse_failures,
        test_exclusions,
    })
}

/// What reading found besides the files themselves.
#[derive(Default)]
pub struct ReadOutcome {
    /// Sorted by `rel`; empty from [`Reader::finish`].
    pub parsed: Vec<Parsed>,
    pub skipped: Vec<Skip>,
    pub parse_failures: Vec<ParseFailure>,
    pub test_exclusions: Vec<ExcludedEntry>,
}

/// Files read on demand, one at a time.
pub trait FileSource: Send {
    /// The next supported-language file in `rel` order, read now. The caller
    /// owns the result and drops it when done.
    fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError>;

    /// Reads a file returned by `next_file` again, with the same blanking.
    /// A parse failure or test exclusion it finds is added to the evidence.
    fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError>;
}

/// A [`FileSource`] over a list of candidates.
pub struct Reader {
    files: Vec<Candidate>,
    next: usize,
    include_tests: bool,
    cancel: CancellationToken,
    outcome: ReadOutcome,
}

impl Reader {
    /// Takes the candidates `supported` accepts, in `rel` order. Nothing is
    /// read until `next_file`.
    pub fn new(
        files: &[Candidate],
        supported: &dyn Fn(Grammar) -> bool,
        include_tests: bool,
        cancel: &CancellationToken,
    ) -> Reader {
        let mut files: Vec<Candidate> = files
            .iter()
            .filter(|f| supported(f.grammar))
            .cloned()
            .collect();
        files.sort_by(|a, b| a.rel.as_bytes().cmp(b.rel.as_bytes()));
        Reader {
            files,
            next: 0,
            include_tests,
            cancel: cancel.clone(),
            outcome: ReadOutcome::default(),
        }
    }

    /// The skips, parse failures and test exclusions of every file pulled so
    /// far, each list sorted.
    pub fn finish(mut self) -> ReadOutcome {
        let outcome = &mut self.outcome;
        outcome
            .skipped
            .sort_by(|a, b| a.file.file.as_bytes().cmp(b.file.file.as_bytes()));
        outcome
            .parse_failures
            .sort_by(|a, b| a.file.as_bytes().cmp(b.file.as_bytes()));
        outcome.test_exclusions.sort_by(|a, b| {
            let key = |e: &ExcludedEntry| {
                e.range
                    .as_ref()
                    .map(|r| (r.start_line(), r.start_col(), r.end_line(), r.end_col()))
            };
            a.path
                .as_bytes()
                .cmp(b.path.as_bytes())
                .then_with(|| key(a).cmp(&key(b)))
        });
        self.outcome
    }
}

impl FileSource for Reader {
    fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
        while let Some(file) = self.files.get(self.next) {
            if self.cancel.is_cancelled() {
                return Err(AnalyzeError::Cancelled);
            }
            self.next += 1;
            match read_file(file, self.include_tests)? {
                ReadResult::Parsed {
                    parsed,
                    parse_failures,
                    test_exclusions,
                } => {
                    self.outcome.parse_failures.extend(parse_failures);
                    self.outcome.test_exclusions.extend(test_exclusions);
                    return Ok(Some(parsed));
                }
                ReadResult::Skipped(skip) => self.outcome.skipped.push(skip),
                ReadResult::TestFile(entry) => self.outcome.test_exclusions.push(entry),
            }
        }
        Ok(None)
    }

    fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
        if self.cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        let failed = |cause: String| AnalyzeError::Failed {
            analysis: "analyze",
            cause,
        };
        let Some(file) = self.files[..self.next].iter().find(|f| f.rel == rel) else {
            return Err(failed(format!("{rel} was not returned by this reader")));
        };
        match read_file(file, self.include_tests)? {
            ReadResult::Parsed {
                parsed,
                parse_failures,
                test_exclusions,
            } => {
                add_new(&mut self.outcome.parse_failures, parse_failures);
                add_new(&mut self.outcome.test_exclusions, test_exclusions);
                Ok(parsed)
            }
            ReadResult::Skipped(skip) => Err(failed(format!(
                "cannot read {rel} again: {}",
                skip.file.reason
            ))),
            ReadResult::TestFile(_) => Err(failed(format!("{rel} is now a test file"))),
        }
    }
}

/// Adds each of `found` that `list` does not hold yet.
fn add_new<T: PartialEq>(list: &mut Vec<T>, found: Vec<T>) {
    for item in found {
        if !list.contains(&item) {
            list.push(item);
        }
    }
}

/// Reads every supported-language file and keeps them all in `parsed`.
pub fn read_files(
    files: &[Candidate],
    supported: &dyn Fn(Grammar) -> bool,
    include_tests: bool,
    cancel: &CancellationToken,
) -> Result<ReadOutcome, AnalyzeError> {
    let mut reader = Reader::new(files, supported, include_tests, cancel);
    let mut parsed = Vec::new();
    while let Some(file) = reader.next_file()? {
        parsed.push(file);
    }
    let mut outcome = reader.finish();
    outcome.parsed = parsed;
    Ok(outcome)
}

/// The range of the first ERROR or MISSING node in document order.
fn first_error(source: &SourceFile) -> Option<CompactRange> {
    fn find(root: Node) -> Option<Node> {
        let mut cursor = root.walk();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if !node.has_error() {
                continue;
            }
            // A recovered leaf can carry the error without being marked missing
            // (tree-sitter-zig's empty container member).
            if node.is_error() || node.is_missing() || node.child_count() == 0 {
                return Some(node);
            }
            let start = stack.len();
            stack.extend(node.children(&mut cursor));
            stack[start..].reverse();
        }
        None
    }
    let node = find(source.tree().root_node())?;
    let (start, end) = (node.start_position(), node.end_position());
    Some(CompactRange::new(
        start.row as u32 + 1,
        start.column as u32 + 1,
        end.row as u32 + 1,
        end.column as u32 + 1,
    ))
}

fn skip(file: &Candidate, kind: SkipKind, reason: String) -> Skip {
    Skip {
        grammar: Some(file.grammar),
        file: SkippedFile {
            file: file.rel.clone(),
            kind,
            reason,
        },
    }
}

/// The text of a regular file of at most [`MAX_FILE_BYTES`], or the skip that
/// says why not. A symlink is not followed, and a file that is not regular is
/// not read.
fn read_content(file: &Candidate) -> Result<String, Skip> {
    let unreadable = |reason: String| skip(file, SkipKind::Unreadable, reason);
    let linked = fs::symlink_metadata(&file.path)
        .map_err(|err| unreadable(format!("cannot read file: {err}")))?;
    if !linked.is_file() {
        return Err(unreadable("not a regular file".to_string()));
    }
    let handle =
        open_no_follow(&file.path).map_err(|err| unreadable(format!("cannot read file: {err}")))?;
    let meta = handle
        .metadata()
        .map_err(|err| unreadable(format!("cannot stat file: {err}")))?;
    if !meta.is_file() {
        return Err(unreadable("not a regular file".to_string()));
    }
    let too_big = |len: u64| {
        skip(
            file,
            SkipKind::SizeLimit,
            format!("exceeds size cap: {len} bytes > cap of {MAX_FILE_BYTES} bytes"),
        )
    };
    if meta.len() > MAX_FILE_BYTES {
        return Err(too_big(meta.len()));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    handle
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| unreadable(format!("cannot read file: {err}")))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(too_big(bytes.len() as u64));
    }
    String::from_utf8(bytes).map_err(|_| {
        skip(
            file,
            SkipKind::NotUtf8,
            "content is not valid UTF-8".to_string(),
        )
    })
}

fn parse(file: &Candidate, content: String) -> Result<SourceFile, AnalyzeError> {
    SourceFile::from_content(&file.path, content, file.grammar).map_err(|err| {
        AnalyzeError::Failed {
            analysis: "analyze",
            cause: format!("cannot parse {}: {err}", file.rel),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn candidate(root: &Path, rel: &str) -> Candidate {
        Candidate {
            path: root.join(rel),
            rel: rel.to_string(),
            grammar: Grammar::Rust,
        }
    }

    fn write(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn skipped(result: Result<ReadResult, AnalyzeError>) -> Skip {
        match result {
            Ok(ReadResult::Skipped(skip)) => skip,
            _ => panic!("expected a skipped file"),
        }
    }

    fn parsed(
        result: Result<ReadResult, AnalyzeError>,
    ) -> (Parsed, Vec<ParseFailure>, Vec<ExcludedEntry>) {
        match result {
            Ok(ReadResult::Parsed {
                parsed,
                parse_failures,
                test_exclusions,
            }) => (parsed, parse_failures, test_exclusions),
            _ => panic!("expected a parsed file"),
        }
    }

    /// Valid Rust of exactly `len` bytes.
    fn rust_of_len(len: usize) -> Vec<u8> {
        let mut out = b"fn a() {}\n//".to_vec();
        out.resize(len - 1, b'x');
        out.push(b'\n');
        out
    }

    #[test]
    fn file_over_the_cap_is_skipped_without_being_read() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/big.rs", &rust_of_len(8_388_609));
        let skip = skipped(read_file(&candidate(dir.path(), "src/big.rs"), false));
        assert_eq!(skip.grammar, Some(Grammar::Rust));
        assert_eq!(skip.file.file, "src/big.rs");
        assert_eq!(skip.file.kind, SkipKind::SizeLimit);
        assert_eq!(
            skip.file.reason,
            "exceeds size cap: 8388609 bytes > cap of 8388608 bytes"
        );
    }

    #[test]
    fn file_at_the_cap_is_parsed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/ok.rs", &rust_of_len(8_388_608));
        let (p, failures, exclusion) =
            parsed(read_file(&candidate(dir.path(), "src/ok.rs"), false));
        assert_eq!(p.rel, "src/ok.rs");
        assert_eq!(p.source.content().len(), 8_388_608);
        assert!(failures.is_empty() && exclusion.is_empty());
    }

    #[test]
    fn invalid_utf8_is_not_utf8() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {} // \xff\n");
        let skip = skipped(read_file(&candidate(dir.path(), "a.rs"), false));
        assert_eq!(skip.file.kind, SkipKind::NotUtf8);
        assert!(!skip.file.reason.contains(dir.path().to_str().unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn fifo_is_unreadable_and_not_opened() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("p.rs");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        let skip = skipped(read_file(&candidate(dir.path(), "p.rs"), false));
        assert_eq!(skip.file.kind, SkipKind::Unreadable);
        assert_eq!(skip.file.reason, "not a regular file");
    }

    #[cfg(unix)]
    #[test]
    fn file_without_permission_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}\n");
        fs::set_permissions(dir.path().join("a.rs"), fs::Permissions::from_mode(0o000)).unwrap();
        if fs::File::open(dir.path().join("a.rs")).is_ok() {
            return;
        }
        let skip = skipped(read_file(&candidate(dir.path(), "a.rs"), false));
        assert_eq!(skip.file.kind, SkipKind::Unreadable);
        assert!(!skip.file.reason.contains(dir.path().to_str().unwrap()));
    }

    /// The files discovery records in `root`.
    #[cfg(unix)]
    fn discovered(root: &Path) -> Vec<Candidate> {
        use crate::analyze::discovery::{Includes, Scope, discover};
        let scope = Scope::new(root, &[], &[], &[], Includes::default()).unwrap();
        discover(root, &scope, &rust_only, &CancellationToken::new())
            .unwrap()
            .files
    }

    #[cfg(unix)]
    #[test]
    fn symlink_swapped_in_after_discovery_is_not_followed() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.rs", b"fn secret() {}\n");
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}\n");
        let files = discovered(dir.path());
        assert_eq!(files.len(), 1);
        fs::remove_file(dir.path().join("a.rs")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.rs"), dir.path().join("a.rs"))
            .unwrap();
        let skip = skipped(read_file(&files[0], false));
        assert_eq!(skip.file.kind, SkipKind::Unreadable);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_file_is_not_read_before_any_open() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.rs", b"fn secret() {}\n");
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}\n");
        let files = discovered(dir.path());
        fs::remove_file(dir.path().join("a.rs")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.rs"), dir.path().join("a.rs"))
            .unwrap();
        let skip = skipped(read_file(&files[0], false));
        assert_eq!(skip.file.reason, "not a regular file");
    }

    #[cfg(unix)]
    #[test]
    fn fifo_swapped_in_after_discovery_is_not_waited_on() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}\n");
        let files = discovered(dir.path());
        assert_eq!(files.len(), 1);
        fs::remove_file(dir.path().join("a.rs")).unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(dir.path().join("a.rs"))
            .status()
            .unwrap();
        assert!(made.success());
        let (tx, rx) = std::sync::mpsc::channel();
        let file = files[0].clone();
        std::thread::spawn(move || {
            let _ = tx.send(skipped(read_file(&file, false)));
        });
        let skip = rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("the read returns within 1 s");
        assert_eq!(skip.file.kind, SkipKind::Unreadable);
        assert_eq!(skip.file.reason, "not a regular file");
    }

    #[test]
    fn missing_file_is_unreadable_without_an_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let skip = skipped(read_file(&candidate(dir.path(), "gone.rs"), false));
        assert_eq!(skip.file.kind, SkipKind::Unreadable);
        assert!(!skip.file.reason.contains(dir.path().to_str().unwrap()));
    }

    const LIB: &str = "fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    if sum > 100 {\n        return 100;\n    }\n    sum\n}\n\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";

    fn has_mod_tests(p: &Parsed) -> bool {
        p.source.content().contains("mod tests")
    }

    #[test]
    fn syntax_error_is_one_parse_failure_and_the_file_is_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let content =
            "fn broken() {\n    let a = 1;\n    let b = 2;\n    let c = ;\n    a + b\n}\n";
        write(dir.path(), "src/broken.rs", content.as_bytes());
        let (p, failures, _) = parsed(read_file(&candidate(dir.path(), "src/broken.rs"), false));
        assert_eq!(p.rel, "src/broken.rs");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].file, "src/broken.rs");
        assert_eq!(failures[0].range.start_line(), 4);
        assert_eq!(failures[0].range.end_line(), 4);
    }

    #[test]
    fn test_module_is_blanked_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/lib.rs", LIB.as_bytes());
        let (p, failures, exclusion) =
            parsed(read_file(&candidate(dir.path(), "src/lib.rs"), false));
        assert!(failures.is_empty());
        assert_eq!(exclusion.len(), 1);
        assert_eq!(exclusion[0].path, "src/lib.rs");
        assert_eq!(
            exclusion[0].range.as_ref().unwrap().to_string(),
            "10:1-14:2"
        );
        assert!(!has_mod_tests(&p));
        assert_eq!(p.source.content().lines().count(), LIB.lines().count());
        assert!(p.source.content().starts_with("fn add"));
    }

    #[test]
    fn tests_are_kept_when_included() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/lib.rs", LIB.as_bytes());
        let (p, _, exclusion) = parsed(read_file(&candidate(dir.path(), "src/lib.rs"), true));
        assert!(exclusion.is_empty());
        assert_eq!(p.source.content(), LIB);
    }

    #[test]
    fn syntax_error_inside_a_test_module_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        let content =
            "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() {\n        let c = ;\n    }\n}\n";
        write(dir.path(), "a.rs", content.as_bytes());
        let (_, failures, exclusion) = parsed(read_file(&candidate(dir.path(), "a.rs"), false));
        assert!(failures.is_empty());
        assert_eq!(exclusion[0].range.as_ref().unwrap().to_string(), "2:1-7:2");
    }

    #[test]
    fn inner_cfg_test_file_is_a_whole_file_exclusion() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "t.rs", b"#![cfg(test)]\nfn t() {}\n");
        match read_file(&candidate(dir.path(), "t.rs"), false) {
            Ok(ReadResult::TestFile(entry)) => {
                assert_eq!(entry.path, "t.rs");
                assert!(entry.range.is_none());
            }
            _ => panic!("expected a test file"),
        }
    }

    #[test]
    fn inner_cfg_test_file_is_parsed_when_tests_are_included() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "t.rs", b"#![cfg(test)]\nfn t() {}\n");
        let (_, _, exclusion) = parsed(read_file(&candidate(dir.path(), "t.rs"), true));
        assert!(exclusion.is_empty());
    }

    fn rust_only(g: Grammar) -> bool {
        g == Grammar::Rust
    }

    fn conv(root: &Path) -> Vec<Candidate> {
        write(root, "src/lib.rs", LIB.as_bytes());
        write(root, "src/big.rs", &rust_of_len(8_388_609));
        write(
            root,
            "src/broken.rs",
            b"fn broken() {\n    let a = 1;\n    let b = 2;\n    let c = ;\n    a + b\n}\n",
        );
        write(root, "src/more.rs", b"fn more() {}\n");
        write(root, "src/zz.rs", b"#![cfg(test)]\nfn t() {}\n");
        write(root, "notes.toml", b"a = 1\n");
        let mut files: Vec<Candidate> = [
            "src/zz.rs",
            "src/more.rs",
            "src/lib.rs",
            "src/big.rs",
            "src/broken.rs",
        ]
        .iter()
        .map(|rel| candidate(root, rel))
        .collect();
        files.push(Candidate {
            path: root.join("notes.toml"),
            rel: "notes.toml".to_string(),
            grammar: Grammar::Toml,
        });
        files
    }

    fn pulled(reader: &mut Reader) -> Vec<String> {
        let mut rels = Vec::new();
        while let Some(p) = reader.next_file().unwrap() {
            rels.push(p.rel);
        }
        rels
    }

    #[test]
    fn reader_yields_supported_files_in_rel_order_and_finish_reports_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let mut reader = Reader::new(&files, &rust_only, false, &CancellationToken::new());
        assert_eq!(
            pulled(&mut reader),
            ["src/broken.rs", "src/lib.rs", "src/more.rs"]
        );
        let out = reader.finish();
        assert!(out.parsed.is_empty());
        let skipped: Vec<_> = out
            .skipped
            .iter()
            .map(|s| (s.file.file.as_str(), s.file.kind))
            .collect();
        assert_eq!(skipped, [("src/big.rs", SkipKind::SizeLimit)]);
        assert_eq!(out.parse_failures.len(), 1);
        assert_eq!(out.parse_failures[0].file, "src/broken.rs");
        assert_eq!(out.parse_failures[0].range.start_line(), 4);
        let tests: Vec<_> = out
            .test_exclusions
            .iter()
            .map(|e| (e.path.as_str(), e.range.as_ref().map(|r| r.to_string())))
            .collect();
        assert_eq!(
            tests,
            [
                ("src/lib.rs", Some("10:1-14:2".to_string())),
                ("src/zz.rs", None)
            ]
        );
    }

    #[test]
    fn read_files_collects_parsed_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let out = read_files(&files, &rust_only, false, &CancellationToken::new()).unwrap();
        let rels: Vec<_> = out.parsed.iter().map(|p| p.rel.as_str()).collect();
        assert_eq!(rels, ["src/broken.rs", "src/lib.rs", "src/more.rs"]);
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.parse_failures.len(), 1);
        assert_eq!(out.test_exclusions.len(), 2);
        assert!(!has_mod_tests(&out.parsed[1]));
    }

    #[test]
    fn read_files_with_tests_included_has_no_exclusion() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let out = read_files(&files, &rust_only, true, &CancellationToken::new()).unwrap();
        assert!(out.test_exclusions.is_empty());
        assert_eq!(out.parsed.len(), 4);
        assert!(has_mod_tests(&out.parsed[1]));
    }

    #[test]
    fn unsupported_language_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let files = vec![Candidate {
            path: dir.path().join("absent.toml"),
            rel: "absent.toml".to_string(),
            grammar: Grammar::Toml,
        }];
        let out = read_files(&files, &rust_only, false, &CancellationToken::new()).unwrap();
        assert!(out.parsed.is_empty() && out.skipped.is_empty());
    }

    #[test]
    fn cancelled_token_stops_before_any_read() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            read_files(&files, &rust_only, false, &cancel),
            Err(AnalyzeError::Cancelled)
        ));
        let mut reader = Reader::new(&files, &rust_only, false, &cancel);
        assert!(matches!(reader.next_file(), Err(AnalyzeError::Cancelled)));
        let out = reader.finish();
        assert!(out.skipped.is_empty() && out.parse_failures.is_empty());
    }

    #[test]
    fn reread_gives_an_equal_file() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let mut reader = Reader::new(&files, &rust_only, false, &CancellationToken::new());
        let first = reader.next_file().unwrap().unwrap();
        let second = reader.next_file().unwrap().unwrap();
        assert_eq!(second.rel, "src/lib.rs");
        let again = reader.reread("src/lib.rs").unwrap();
        assert_eq!(again.rel, second.rel);
        assert_eq!(again.source.path(), second.source.path());
        assert_eq!(again.source.content(), second.source.content());
        assert_eq!(
            again.source.tree().root_node().to_sexp(),
            second.source.tree().root_node().to_sexp()
        );
        assert!(!has_mod_tests(&again));
        drop(first);
    }

    #[test]
    fn reread_after_cancel_is_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let cancel = CancellationToken::new();
        let mut reader = Reader::new(&files, &rust_only, false, &cancel);
        reader.next_file().unwrap().unwrap();
        cancel.cancel();
        assert!(matches!(
            reader.reread("src/broken.rs"),
            Err(AnalyzeError::Cancelled)
        ));
    }

    #[test]
    fn reread_records_a_new_parse_failure_once() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let mut reader = Reader::new(&files, &rust_only, false, &CancellationToken::new());
        pulled(&mut reader);
        write(
            dir.path(),
            "src/more.rs",
            b"fn more() {\n    let c = ;\n}\n",
        );
        reader.reread("src/more.rs").unwrap();
        reader.reread("src/more.rs").unwrap();
        reader.reread("src/broken.rs").unwrap();
        reader.reread("src/lib.rs").unwrap();
        let out = reader.finish();
        let failures: Vec<_> = out
            .parse_failures
            .iter()
            .map(|f| (f.file.as_str(), f.range.start_line()))
            .collect();
        assert_eq!(failures, [("src/broken.rs", 4), ("src/more.rs", 2)]);
        assert_eq!(out.test_exclusions.len(), 2);
    }

    #[test]
    fn reread_of_an_unknown_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let files = conv(dir.path());
        let mut reader = Reader::new(&files, &rust_only, false, &CancellationToken::new());
        reader.next_file().unwrap();
        for rel in ["nope.rs", "src/more.rs", "notes.toml"] {
            assert!(
                matches!(reader.reread(rel), Err(AnalyzeError::Failed { .. })),
                "{rel}"
            );
        }
    }

    #[cfg(unix)]
    fn rss_kib() -> u64 {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn dropped_files_are_not_kept_by_the_reader() {
        // Other tests of the process allocate at the same time, so the
        // measurement runs in a child process that runs only this test.
        const NAME: &str = "analyze::read::tests::dropped_files_are_not_kept_by_the_reader";
        if std::env::var_os("ACT_RSS_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME])
                .env("ACT_RSS_CHILD", "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for n in 0..3 {
            let rel = format!("f{n}.rs");
            let mut text = String::new();
            let mut i = 0;
            while text.len() < 3 << 20 {
                text.push_str(&format!("fn f{i}(a: u32) -> u32 {{ a + {i} }}\n"));
                i += 1;
            }
            write(dir.path(), &rel, text.as_bytes());
            files.push(candidate(dir.path(), &rel));
        }
        // The allocator keeps the pages of the first parse; read one file
        // first so that only growth by retained files counts.
        drop(read_file(&files[0], false).unwrap());
        let mut reader = Reader::new(&files, &rust_only, false, &CancellationToken::new());
        let before = rss_kib();
        let mut rels = Vec::new();
        while let Some(p) = reader.next_file().unwrap() {
            rels.push(p.rel.clone());
            drop(p);
        }
        let after = rss_kib();
        assert_eq!(rels.len(), 3);
        assert!(
            after < before + 4096,
            "resident set grew from {before} KiB to {after} KiB"
        );
    }
}
