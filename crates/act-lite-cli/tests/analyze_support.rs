//! The runner's inputs beyond the files in scope: removed test-region text,
//! support reads, resolution inputs, directory listings, the workspace walk,
//! focus files and the hook after the empty-scope check.

use act_lite_cli::analyze::discovery::{Candidate, Includes, discover, walk_workspace};
use act_lite_cli::analyze::envelope::{AnalyzeError, Finding, Location};
use act_lite_cli::analyze::focus::{FocusSet, check_focus, focus_files};
use act_lite_cli::analyze::read::{
    EntryKind, FileSource, ReadOutcome, ReadResult, Reader, SupportError, SupportReader,
    SymlinkTarget, read_file,
};
use act_lite_cli::analyze::run::{Analysis, Analyzer, Request, RunContext, run};
use act_lite_parser::Grammar;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio_util::sync::CancellationToken;

const CAP: usize = 8 * 1024 * 1024;

fn write(root: &Path, rel: &str, content: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn token() -> CancellationToken {
    CancellationToken::new()
}

fn request(root: &Path, exclude: &[&str]) -> Request {
    let exclude: Vec<String> = exclude.iter().map(|s| s.to_string()).collect();
    Request::new(root, &[], &[], &exclude, Includes::default(), 100).unwrap()
}

fn is_js_or_ts(grammar: Grammar) -> bool {
    matches!(
        grammar,
        Grammar::JavaScript | Grammar::Jsx | Grammar::TypeScript | Grammar::Tsx
    )
}

const WITH_TEST_MOD: &str = "fn a() {}\n#[cfg(test)]\nmod tests;\n";

#[test]
fn removed_regions_keep_the_text_of_blanked_test_code() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/lib.rs", WITH_TEST_MOD.as_bytes());
    let file = Candidate {
        path: ws.path().join("src/lib.rs"),
        rel: "src/lib.rs".to_string(),
        grammar: Grammar::Rust,
    };
    let ReadResult::Parsed { parsed, .. } = read_file(&file, false).unwrap() else {
        panic!("src/lib.rs is read");
    };
    assert_eq!(parsed.removed.len(), 1);
    assert_eq!(parsed.removed[0].text, "#[cfg(test)]\nmod tests;");
    assert_eq!(parsed.removed[0].range.to_string(), "2:1-3:11");
    assert!(!parsed.source.content().contains("mod tests"));
    let ReadResult::Parsed { parsed, .. } = read_file(&file, true).unwrap() else {
        panic!("src/lib.rs is read");
    };
    assert!(parsed.removed.is_empty());
}

#[test]
fn support_reads_have_the_read_bounds_and_stay_in_the_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    write(&root, "other/x.rs", WITH_TEST_MOD.as_bytes());
    let mut big = b"fn a() {}\n//".to_vec();
    big.resize(CAP, b'x');
    big.push(b'\n');
    write(&root, "big.rs", &big);
    write(&root, "latin.rs", b"fn a() {} // \xff\n");
    write(tmp.path(), "outside.rs", b"fn o() {}\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tmp.path().join("outside.rs"), root.join("link.rs")).unwrap();

    let support = SupportReader::new(&root, &Includes::default(), &token());
    let parsed = support.source("other/x.rs").unwrap();
    assert_eq!(parsed.rel, "other/x.rs");
    assert_eq!(parsed.removed.len(), 1, "blanked as an analyzed read is");
    assert!(matches!(
        support.source("big.rs"),
        Err(SupportError::TooLarge(_))
    ));
    assert!(matches!(
        support.source("latin.rs"),
        Err(SupportError::NotUtf8(_))
    ));
    assert!(matches!(
        support.source("../outside.rs"),
        Err(SupportError::Refused(_))
    ));
    #[cfg(unix)]
    assert!(matches!(
        support.source("link.rs"),
        Err(SupportError::Refused(_))
    ));
    assert!(matches!(
        support.source("nope.rs"),
        Err(SupportError::NotFound(_))
    ));
    assert_eq!(support.text("other/x.rs").unwrap(), WITH_TEST_MOD);

    let cancelled = token();
    cancelled.cancel();
    let support = SupportReader::new(&root, &Includes::default(), &cancelled);
    assert!(matches!(
        support.source("other/x.rs"),
        Err(SupportError::Cancelled)
    ));
    assert!(matches!(
        support.text("other/x.rs"),
        Err(SupportError::Cancelled)
    ));
    assert!(matches!(
        support.list_dir("other"),
        Err(SupportError::Cancelled)
    ));

    let error = SupportReader::new(&root, &Includes::default(), &token())
        .source("big.rs")
        .err()
        .unwrap()
        .to_string();
    assert!(error.starts_with("big.rs: "), "{error}");
}

