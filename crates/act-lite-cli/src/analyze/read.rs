//! Reading the files in scope: size bound, skipped files, parse failures and
//! the removal of embedded test code, one file at a time.

use super::discovery::{Candidate, Skip};
use super::embedded::{TestRegion, blank, test_regions};
use super::envelope::{AnalyzeError, ExcludedEntry, ParseFailure, SkipKind, SkippedFile};
use act_lite_core::CompactRange;
use act_lite_parser::{Grammar, SourceFile};
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

/// The range of the first ERROR or MISSING node in document order.
fn first_error(source: &SourceFile) -> Option<CompactRange> {
    fn find(node: Node) -> Option<Node> {
        if !node.has_error() {
            return None;
        }
        if node.is_error() || node.is_missing() {
            return Some(node);
        }
        let mut cursor = node.walk();
        node.children(&mut cursor).find_map(find)
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
/// says why not. A file that is not regular is not opened.
fn read_content(file: &Candidate) -> Result<String, Skip> {
    let unreadable = |reason: String| skip(file, SkipKind::Unreadable, reason);
    let meta = std::fs::metadata(&file.path)
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
    std::fs::File::open(&file.path)
        .and_then(|f| f.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes))
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
    use std::fs;
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
}
