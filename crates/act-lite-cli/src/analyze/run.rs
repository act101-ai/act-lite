//! The analysis runner: discovery, reading one file at a time, the analyzer
//! and the result envelope.

use super::discovery::{Includes, Scope, discover};
use super::envelope::{
    AnalyzeError, Coverage, EmptyScope, Envelope, Evidence, Excluded, ExcludedKind, Exclusion,
    Finding, Incomplete, LanguageCoverage, Limit, LimitHit, Unresolved,
};
use super::read::{FileSource, Parsed, Reader};
use act_lite_parser::Grammar;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

/// One analysis over the files of a scope.
pub trait Analyzer: Send + Sync {
    /// The `analysis` value and the CLI subcommand.
    const NAME: &'static str;
    /// The usual next step, shown with the result over MCP.
    const HINT: &'static str;
    type Finding: Finding;

    /// Whether the analyzer reads files of `grammar`.
    fn supports(&self, grammar: Grammar) -> bool;

    /// Pulls each file with [`FileSource::next_file`], analyzes it and drops
    /// it before it pulls the next one. Checks `cancel` between steps.
    fn analyze(
        &self,
        files: &mut dyn FileSource,
        cancel: &CancellationToken,
    ) -> Result<Analysis<Self::Finding>, AnalyzeError>;
}

/// What an analyzer found.
pub struct Analysis<F> {
    pub findings: Vec<F>,
    pub unresolved: Vec<Unresolved>,
    pub limits_hit: Vec<LimitHit>,
    pub details: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The shared parameters of an analysis, checked.
pub struct Request {
    pub root: PathBuf,
    pub scope: Scope,
    pub limit: Limit,
}

impl Request {
    /// Checks `limit`, then each path in order.
    pub fn new(
        root: &Path,
        paths: &[String],
        include: &[String],
        exclude: &[String],
        includes: Includes,
        limit: u32,
    ) -> Result<Request, AnalyzeError> {
        let limit = Limit::new(limit)?;
        let scope = Scope::new(root, paths, include, exclude, includes)?;
        Ok(Request {
            root: root.to_path_buf(),
            scope,
            limit,
        })
    }
}

/// Discovers the scope, hands its files to `analyzer` one at a time and
/// builds the envelope. Files the analyzer did not pull are read for their
/// evidence only; leaving any is a failure unless the analyzer reports a
/// limit hit. A failure carries the analyzer's name.
pub fn run<A: Analyzer>(
    analyzer: &A,
    request: &Request,
    cancel: &CancellationToken,
) -> Result<Envelope<A::Finding>, AnalyzeError> {
    run_named(analyzer, request, cancel).map_err(|err| err.with_analysis(A::NAME))
}

fn run_named<A: Analyzer>(
    analyzer: &A,
    request: &Request,
    cancel: &CancellationToken,
) -> Result<Envelope<A::Finding>, AnalyzeError> {
    if cancel.is_cancelled() {
        return Err(AnalyzeError::Cancelled);
    }
    let supported = |g: Grammar| analyzer.supports(g);
    let discovered = discover(&request.root, &request.scope, &supported, cancel)?;
    let mut reader = Reader::new(
        &discovered.files,
        &supported,
        request.scope.includes.tests,
        cancel,
    );
    let mut counted = Counted {
        inner: &mut reader,
        files: 0,
    };
    let analysis = analyzer.analyze(&mut counted, cancel)?;
    let analyzed_files = counted.files;
    if cancel.is_cancelled() {
        return Err(AnalyzeError::Cancelled);
    }
    let mut unpulled = None;
    while let Some(file) = reader.next_file()? {
        unpulled.get_or_insert(file.rel);
    }
    if let Some(rel) = unpulled.filter(|_| analysis.limits_hit.is_empty()) {
        return Err(AnalyzeError::Failed {
            analysis: A::NAME,
            cause: format!("stopped before {rel} without reporting a limit"),
        });
    }
    let read = reader.finish();

    let test_files: HashSet<&str> = read
        .test_exclusions
        .iter()
        .filter(|e| e.range.is_none())
        .map(|e| e.path.as_str())
        .collect();
    let mut files: HashMap<Grammar, u64> = HashMap::new();
    let in_scope = discovered
        .files
        .iter()
        .filter(|f| !test_files.contains(f.rel.as_str()))
        .map(|f| f.grammar);
    for grammar in in_scope.chain(discovered.skipped.iter().filter_map(|s| s.grammar)) {
        *files.entry(grammar).or_default() += 1;
    }
    let languages: Vec<LanguageCoverage> = files
        .into_iter()
        .map(|(grammar, files)| LanguageCoverage {
            language: grammar.name(),
            files,
            supported: analyzer.supports(grammar),
        })
        .collect();
    let removed_supported = discovered.removed_supported || !test_files.is_empty();
    let no_supported_file = !languages.iter().any(|l| l.supported);
    let no_recognized_file = languages.is_empty();
    let coverage = Coverage::new(languages, analyzed_files, discovered.unrecognized);

    let tests = read.test_exclusions.into_iter().map(|entry| Exclusion {
        kind: ExcludedKind::Tests,
        entry,
    });
    let excluded = Excluded::new(discovered.excluded.into_iter().chain(tests).collect());
    let skipped = discovered
        .skipped
        .into_iter()
        .chain(read.skipped)
        .map(|s| s.file)
        .collect();
    let incomplete = Incomplete::new(
        skipped,
        read.parse_failures,
        analysis.unresolved,
        analysis.limits_hit,
    );

    if no_supported_file && incomplete.is_complete() && (no_recognized_file || removed_supported) {
        let counts = &excluded.counts;
        return Err(AnalyzeError::EmptyScope(EmptyScope {
            tests: counts.tests,
            generated: counts.generated,
            vendored: counts.vendored,
            ignored: counts.ignored,
            filtered: discovered.filtered,
            unrecognized: coverage.unrecognized_files,
        }));
    }
    let sources = if analyzed_files >= 1 {
        vec!["tree-sitter".to_string()]
    } else {
        Vec::new()
    };
    Envelope::new(
        A::NAME,
        analysis.findings,
        request.limit,
        coverage,
        excluded,
        incomplete,
        Evidence::new(sources, Vec::new()),
        analysis.details,
    )
}

/// A [`FileSource`] that counts the files it gives.
struct Counted<'a> {
    inner: &'a mut Reader,
    files: u64,
}