#[test]
fn support_reads_refuse_pruned_directories_and_add_no_evidence() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "node_modules/x.js", b"export const x = 1;\n");
    write(ws.path(), "lib/broken.rs", b"fn a( {\n");
    write(ws.path(), "src/a.rs", b"fn a() {}\n");
    let support = SupportReader::new(ws.path(), &Includes::default(), &token());
    assert!(matches!(
        support.source("node_modules/x.js"),
        Err(SupportError::Refused(_))
    ));
    let mut reader = Reader::new(
        &discover(
            ws.path(),
            &request(ws.path(), &[]).scope,
            &|g| g == Grammar::Rust,
            &token(),
        )
        .unwrap()
        .files
        .into_iter()
        .filter(|f| f.rel == "src/a.rs")
        .collect::<Vec<_>>(),
        &|g| g == Grammar::Rust,
        false,
        &token(),
    );
    while reader.next_file().unwrap().is_some() {}
    let parsed = support.source("lib/broken.rs").unwrap();
    assert!(parsed.source.tree().root_node().has_error());
    assert!(reader.finish().parse_failures.is_empty());
}

#[test]
fn list_dir_gives_names_byte_for_byte() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/b.ts", b"export const b = 1;\n");
    write(ws.path(), "node_modules/p/package.json", b"{}\n");
    let support = SupportReader::new(ws.path(), &Includes::default(), &token());
    let entries = support.list_dir("src").unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["b.ts"]);
    assert!(!names.contains(&"B.ts"));
    assert_eq!(entries[0].kind, EntryKind::File);
    let pruned = support.list_dir("node_modules").unwrap();
    assert_eq!(pruned[0].name, "p");
    assert_eq!(pruned[0].kind, EntryKind::Dir);
    assert!(matches!(
        support.list_dir(".."),
        Err(SupportError::Refused(_))
    ));
}

#[test]
fn the_workspace_walk_prunes_like_discovery() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "vendor/x.js", b"x\n");
    write(ws.path(), "src/a.ts", b"a\n");
    write(ws.path(), "tests/t.ts", b"t\n");
    write(ws.path(), "a-b/c.ts", b"c\n");
    write(ws.path(), ".git/config", b"\n");
    let files = walk_workspace(ws.path(), &Includes::default(), &token())
        .unwrap()
        .files;
    assert_eq!(files, ["a-b/c.ts", "src/a.ts", "tests/t.ts"]);
}

#[test]
fn focus_paths_are_checked_with_the_clones_texts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    write(&root, "a.py", b"x = 1\n");
    write(&root, "sub/s.py", b"y = 2\n");
    write(tmp.path(), "x.py", b"z = 3\n");
    let req = request(&root, &[]);
    let missing = check_focus(&root, &req.scope, &["nope.py".to_string()])
        .unwrap_err()
        .to_string();
    assert!(
        missing.starts_with("The path 'nope.py' does not exist."),
        "{missing}"
    );
    assert_eq!(
        check_focus(&root, &req.scope, &["../x.py".to_string()])
            .unwrap_err()
            .to_string(),
        "path must be inside the workspace root: ../x.py"
    );
    let sub = Request::new(
        &root,
        &["sub".to_string()],
        &[],
        &[],
        Includes::default(),
        100,
    )
    .unwrap();
    assert_eq!(
        check_focus(&root, &sub.scope, &["a.py".to_string()])
            .unwrap_err()
            .to_string(),
        "focus path is not inside the requested paths: a.py"
    );
    let ok = check_focus(&root, &req.scope, &["sub".to_string(), "a.py".to_string()]).unwrap();
    assert_eq!(ok, [root.join("sub"), root.join("a.py")]);
}

/// The focus files of `focus` in a JS/TS analysis of `root`.
fn focus_of(root: &Path, focus: &[&str], exclude: &[&str]) -> FocusSet {
    let req = request(root, exclude);
    let focus: Vec<String> = focus.iter().map(|s| s.to_string()).collect();
    let paths = check_focus(root, &req.scope, &focus).unwrap();
    let discovered = discover(root, &req.scope, &is_js_or_ts, &token()).unwrap();
    focus_files(
        root,
        &paths,
        &discovered,
        &Includes::default(),
        &is_js_or_ts,
        &token(),
    )
    .unwrap()
}

fn reasons(set: &FocusSet) -> BTreeMap<String, Option<String>> {
    set.files
        .iter()
        .map(|f| (f.rel.clone(), f.reason.clone()))
        .collect()
}

#[test]
fn focus_files_follow_discovery_pruning() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/a.ts", b"export const a = 1;\n");
    write(ws.path(), "src/style.css", b".x {}\n");
    write(ws.path(), "src/a.test.ts", b"test\n");
    write(ws.path(), "node_modules/p/i.js", b"i\n");
    fs::create_dir_all(ws.path().join("empty")).unwrap();

    let set = focus_of(ws.path(), &["src"], &[]);
    assert_eq!(set.paths, ["src/"]);
    assert_eq!(
        reasons(&set),
        BTreeMap::from([
            (
                "src/a.test.ts".to_string(),
                Some("excluded as tests".to_string())
            ),
            ("src/a.ts".to_string(), None),
            (
                "src/style.css".to_string(),
                Some("language not supported: CSS".to_string())
            ),
        ])
    );
    assert!(
        set.files
            .iter()
            .find(|f| f.rel == "src/a.ts")
            .unwrap()
            .analyzed
    );
    assert!(set.paths_not_analyzed.is_empty());

    let set = focus_of(ws.path(), &["node_modules/p/i.js"], &[]);
    assert_eq!(
        reasons(&set),
        BTreeMap::from([(
            "node_modules/p/i.js".to_string(),
            Some("excluded as vendored".to_string())
        )])
    );
    assert_eq!(set.paths_not_analyzed, ["node_modules/p/i.js"]);

    let set = focus_of(ws.path(), &["empty"], &[]);
    assert!(set.files.is_empty());
    assert_eq!(set.paths_not_analyzed, ["empty/"]);
}

#[test]
fn every_focus_reason_occurs() {
    let ws = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws.path().join(".git")).unwrap();
    write(ws.path(), ".gitignore", b"ign/\n");
    write(ws.path(), "a.ts", b"export const a = 1;\n");
    write(ws.path(), "a.test.ts", b"t\n");
    write(ws.path(), "x.min.js", b"m\n");
    write(ws.path(), "vendor/v.js", b"v\n");
    write(ws.path(), "ign/i.js", b"i\n");
    write(ws.path(), "skip.ts", b"s\n");
    write(ws.path(), "notes.txt", b"n\n");
    write(ws.path(), "style.css", b".x {}\n");
    let mut big = b"export const a = 1;\n//".to_vec();
    big.resize(CAP + 1, b'x');
    write(ws.path(), "big.ts", &big);

    let req = request(ws.path(), &["skip.ts"]);
    let discovered = discover(ws.path(), &req.scope, &is_js_or_ts, &token()).unwrap();
    // The pruned directories are not walked below `.`; their files are focus
    // files only when named.
    let named = [".", "vendor/v.js", "ign/i.js"].map(String::from);
    let paths = check_focus(ws.path(), &req.scope, &named).unwrap();
    let mut set = focus_files(
        ws.path(),
        &paths,
        &discovered,
        &Includes::default(),
        &is_js_or_ts,
        &token(),
    )
    .unwrap();
    let mut reader = Reader::new(&discovered.files, &is_js_or_ts, false, &token());
    while reader.next_file().unwrap().is_some() {}
    let read: ReadOutcome = reader.finish();
    set.settle(&read);
    let got: Vec<String> = set.files.iter().filter_map(|f| f.reason.clone()).collect();
    for want in [
        "excluded as tests",
        "excluded as generated",
        "excluded as vendored",
        "excluded as ignored",
        "left out by include or exclude",
        "unrecognized",
        "language not supported: CSS",
        "skipped: size_limit",
    ] {
        assert!(got.iter().any(|r| r.as_str() == want), "{want} in {got:?}");
    }
    assert!(set.files.iter().find(|f| f.rel == "a.ts").unwrap().analyzed);
}