impl FileSource for Counted<'_> {
    fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
        let file = self.inner.next_file()?;
        self.files += u64::from(file.is_some());
        Ok(file)
    }

    fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
        self.inner.reread(rel)
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use crate::analyze::fixture::{FixtureAnalyzer, FixtureFinding};
    use serde_json::{Value, json};
    use std::fs;
    #[cfg(unix)]
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn write(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// A function on lines 1–8 when `long`, else on lines 1–3.
    fn function(name: &str, long: bool) -> String {
        if long {
            format!(
                "fn {name}(a: i32, b: i32) -> i32 {{\n    let sum = a + b;\n    if sum > 100 {{\n        return 100;\n    }}\n    let out = sum;\n    out\n}}\n"
            )
        } else {
            format!("fn {name}() {{\n    let _ = 1;\n}}\n")
        }
    }

    /// Valid Rust of exactly `len` bytes.
    fn rust_of_len(len: usize) -> Vec<u8> {
        let mut out = b"fn a() {}\n//".to_vec();
        out.resize(len - 1, b'x');
        out.push(b'\n');
        out
    }

    /// The `conv` workspace. With `long` false, no function is reported.
    fn conv(root: &Path, long: bool) {
        fs::create_dir_all(root.join(".git")).unwrap();
        write(root, ".gitignore", b"build-out/\n*.log\n");
        write(root, "Cargo.toml", b"[package]\nname = \"conv\"\n");
        let lib = format!(
            "{}\n#[cfg(test)]\nmod tests {{\n    #[test]\n    fn t() {{}}\n}}\n",
            function("add", long)
        );
        write(root, "src/lib.rs", lib.as_bytes());
        write(root, "src/big.rs", &rust_of_len(8_388_609));
        let broken = if long {
            "fn broken() {\n    let a = 1;\n    let b = 2;\n    let c = ;\n    a + b\n}\n"
        } else {
            "fn broken() {\n    let c = ;\n}\n"
        };
        write(root, "src/broken.rs", broken.as_bytes());
        write(root, "src/more.rs", function("more", long).as_bytes());
        write(
            root,
            "src/gen.rs",
            b"// Code generated by synth. DO NOT EDIT.\nfn g() {}\n",
        );
        write(root, "src/notes.txt", b"notes");
        write(root, "tests/cli.rs", b"fn t() {}");
        write(root, "vendor/dep/lib.rs", b"fn v() {}");
        write(root, "build-out/x.rs", b"fn x() {}");
        write(root, "debug.log", b"log");
    }

    fn request(root: &Path, paths: &[&str], exclude: &[&str], limit: u32) -> Request {
        let strings = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Request::new(
            root,
            &strings(paths),
            &[],
            &strings(exclude),
            Includes::default(),
            limit,
        )
        .unwrap()
    }

    fn fixture(request: &Request) -> Result<Envelope<FixtureFinding>, AnalyzeError> {
        run(&FixtureAnalyzer, request, &CancellationToken::new())
    }

    fn value<F: Finding>(result: Result<Envelope<F>, AnalyzeError>) -> Value {
        serde_json::to_value(result.unwrap()).unwrap()
    }

    fn empty_scope(result: Result<Envelope<FixtureFinding>, AnalyzeError>) -> EmptyScope {
        match result {
            Err(AnalyzeError::EmptyScope(s)) => s,
            Err(other) => panic!("expected the empty-scope error, got {other}"),
            Ok(_) => panic!("expected the empty-scope error, got a result"),
        }
    }

    fn no_lists() -> Value {
        json!({"tests":[],"generated":[],"vendored":[],"ignored":[],
               "counts":{"tests":0,"generated":0,"vendored":0,"ignored":0},"truncated":false})
    }

    #[test]
    fn conv_with_limit_2_is_the_worked_example() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path(), true);
        let mut out = value(fixture(&request(dir.path(), &[], &[], 2)));
        // Compared apart from: finding ranges, the reason text and the
        // parse-failure range, which is on line 4.
        for f in out["findings"].as_array_mut().unwrap() {
            f.as_object_mut().unwrap().remove("range");
        }
        let failure = out["incomplete"]["parse_failures"][0]
            .as_object_mut()
            .unwrap()
            .remove("range")
            .unwrap();
        assert!(failure.as_str().unwrap().starts_with("4:"), "{failure}");
        out["incomplete"]["skipped_files"][0]
            .as_object_mut()
            .unwrap()
            .remove("reason");
        let expected = json!({
          "analysis": "fixture",
          "status": "findings",
          "complete": false,
          "findings": [{"file": "src/broken.rs"}, {"file": "src/lib.rs"}],
          "total": 3,
          "limit": 2,
          "truncated": true,
          "coverage": {
            "analyzed_files": 3,
            "languages": [
              {"language": "Rust", "files": 4, "supported": true},
              {"language": "TOML", "files": 1, "supported": false}
            ],
            "unrecognized_files": 2
          },
          "excluded": {
            "tests": [{"path": "src/lib.rs", "range": "10:1-14:2"}, {"path": "tests/cli.rs"}],
            "generated": [{"path": "src/gen.rs"}],
            "vendored": [{"path": "vendor/"}],
            "ignored": [{"path": "build-out/"}],
            "counts": {"tests": 2, "generated": 1, "vendored": 1, "ignored": 1},
            "truncated": false
          },
          "incomplete": {
            "skipped_files": [{"file": "src/big.rs", "kind": "size_limit"}],
            "parse_failures": [{"file": "src/broken.rs"}],
            "unresolved": [],
            "limits_hit": [],
            "counts": {"skipped_files": 1, "parse_failures": 1, "unresolved": 0, "limits_hit": 0},
            "truncated": false
          },
          "evidence": {"sources": ["tree-sitter"], "language_servers": []}
        });
        assert_eq!(out, expected);
    }

    #[test]
    fn missing_evidence_without_findings_is_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path(), false);
        let out = value(fixture(&request(dir.path(), &[], &[], 100)));
        assert_eq!(out["status"], "incomplete");
        assert_eq!(out["total"], 0);
        assert_eq!(out["incomplete"]["counts"]["skipped_files"], 1);
        assert_eq!(out["incomplete"]["counts"]["parse_failures"], 1);
    }

    #[test]
    fn one_unreported_file_is_no_findings() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/ok.rs", function("ok", false).as_bytes());
        let out = value(fixture(&request(dir.path(), &[], &[], 100)));
        assert_eq!(out["status"], "no_findings");
        assert_eq!(out["complete"], true);
        assert_eq!(out["total"], 0);
        assert_eq!(out["coverage"]["analyzed_files"], 1);
        assert_eq!(out["evidence"]["sources"], json!(["tree-sitter"]));
        assert_eq!(out["evidence"]["language_servers"], json!([]));
        assert_eq!(out["excluded"], no_lists());
    }

    #[test]
    fn named_unsupported_file_is_the_unsupported_example() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path(), true);
        let out = value(fixture(&request(dir.path(), &["Cargo.toml"], &[], 100)));
        let expected = json!({"analysis":"fixture","status":"unsupported","complete":true,"findings":[],"total":0,"limit":100,"truncated":false,
         "coverage":{"analyzed_files":0,"languages":[{"language":"TOML","files":1,"supported":false}],"unrecognized_files":0},
         "excluded":{"tests":[],"generated":[],"vendored":[],"ignored":[],"counts":{"tests":0,"generated":0,"vendored":0,"ignored":0},"truncated":false},
         "incomplete":{"skipped_files":[],"parse_failures":[],"unresolved":[],"limits_hit":[],"counts":{"skipped_files":0,"parse_failures":0,"unresolved":0,"limits_hit":0},"truncated":false},
         "evidence":{"sources":[],"language_servers":[]}});
        assert_eq!(out, expected);
    }

    #[test]
    fn supported_file_left_out_by_a_glob_is_the_empty_scope_error() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", b"[package]\nname = \"a\"\n");
        write(dir.path(), "src/ok.rs", function("ok", false).as_bytes());
        let s = empty_scope(fixture(&request(dir.path(), &[], &["src/*.rs"], 100)));
        assert_eq!(
            s,
            EmptyScope {
                tests: 0,
                generated: 0,
                vendored: 0,
                ignored: 0,
                filtered: 1,
                unrecognized: 0
            }
        );
    }

    #[test]
    fn unsupported_beside_supported_is_no_findings() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", b"[package]\nname = \"a\"\n");
        write(dir.path(), "src/ok.rs", function("ok", false).as_bytes());
        let out = value(fixture(&request(dir.path(), &[], &[], 100)));
        assert_eq!(out["status"], "no_findings");
        assert_eq!(
            out["coverage"]["languages"],
            json!([
                {"language": "Rust", "files": 1, "supported": true},
                {"language": "TOML", "files": 1, "supported": false}
            ])
        );
    }

    #[test]
    fn named_test_directory_is_the_empty_scope_message() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path(), true);
        let err = fixture(&request(dir.path(), &["tests"], &[], 100))
            .err()
            .unwrap();
        assert!(err.is_invalid_params());
        assert_eq!(
            err.to_string(),
            "no files to analyze in the requested scope: 1 excluded as tests, 0 as generated, 0 as vendored, 0 as ignored; 0 left out by include or exclude; 0 unrecognized. Re-run with the matching include option (tests, generated, vendored, ignored)."
        );
    }

    #[test]
    fn only_file_with_inner_cfg_test_is_the_empty_scope_error() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/t.rs", b"#![cfg(test)]\nfn t() {}\n");
        let s = empty_scope(fixture(&request(dir.path(), &[], &[], 100)));
        assert_eq!(
            s,
            EmptyScope {
                tests: 1,
                generated: 0,
                vendored: 0,
                ignored: 0,
                filtered: 0,
                unrecognized: 0
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn only_a_directory_symlink_outside_is_incomplete() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "x.rs", function("x", true).as_bytes());
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("shared")).unwrap();
        let out = value(fixture(&request(dir.path(), &[], &[], 100)));
        assert_eq!(out["status"], "incomplete");
        assert_eq!(out["complete"], false);
        assert_eq!(out["incomplete"]["skipped_files"][0]["file"], "shared/");
        assert_eq!(
            out["incomplete"]["skipped_files"][0]["kind"],
            "symlink_outside_workspace"
        );
        assert_eq!(out["coverage"]["languages"], json!([]));
    }

    #[test]
    fn limit_is_checked_before_paths() {
        let dir = tempfile::tempdir().unwrap();
        let result = Request::new(
            dir.path(),
            &["missing.rs".to_string()],
            &[],
            &[],
            Includes::default(),
            1001,
        );
        assert!(matches!(result, Err(AnalyzeError::LimitOutOfRange(1001))));
    }

    /// A test analyzer whose behavior each test sets.
    struct Probe {
        called: AtomicBool,
        cancel_inside: bool,
        limit_hit: bool,
        /// The most files the probe pulls.
        pull: usize,
    }

    impl Probe {
        fn new(cancel_inside: bool, limit_hit: bool) -> Probe {
            Probe {
                called: AtomicBool::new(false),
                cancel_inside,
                limit_hit,
                pull: usize::MAX,
            }
        }

        fn stopping_after(pull: usize, limit_hit: bool) -> Probe {
            Probe {
                pull,
                ..Probe::new(false, limit_hit)
            }
        }
    }

    impl Analyzer for Probe {
        const NAME: &'static str = "probe";
        const HINT: &'static str = "";
        type Finding = FixtureFinding;

        fn supports(&self, grammar: Grammar) -> bool {
            grammar == Grammar::Rust
        }

        fn analyze(
            &self,
            files: &mut dyn FileSource,
            cancel: &CancellationToken,
        ) -> Result<Analysis<FixtureFinding>, AnalyzeError> {
            self.called.store(true, Ordering::SeqCst);
            let mut pulled = 0;
            while pulled < self.pull && files.next_file()?.is_some() {
                pulled += 1;
            }
            if self.cancel_inside {
                cancel.cancel();
            }
            let limits_hit = if self.limit_hit {
                vec![LimitHit {
                    limit: "max_depth".to_string(),
                    value: 3,
                    detail: "stopped at depth 3".to_string(),
                }]
            } else {
                vec![]
            };
            Ok(Analysis {
                findings: vec![],
                unresolved: vec![],
                limits_hit,
                details: None,
            })
        }
    }

    fn ok_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/ok.rs", function("ok", false).as_bytes());
        dir
    }

    #[test]
    fn cancelled_token_stops_before_the_analyzer() {
        let dir = ok_workspace();
        let probe = Probe::new(false, false);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = run(&probe, &request(dir.path(), &[], &[], 100), &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
        assert!(!probe.called.load(Ordering::SeqCst));
    }

    #[test]
    fn cancel_during_the_analysis_is_cancelled() {
        let dir = ok_workspace();
        let probe = Probe::new(true, false);
        let cancel = CancellationToken::new();
        let result = run(&probe, &request(dir.path(), &[], &[], 100), &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
        assert!(probe.called.load(Ordering::SeqCst));
    }

    #[test]
    fn limit_hit_without_findings_is_incomplete() {
        let dir = ok_workspace();
        let probe = Probe::new(false, true);
        let request = request(dir.path(), &[], &[], 100);
        let out = value(run(&probe, &request, &CancellationToken::new()));
        assert_eq!(out["analysis"], "probe");
        assert_eq!(out["status"], "incomplete");
        assert_eq!(
            out["incomplete"]["limits_hit"],
            json!([{"limit": "max_depth", "value": 3, "detail": "stopped at depth 3"}])
        );
    }

    /// `src/a.rs` analyzed, `src/b.rs` over the size cap, `src/c.rs` with a
    /// parse failure and `src/d.rs` a test file.
    fn four_files() -> tempfile::TempDir {
        let dir = ok_workspace();
        write(dir.path(), "src/a.rs", function("a", false).as_bytes());
        write(dir.path(), "src/b.rs", &rust_of_len(8_388_609));
        write(dir.path(), "src/c.rs", b"fn c() {\n    let x = ;\n}\n");
        write(dir.path(), "src/d.rs", b"#![cfg(test)]\nfn t() {}\n");
        fs::remove_file(dir.path().join("src/ok.rs")).unwrap();
        dir
    }

    fn failed(result: Result<Envelope<FixtureFinding>, AnalyzeError>) -> String {
        match result {
            Err(err @ AnalyzeError::Failed { .. }) => err.to_string(),
            Err(other) => panic!("expected a failure, got {other}"),
            Ok(_) => panic!("expected a failure, got a result"),
        }
    }

    #[test]
    fn analyzer_that_pulls_no_file_without_a_limit_fails() {
        let dir = ok_workspace();
        let probe = Probe::stopping_after(0, false);
        let request = request(dir.path(), &[], &[], 100);
        assert_eq!(
            failed(run(&probe, &request, &CancellationToken::new())),
            "probe failed: stopped before src/ok.rs without reporting a limit"
        );
    }

    #[test]
    fn analyzer_that_pulls_no_file_with_a_limit_is_incomplete() {
        let dir = ok_workspace();
        let probe = Probe::stopping_after(0, true);
        let request = request(dir.path(), &[], &[], 100);
        let out = value(run(&probe, &request, &CancellationToken::new()));
        assert_eq!(out["status"], "incomplete");
        assert_eq!(out["coverage"]["analyzed_files"], 0);
        assert_eq!(
            out["coverage"]["languages"],
            json!([{"language": "Rust", "files": 1, "supported": true}])
        );
    }

    #[test]
    fn analyzer_that_stops_early_without_a_limit_fails() {
        let dir = four_files();
        let probe = Probe::stopping_after(1, false);
        let request = request(dir.path(), &[], &[], 100);
        assert_eq!(
            failed(run(&probe, &request, &CancellationToken::new())),
            "probe failed: stopped before src/c.rs without reporting a limit"
        );
    }

    #[test]
    fn analyzer_that_stops_early_with_a_limit_keeps_all_evidence() {
        let dir = four_files();
        let probe = Probe::stopping_after(1, true);
        let request = request(dir.path(), &[], &[], 100);
        let out = value(run(&probe, &request, &CancellationToken::new()));
        assert_eq!(out["status"], "incomplete");
        assert_eq!(out["complete"], false);
        assert_eq!(out["coverage"]["analyzed_files"], 1);
        assert_eq!(
            out["coverage"]["languages"],
            json!([{"language": "Rust", "files": 3, "supported": true}])
        );
        assert_eq!(out["incomplete"]["skipped_files"][0]["file"], "src/b.rs");
        assert_eq!(out["incomplete"]["skipped_files"][0]["kind"], "size_limit");
        assert_eq!(out["incomplete"]["parse_failures"][0]["file"], "src/c.rs");
        assert_eq!(out["excluded"]["tests"], json!([{"path": "src/d.rs"}]));
        assert_eq!(out["incomplete"]["counts"]["limits_hit"], 1);
    }

    /// Asks the reader for a file it never returned.
    struct Rereading;

    impl Analyzer for Rereading {
        const NAME: &'static str = "fixture";
        const HINT: &'static str = "";
        type Finding = FixtureFinding;

        fn supports(&self, grammar: Grammar) -> bool {
            grammar == Grammar::Rust
        }

        fn analyze(
            &self,
            files: &mut dyn FileSource,
            _cancel: &CancellationToken,
        ) -> Result<Analysis<FixtureFinding>, AnalyzeError> {
            files.reread("nope.rs")?;
            unreachable!("the reread fails")
        }
    }

    #[test]
    fn failure_inside_the_run_names_the_analyzer() {
        let dir = ok_workspace();
        let request = request(dir.path(), &[], &[], 100);
        let message = failed(run(&Rereading, &request, &CancellationToken::new()));
        assert!(message.starts_with("fixture failed: "), "{message}");
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

    /// A file source that records the resident set after each file it gives.
    #[cfg(unix)]
    struct Recording<'a> {
        inner: &'a mut dyn FileSource,
        samples: &'a Mutex<Vec<u64>>,
    }

    #[cfg(unix)]
    impl FileSource for Recording<'_> {
        fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
            let file = self.inner.next_file();
            self.samples.lock().unwrap().push(rss_kib());
            file
        }

        fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
            self.inner.reread(rel)
        }
    }

    /// The fixture analyzer, measured on each file it pulls.
    #[cfg(unix)]
    struct Measured {
        samples: Mutex<Vec<u64>>,
    }

    #[cfg(unix)]
    impl Analyzer for Measured {
        const NAME: &'static str = "fixture";
        const HINT: &'static str = "";
        type Finding = FixtureFinding;

        fn supports(&self, grammar: Grammar) -> bool {
            FixtureAnalyzer.supports(grammar)
        }

        fn analyze(
            &self,
            files: &mut dyn FileSource,
            cancel: &CancellationToken,
        ) -> Result<Analysis<FixtureFinding>, AnalyzeError> {
            let mut recording = Recording {
                inner: files,
                samples: &self.samples,
            };
            FixtureAnalyzer.analyze(&mut recording, cancel)
        }
    }

    #[cfg(unix)]
    #[test]
    fn analysis_holds_one_file_at_a_time() {
        // Other tests of the process allocate at the same time, so the
        // measurement runs in a child process that runs only this test.
        const NAME: &str = "analyze::run::tests::analysis_holds_one_file_at_a_time";
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
        for n in 0..3 {
            let mut text = String::new();
            let mut i = 0;
            while text.len() < 1 << 20 {
                text.push_str(&format!("fn f{i}(a: u32) -> u32 {{ a + {i} }}\n"));
                i += 1;
            }
            write(dir.path(), &format!("src/f{n}.rs"), text.as_bytes());
        }
        // The allocator keeps the pages of the first parse; analyze one file
        // first so that only growth by retained files counts.
        fixture(&request(dir.path(), &["src/f0.rs"], &[], 100)).unwrap();
        let request = request(dir.path(), &[], &[], 100);
        let baseline = rss_kib();
        let measured = Measured {
            samples: Mutex::new(Vec::new()),
        };
        let out = value(run(&measured, &request, &CancellationToken::new()));
        assert_eq!(out["coverage"]["analyzed_files"], 3);
        let samples = measured.samples.into_inner().unwrap();
        assert_eq!(samples.len(), 4);
        let peak = samples.iter().copied().max().unwrap();
        assert!(
            peak <= baseline + 4096,
            "resident set grew from {baseline} KiB to {peak} KiB: {samples:?}"
        );
    }
}