#[test]
fn settle_marks_what_the_read_left_out() {
    let ws = tempfile::tempdir().unwrap();
    let mut big = b"fn a() {}\n//".to_vec();
    big.resize(CAP + 1, b'x');
    write(ws.path(), "src/big.rs", &big);
    write(ws.path(), "src/t.rs", b"#![cfg(test)]\nfn t() {}\n");
    write(ws.path(), "src/ok.rs", b"fn ok() {}\n");
    let rust = |g: Grammar| g == Grammar::Rust;
    let req = request(ws.path(), &[]);
    let discovered = discover(ws.path(), &req.scope, &rust, &token()).unwrap();
    let paths = check_focus(ws.path(), &req.scope, &["src".to_string()]).unwrap();
    let mut set = focus_files(
        ws.path(),
        &paths,
        &discovered,
        &Includes::default(),
        &rust,
        &token(),
    )
    .unwrap();
    assert!(set.files.iter().all(|f| f.analyzed));
    let mut reader = Reader::new(&discovered.files, &rust, false, &token());
    while reader.next_file().unwrap().is_some() {}
    set.settle(&reader.finish());
    assert_eq!(
        reasons(&set),
        BTreeMap::from([
            (
                "src/big.rs".to_string(),
                Some("skipped: size_limit".to_string())
            ),
            ("src/ok.rs".to_string(), None),
            (
                "src/t.rs".to_string(),
                Some("excluded as tests".to_string())
            ),
        ])
    );
}

/// A Rust analyzer whose `after_scope` fails.
struct FailsAfterScope;

#[derive(Debug, Serialize)]
struct NoFinding {
    #[serde(flatten)]
    location: Location,
}

impl Finding for NoFinding {
    fn location(&self) -> &Location {
        &self.location
    }
    fn order(&self, other: &Self) -> Ordering {
        self.location.cmp(&other.location)
    }
}

impl Analyzer for FailsAfterScope {
    const NAME: &'static str = "after";
    const HINT: &'static str = "";
    type Finding = NoFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        grammar == Grammar::Rust
    }

    fn analyze(
        &self,
        files: &mut dyn FileSource,
        _cancel: &CancellationToken,
    ) -> Result<Analysis<NoFinding>, AnalyzeError> {
        while files.next_file()?.is_some() {}
        Ok(Analysis {
            findings: Vec::new(),
            unresolved: Vec::new(),
            limits_hit: Vec::new(),
            details: None,
            failed_inputs: None,
        })
    }

    fn after_scope(
        &self,
        ctx: &RunContext,
        _read: &ReadOutcome,
        _analysis: &mut Analysis<NoFinding>,
    ) -> Result<(), AnalyzeError> {
        assert!(ctx.root.exists());
        Err(AnalyzeError::Failed {
            analysis: "after",
            cause: "the hook ran".to_string(),
        })
    }
}

#[test]
fn after_scope_runs_after_the_empty_scope_check() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "tests/a.rs", b"fn a() {}\n");
    let empty = run(&FailsAfterScope, &request(ws.path(), &[]), &token()).unwrap_err();
    assert!(matches!(empty, AnalyzeError::EmptyScope(_)), "{empty}");
    write(ws.path(), "src/a.rs", b"fn a() {}\n");
    let hook = run(&FailsAfterScope, &request(ws.path(), &[]), &token()).unwrap_err();
    assert!(hook.to_string().contains("the hook ran"), "{hook}");
}

/// Every file below `root` with its content and modification time.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                stack.push(path.clone());
            }
            let content = if meta.is_file() {
                fs::read(&path).unwrap()
            } else {
                Vec::new()
            };
            out.insert(path, (content, meta.modified().unwrap()));
        }
    }
    out
}

#[test]
fn support_reads_write_nothing() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "src/lib.rs", WITH_TEST_MOD.as_bytes());
    write(ws.path(), "package.json", b"{\"name\":\"p\"}\n");
    let before = snapshot(ws.path());
    let support = SupportReader::new(ws.path(), &Includes::default(), &token());
    support.source("src/lib.rs").unwrap();
    support.text("package.json").unwrap();
    support.list_dir("src").unwrap();
    walk_workspace(ws.path(), &Includes::default(), &token()).unwrap();
    assert_eq!(snapshot(ws.path()), before);
}

// ---- review round 2 ----

/// The workspace of the round-2 cases: ignored files, a whole-file test, a
/// FIFO, symlinks out of the workspace and broken, an unlistable directory,
/// a vendored directory with a nested `node_modules`, and a `.git` file.
#[cfg(unix)]
fn round2(tmp: &Path) -> PathBuf {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let ws = tmp.join("ws");
    fs::create_dir_all(ws.join(".git")).unwrap();
    write(&ws, ".gitignore", b"secret.json\nign.log\n");
    write(&ws, "secret.json", b"{}\n");
    write(&ws, "ign.log", b"x\n");
    write(&ws, "src/a.ts", b"export const a = 1;\n");
    write(&ws, "src/inner.ts", b"export const i = 1;\n");
    write(&ws, "src/t.rs", b"#![cfg(test)]\nfn t() {}\n");
    write(tmp, "out.ts", b"o\n");
    symlink(tmp.join("out.ts"), ws.join("src/out.ts")).unwrap();
    symlink(ws.join("missing.ts"), ws.join("src/broken.ts")).unwrap();
    symlink(ws.join("src/inner.ts"), ws.join("src/in.ts")).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(ws.join("src/pipe.ts"))
            .status()
            .unwrap()
            .success()
    );
    write(&ws, "locked/x.ts", b"x\n");
    fs::set_permissions(ws.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    write(&ws, "vendor/lib/a.js", b"a\n");
    write(&ws, "vendor/lib/node_modules/q/x.js", b"x\n");
    write(&ws, "src/sub/.git", b"gitdir: x\n");
    ws
}

#[cfg(unix)]
fn unlock(ws: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(ws.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn walk_workspace_returns_walk_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let walked = walk_workspace(&ws, &Includes::default(), &token()).unwrap();
    let (files, skipped) = (walked.files, walked.skipped);
    unlock(&ws);
    assert!(!files.iter().any(|f| f.starts_with("locked/")), "{files:?}");
    assert!(!files.contains(&"secret.json".to_string()), "{files:?}");
    let locked: Vec<_> = skipped
        .iter()
        .filter(|s| s.file.file == "locked/")
        .map(|s| serde_json::to_value(s.file.kind).unwrap())
        .collect();
    assert_eq!(locked, [serde_json::json!("walk_error")], "{skipped:?}");
}

#[test]
fn a_cancelled_walk_stops() {
    let ws = tempfile::tempdir().unwrap();
    write(ws.path(), "a/b/c.ts", b"c\n");
    let cancelled = token();
    cancelled.cancel();
    assert!(matches!(
        walk_workspace(ws.path(), &Includes::default(), &cancelled),
        Err(AnalyzeError::Cancelled)
    ));
}

#[cfg(unix)]
#[test]
fn list_dir_tells_symlink_targets_apart() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let support = SupportReader::new(&ws, &Includes::default(), &token());
    let entries = support.list_dir("src").unwrap();
    unlock(&ws);
    let target = |name: &str| {
        entries
            .iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("{name} in {entries:?}"))
            .target
            .clone()
    };
    assert_eq!(target("out.ts"), Some(SymlinkTarget::Outside));
    assert_eq!(target("broken.ts"), Some(SymlinkTarget::Broken));
    assert_eq!(
        target("in.ts"),
        Some(SymlinkTarget::Inside("src/inner.ts".to_string()))
    );
    assert_eq!(target("a.ts"), None);
}

#[cfg(unix)]
#[test]
fn a_whole_file_test_is_a_test_file_error() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let support = SupportReader::new(&ws, &Includes::default(), &token());
    let err = support.source("src/t.rs").err().unwrap();
    unlock(&ws);
    assert_eq!(err, SupportError::TestFile("src/t.rs".to_string()));
    assert_eq!(err.to_string(), "src/t.rs: excluded as tests");
}

#[cfg(unix)]
#[test]
fn ignored_files_are_refused_unless_included() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let default = SupportReader::new(&ws, &Includes::default(), &token());
    let refused = default.text("secret.json");
    let included = Includes {
        ignored: true,
        ..Includes::default()
    };
    let allowed = SupportReader::new(&ws, &included, &token()).text("secret.json");
    unlock(&ws);
    assert!(
        matches!(refused, Err(SupportError::Refused(_))),
        "{refused:?}"
    );
    assert_eq!(allowed.unwrap(), "{}\n");
}

#[cfg(unix)]
#[test]
fn support_reads_honour_the_request_includes() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let default = SupportReader::new(&ws, &Includes::default(), &token());
    let refused = default.source("vendor/lib/a.js").err();
    let vendored = Includes {
        vendored: true,
        ..Includes::default()
    };
    let read = SupportReader::new(&ws, &vendored, &token())
        .source("vendor/lib/a.js")
        .map(|p| p.rel);
    unlock(&ws);
    assert!(
        matches!(refused, Some(SupportError::Refused(_))),
        "{refused:?}"
    );
    assert_eq!(read.unwrap(), "vendor/lib/a.js");
}

/// The focus files of `focus` in a JS/TS analysis of the round-2 workspace,
/// settled by a read.
#[cfg(unix)]
fn round2_focus(ws: &Path, focus: &[&str]) -> BTreeMap<String, Option<String>> {
    let req = request(ws, &[]);
    let focus: Vec<String> = focus.iter().map(|s| s.to_string()).collect();
    let paths = check_focus(ws, &req.scope, &focus).unwrap();
    let discovered = discover(ws, &req.scope, &is_js_or_ts, &token()).unwrap();
    let mut set = focus_files(
        ws,
        &paths,
        &discovered,
        &Includes::default(),
        &is_js_or_ts,
        &token(),
    )
    .unwrap();
    let mut reader = Reader::new(&discovered.files, &is_js_or_ts, false, &token());
    while reader.next_file().unwrap().is_some() {}
    set.settle(&reader.finish());
    reasons(&set)
}

#[cfg(unix)]
#[test]
fn a_named_pruned_directory_prunes_its_nested_pruned_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let found = round2_focus(&ws, &["vendor/lib"]);
    unlock(&ws);
    assert_eq!(
        found,
        BTreeMap::from([(
            "vendor/lib/a.js".to_string(),
            Some("excluded as vendored".to_string())
        )])
    );
}

#[cfg(unix)]
#[test]
fn focus_walk_follows_discovery_for_special_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = round2(tmp.path());
    let found = round2_focus(&ws, &["."]);
    unlock(&ws);
    assert!(!found.contains_key("src/sub/.git"), "{found:?}");
    assert_eq!(found["ign.log"], Some("excluded as ignored".to_string()));
    assert_eq!(
        found["src/pipe.ts"],
        Some("skipped: unreadable".to_string())
    );
    assert_eq!(
        found["src/out.ts"],
        Some("skipped: symlink_outside_workspace".to_string())
    );
    assert_eq!(found["locked/"], Some("skipped: walk_error".to_string()));
    assert!(
        !found.contains_key("src/in.ts"),
        "a link inside the workspace: {found:?}"
    );
}
