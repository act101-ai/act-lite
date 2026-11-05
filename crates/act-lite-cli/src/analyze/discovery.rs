//! Repository discovery: the files an analysis reads, and the entries it
//! leaves out.

use super::classify::{
    has_generated_header, is_generated_dir, is_generated_name, is_test_file, is_vendored_dir,
};
use super::envelope::{
    AnalyzeError, ExcludedEntry, ExcludedKind, Exclusion, SkipKind, SkippedFile, rel_path,
};
use super::glob::Glob;
use super::ignore::IgnoreRules;
use crate::mcp::path_error::enrich_not_found_error;
use act_lite_parser::Grammar;
use std::ffi::OsStr;
use std::fs::{self, FileType};
use std::io;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

/// The categories an analysis includes although discovery excludes them by
/// default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Includes {
    pub tests: bool,
    pub generated: bool,
    pub vendored: bool,
    pub ignored: bool,
}

/// The files and directories of a request, and its globs.
pub struct Scope {
    /// The named paths, resolved inside the workspace root.
    pub paths: Vec<PathBuf>,
    pub include: Vec<Glob>,
    pub exclude: Vec<Glob>,
    pub includes: Includes,
}

impl Scope {
    pub fn new(
        root: &Path,
        paths: &[String],
        include: &[String],
        exclude: &[String],
        includes: Includes,
    ) -> Result<Scope, AnalyzeError> {
        let canonical_root = canonical(root)?;
        let mut resolved = Vec::new();
        for input in paths {
            let joined = root.join(input);
            let Ok(target) = joined.canonicalize() else {
                return Err(AnalyzeError::NotFound(enrich_not_found_error(
                    input, &joined, root,
                )));
            };
            let Ok(rel) = target.strip_prefix(&canonical_root) else {
                return Err(AnalyzeError::OutsideRoot(input.clone()));
            };
            resolved.push(root.join(rel));
        }
        if paths.is_empty() {
            resolved.push(root.to_path_buf());
        }
        let globs = |patterns: &[String]| patterns.iter().map(|p| Glob::new(p)).collect();
        Ok(Scope {
            paths: resolved,
            include: globs(include),
            exclude: globs(exclude),
            includes,
        })
    }
}

/// `path` with every symlink and `..` resolved.
fn canonical(path: &Path) -> Result<PathBuf, AnalyzeError> {
    path.canonicalize().map_err(|err| AnalyzeError::Failed {
        analysis: "analyze",
        cause: format!("cannot resolve the workspace root: {err}"),
    })
}

/// A file in scope in a recognized language, supported or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub rel: String,
    pub grammar: Grammar,
}

/// An entry in scope that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    /// The language the entry counts in; `None` for a directory.
    pub grammar: Option<Grammar>,
    pub file: SkippedFile,
}

/// The result of discovery.
#[derive(Debug, Default)]
pub struct Discovered {
    /// Sorted by `rel`, each file once.
    pub files: Vec<Candidate>,
    pub excluded: Vec<Exclusion>,
    pub skipped: Vec<Skip>,
    pub unrecognized: u64,
    /// Entries left out by `include` or `exclude`.
    pub filtered: u64,
    /// Whether a file in a supported language was left out by a glob, as a
    /// test file, or as a generated file by name or header.
    pub removed_supported: bool,
}

/// Walks each path of `scope` below `root` and applies the discovery steps in
/// order: `.git` pruned; ignored, vendored and generated directories
/// excluded; the globs; unrecognized files counted; test files and generated
/// file names excluded; symlinks not followed; generated headers excluded.
/// The steps before the globs are also applied to the directories between
/// `root` and a named path. `supported` tells which languages the analysis
/// reads: only their files get a header read and a skip report.
pub fn discover(
    root: &Path,
    scope: &Scope,
    supported: &dyn Fn(Grammar) -> bool,
    cancel: &CancellationToken,
) -> Result<Discovered, AnalyzeError> {
    let mut walk = Walk {
        root,
        canonical_root: canonical(root)?,
        scope,
        supported,
        cancel,
        out: Discovered::default(),
        excluded: Vec::new(),
        skipped: Vec::new(),
    };
    walk.check_cancel()?;
    let mut named = scope.paths.clone();
    named.sort();
    named.dedup();
    for path in &named {
        if !named
            .iter()
            .any(|other| other != path && path.starts_with(other))
        {
            walk.named(path)?;
        }
    }
    let mut out = walk.out;
    out.files
        .sort_by(|a, b| (&a.rel, bytes(&a.path)).cmp(&(&b.rel, bytes(&b.path))));
    walk.excluded
        .sort_by(|(a, x), (b, y)| (&x.entry.path, bytes(a)).cmp(&(&y.entry.path, bytes(b))));
    walk.skipped
        .sort_by(|(a, x), (b, y)| (&x.file.file, bytes(a)).cmp(&(&y.file.file, bytes(b))));
    out.excluded = walk.excluded.into_iter().map(|(_, e)| e).collect();
    out.skipped = walk.skipped.into_iter().map(|(_, s)| s).collect();
    Ok(out)
}

/// The path as it is on disk: two names that differ only in invalid UTF-8
/// have the same written form and different bytes.
fn bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_encoded_bytes()
}

/// What steps 0–3 do with a real directory.
enum DirStep {
    Prune,
    Exclude(ExcludedKind),
    Walk,
}

struct Walk<'a> {
    root: &'a Path,
    canonical_root: PathBuf,
    scope: &'a Scope,
    supported: &'a dyn Fn(Grammar) -> bool,
    cancel: &'a CancellationToken,
    out: Discovered,
    /// Each entry with its path on disk, which orders entries whose written
    /// paths are equal.
    excluded: Vec<(PathBuf, Exclusion)>,
    skipped: Vec<(PathBuf, Skip)>,
}

impl Walk<'_> {
    fn check_cancel(&self) -> Result<(), AnalyzeError> {
        if self.cancel.is_cancelled() {
            return Err(AnalyzeError::Cancelled);
        }
        Ok(())
    }

    /// A named path: steps 0–3 on each directory between the root and the
    /// path, then the path itself as a walked entry.
    fn named(&mut self, path: &Path) -> Result<(), AnalyzeError> {
        let Ok(rel) = path.strip_prefix(self.root) else {
            return Err(AnalyzeError::Failed {
                analysis: "analyze",
                cause: format!(
                    "path {} is not inside the workspace root {}",
                    path.display(),
                    self.root.display()
                ),
            });
        };
        self.check_cancel()?;
        let mut rules = IgnoreRules::for_root(self.root);
        let parts: Vec<_> = rel.components().collect();
        let Some((last, ancestors)) = parts.split_last() else {
            return self.walk_dir(self.root, &rules);
        };
        let mut dir = self.root.to_path_buf();
        for part in ancestors {
            dir.push(part);
            match self.dir_step(&dir, &rules)? {
                DirStep::Prune => return Ok(()),
                DirStep::Exclude(kind) => {
                    let is_dir = path.is_dir();
                    if kind != ExcludedKind::Ignored || is_dir || Grammar::from_path(path).is_some()
                    {
                        self.exclude(kind, path, is_dir)?;
                    }
                    return Ok(());
                }
                DirStep::Walk => {
                    self.check_cancel()?;
                    rules = rules.child(&dir);
                }
            }
        }
        let path = dir.join(last);
        match fs::symlink_metadata(&path) {
            Ok(meta) => self.entry(&path, meta.file_type(), &rules),
            Err(err) => self.walk_error(&path, false, err),
        }
    }

    /// Steps 0–3 for `dir`; the token is checked before any marker is read.
    fn dir_step(&self, dir: &Path, rules: &IgnoreRules) -> Result<DirStep, AnalyzeError> {
        self.check_cancel()?;
        let includes = self.scope.includes;
        Ok(if dir.file_name() == Some(OsStr::new(".git")) {
            DirStep::Prune
        } else if !includes.ignored && rules.is_ignored(dir, true) {
            DirStep::Exclude(ExcludedKind::Ignored)
        } else if !includes.vendored && is_vendored_dir(dir) {
            DirStep::Exclude(ExcludedKind::Vendored)
        } else if !includes.generated && is_generated_dir(dir) {
            DirStep::Exclude(ExcludedKind::Generated)
        } else {
            DirStep::Walk
        })
    }

    fn walk_dir(&mut self, dir: &Path, rules: &IgnoreRules) -> Result<(), AnalyzeError> {
        self.check_cancel()?;
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => return self.walk_error(dir, true, err),
        };
        let mut listed = true;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    if listed {
                        self.walk_error(dir, true, err)?;
                    }
                    listed = false;
                    continue;
                }
            };
            let path = entry.path();
            match entry.file_type() {
                Ok(file_type) => self.entry(&path, file_type, rules)?,
                Err(err) => self.walk_error(&path, false, err)?,
            }
        }
        Ok(())
    }

    /// One entry of a directory that `rules` apply to.
    fn entry(
        &mut self,
        path: &Path,
        file_type: FileType,
        rules: &IgnoreRules,
    ) -> Result<(), AnalyzeError> {
        if file_type.is_dir() {
            return match self.dir_step(path, rules)? {
                DirStep::Prune => Ok(()),
                DirStep::Exclude(kind) => self.exclude(kind, path, true),
                DirStep::Walk => {
                    self.check_cancel()?;
                    self.walk_dir(path, &rules.child(path))
                }
            };
        }
        let Some(name) = path.file_name() else {
            return Ok(());
        };
        if name == ".git" {
            return Ok(());
        }
        let includes = self.scope.includes;
        let grammar = Grammar::from_path(path);
        if !includes.ignored && rules.is_ignored(path, false) {
            if grammar.is_some() {
                self.exclude(ExcludedKind::Ignored, path, false)?;
            }
            return Ok(());
        }
        let rel = rel_path(self.root, path, false)?;
        let supported = grammar.is_some_and(self.supported);
        if !self.in_globs(&rel) {
            self.out.filtered += 1;
            self.out.removed_supported |= supported;
            return Ok(());
        }
        if file_type.is_symlink() {
            return self.symlink(path, rel, grammar, supported);
        }
        let Some(grammar) = grammar else {
            self.out.unrecognized += 1;
            return Ok(());
        };
        let generated_name = is_generated_name(&name.to_string_lossy());
        if !includes.tests && is_test_file(&rel) {
            return self.remove(ExcludedKind::Tests, path, supported);
        }
        if !includes.generated && generated_name {
            return self.remove(ExcludedKind::Generated, path, supported);
        }
        if supported && !includes.generated && file_type.is_file() {
            self.check_cancel()?;
            match has_generated_header(path) {
                Ok(true) => return self.remove(ExcludedKind::Generated, path, true),
                Ok(false) => {}
                Err(err) => {
                    let reason = format!("cannot read file header: {err}");
                    self.skip(path, Some(grammar), rel, SkipKind::Unreadable, reason);
                    return Ok(());
                }
            }
        }
        let utf8 = path
            .strip_prefix(self.root)
            .ok()
            .and_then(Path::to_str)
            .is_some();
        if supported && !utf8 {
            let reason = "file path is not valid UTF-8".to_string();
            self.skip(path, Some(grammar), rel, SkipKind::NotUtf8, reason);
            return Ok(());
        }
        self.out.files.push(Candidate {
            path: path.to_path_buf(),
            rel,
            grammar,
        });
        Ok(())
    }

    fn in_globs(&self, rel: &str) -> bool {
        let scope = self.scope;
        (scope.include.is_empty() || scope.include.iter().any(|g| g.is_match(rel)))
            && !scope.exclude.iter().any(|g| g.is_match(rel))
    }

    /// A symlink is not followed. Reported: a link to a directory outside the
    /// workspace, and a link named in a supported language that points
    /// outside the workspace or is broken.
    fn symlink(
        &mut self,
        path: &Path,
        rel: String,
        grammar: Option<Grammar>,
        supported: bool,
    ) -> Result<(), AnalyzeError> {
        let target = fs::metadata(path).and_then(|meta| Ok((meta.is_dir(), path.canonicalize()?)));
        match target {
            Err(err) if supported => {
                let reason = format!("symlink target unreadable: {err}");
                self.skip(path, grammar, rel, SkipKind::Unreadable, reason);
            }
            Err(_) => {}
            Ok((_, target)) if target.starts_with(&self.canonical_root) => {}
            Ok((true, _)) => {
                let rel = rel_path(self.root, path, true)?;
                let reason = "symlink to a directory outside the workspace, not followed";
                self.skip(
                    path,
                    None,
                    rel,
                    SkipKind::SymlinkOutsideWorkspace,
                    reason.to_string(),
                );
            }
            Ok((false, _)) if supported => {
                let reason = "symlink to a file outside the workspace, not followed";
                self.skip(
                    path,
                    grammar,
                    rel,
                    SkipKind::SymlinkOutsideWorkspace,
                    reason.to_string(),
                );
            }
            Ok(_) => {}
        }
        Ok(())
    }

    /// Excludes a file by a file-level step.
    fn remove(
        &mut self,
        kind: ExcludedKind,
        path: &Path,
        supported: bool,
    ) -> Result<(), AnalyzeError> {
        self.out.removed_supported |= supported;
        self.exclude(kind, path, false)
    }

    fn exclude(
        &mut self,
        kind: ExcludedKind,
        path: &Path,
        is_dir: bool,
    ) -> Result<(), AnalyzeError> {
        let rel = rel_path(self.root, path, is_dir)?;
        let exclusion = Exclusion {
            kind,
            entry: ExcludedEntry {
                path: rel,
                range: None,
            },
        };
        self.excluded.push((path.to_path_buf(), exclusion));
        Ok(())
    }

    fn walk_error(
        &mut self,
        path: &Path,
        is_dir: bool,
        err: io::Error,
    ) -> Result<(), AnalyzeError> {
        let file = rel_path(self.root, path, is_dir)?;
        let reason = format!("walk error: {err}");
        self.skip(path, None, file, SkipKind::WalkError, reason);
        Ok(())
    }

    fn skip(
        &mut self,
        path: &Path,
        grammar: Option<Grammar>,
        file: String,
        kind: SkipKind,
        reason: String,
    ) {
        let skip = Skip {
            grammar,
            file: SkippedFile { file, kind, reason },
        };
        self.skipped.push((path.to_path_buf(), skip));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    const BIG_BYTES: usize = 8_388_609;

    /// The `conv` workspace: Rust sources, a generated file, a test file, a
    /// vendored and an ignored directory, and two unrecognized files.
    fn conv(root: &Path) {
        fs::create_dir_all(root.join(".git")).unwrap();
        write(root, ".gitignore", b"build-out/\n*.log\n");
        write(root, "Cargo.toml", b"[package]\nname = \"conv\"\n");
        write(
            root,
            "src/lib.rs",
            b"fn a() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n    let w = 4;\n    let v = 5;\n    let u = 6;\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
        );
        let mut big = b"fn big() {}\n".to_vec();
        big.resize(BIG_BYTES, b' ');
        write(root, "src/big.rs", &big);
        write(
            root,
            "src/broken.rs",
            b"fn b() {\n    let a = 1;\n    let b = 2;\n    let c = ;\n    let d = 4;\n}\n",
        );
        write(
            root,
            "src/more.rs",
            b"fn m() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let f = 6;\n}\n",
        );
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

    fn rust(g: Grammar) -> bool {
        g == Grammar::Rust
    }

    fn scope_with(
        root: &Path,
        paths: &[&str],
        include: &[&str],
        exclude: &[&str],
        includes: Includes,
    ) -> Result<Scope, AnalyzeError> {
        let owned = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Scope::new(
            root,
            &owned(paths),
            &owned(include),
            &owned(exclude),
            includes,
        )
    }

    fn run_with(
        root: &Path,
        paths: &[&str],
        include: &[&str],
        exclude: &[&str],
        includes: Includes,
    ) -> Discovered {
        let scope = scope_with(root, paths, include, exclude, includes).unwrap();
        discover(root, &scope, &rust, &CancellationToken::new()).unwrap()
    }

    fn run(root: &Path, paths: &[&str]) -> Discovered {
        run_with(root, paths, &[], &[], Includes::default())
    }

    fn rels(d: &Discovered) -> Vec<&str> {
        d.files.iter().map(|c| c.rel.as_str()).collect()
    }

    fn excluded(d: &Discovered) -> Vec<(ExcludedKind, &str)> {
        let mut out: Vec<_> = d
            .excluded
            .iter()
            .map(|e| (e.kind, e.entry.path.as_str()))
            .collect();
        out.sort_by_key(|(_, p)| *p);
        out
    }

    fn skipped(d: &Discovered) -> Vec<(&str, SkipKind, Option<Grammar>)> {
        d.skipped
            .iter()
            .map(|s| (s.file.file.as_str(), s.file.kind, s.grammar))
            .collect()
    }

    /// No reported path or reason holds the workspace's absolute path.
    fn assert_no_absolute_path(d: &Discovered, root: &Path) {
        let forms = [
            root.to_string_lossy().into_owned(),
            root.canonicalize().unwrap().to_string_lossy().into_owned(),
        ];
        let texts = d.excluded.iter().map(|e| e.entry.path.as_str()).chain(
            d.skipped
                .iter()
                .flat_map(|s| [s.file.file.as_str(), s.file.reason.as_str()]),
        );
        for text in texts {
            for form in &forms {
                assert!(!text.contains(form.as_str()), "{text}");
            }
        }
    }

    #[test]
    fn conv_files_exclusions_and_unrecognized() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let d = run(dir.path(), &[]);
        assert_eq!(
            rels(&d),
            [
                "Cargo.toml",
                "src/big.rs",
                "src/broken.rs",
                "src/lib.rs",
                "src/more.rs"
            ]
        );
        assert_eq!(
            excluded(&d),
            [
                (ExcludedKind::Ignored, "build-out/"),
                (ExcludedKind::Generated, "src/gen.rs"),
                (ExcludedKind::Tests, "tests/cli.rs"),
                (ExcludedKind::Vendored, "vendor/"),
            ]
        );
        assert_eq!(d.unrecognized, 2);
        assert_eq!(d.filtered, 0);
        assert!(d.skipped.is_empty());
        let all = format!("{:?}", (&d.files, &d.excluded, &d.skipped));
        assert!(!all.contains("debug.log"));
        assert!(!all.contains(".git"));
        assert_eq!(d.files[0].grammar, Grammar::Toml);
        assert_no_absolute_path(&d, dir.path());
    }

    #[test]
    fn include_vendored_walks_vendor() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let includes = Includes {
            vendored: true,
            ..Includes::default()
        };
        let d = run_with(dir.path(), &[], &[], &[], includes);
        assert!(rels(&d).contains(&"vendor/dep/lib.rs"));
        assert!(
            !excluded(&d)
                .iter()
                .any(|(k, _)| *k == ExcludedKind::Vendored)
        );
    }

    #[test]
    fn excluded_directories_are_reported_whatever_the_globs() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let d = run_with(dir.path(), &[], &["src/**"], &[], Includes::default());
        assert_eq!(
            excluded(&d),
            [
                (ExcludedKind::Ignored, "build-out/"),
                (ExcludedKind::Generated, "src/gen.rs"),
                (ExcludedKind::Vendored, "vendor/"),
            ]
        );
        assert_eq!(
            rels(&d),
            ["src/big.rs", "src/broken.rs", "src/lib.rs", "src/more.rs"]
        );
        // .gitignore, Cargo.toml and tests/cli.rs.
        assert_eq!(d.filtered, 3);
        assert_eq!(d.unrecognized, 1);
    }

    #[test]
    fn include_and_exclude_globs() {
        let dir = tempfile::tempdir().unwrap();
        for rel in ["src/x.rs", "src/ax.rs", "src/a/b/x.rs", "lib.rs"] {
            write(dir.path(), rel, b"fn f() {}\n");
        }
        let with = |include: &[&str], exclude: &[&str]| {
            let d = run_with(dir.path(), &[], include, exclude, Includes::default());
            rels(&d).into_iter().map(String::from).collect::<Vec<_>>()
        };
        assert_eq!(with(&["src/**/x.rs"], &[]), ["src/a/b/x.rs", "src/x.rs"]);
        assert_eq!(
            with(&["*.rs"], &[]),
            ["lib.rs", "src/a/b/x.rs", "src/ax.rs", "src/x.rs"]
        );
        assert_eq!(with(&[], &["src/*.rs"]), ["lib.rs", "src/a/b/x.rs"]);
    }

    #[test]
    fn without_a_work_tree_ignore_rules_do_not_apply() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        fs::remove_dir(dir.path().join(".git")).unwrap();
        let d = run(dir.path(), &[]);
        assert!(rels(&d).contains(&"build-out/x.rs"));
    }

    #[test]
    fn named_path_under_an_excluded_directory_is_reported_itself() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let d = run(dir.path(), &["vendor/dep/lib.rs"]);
        assert!(d.files.is_empty());
        assert_eq!(
            excluded(&d),
            [(ExcludedKind::Vendored, "vendor/dep/lib.rs")]
        );
        let d = run(dir.path(), &["build-out/x.rs"]);
        assert!(d.files.is_empty());
        assert_eq!(excluded(&d), [(ExcludedKind::Ignored, "build-out/x.rs")]);
        let includes = Includes {
            vendored: true,
            ..Includes::default()
        };
        let d = run_with(dir.path(), &["vendor/dep/lib.rs"], &[], &[], includes);
        assert_eq!(rels(&d), ["vendor/dep/lib.rs"]);
    }

    #[test]
    fn overlapping_paths_give_each_file_once() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let d = run(dir.path(), &["src", "src/lib.rs"]);
        assert_eq!(
            rels(&d),
            ["src/big.rs", "src/broken.rs", "src/lib.rs", "src/more.rs"]
        );
        assert_eq!(excluded(&d), [(ExcludedKind::Generated, "src/gen.rs")]);
        assert_eq!(d.unrecognized, 1);
        let d = run(dir.path(), &["src/lib.rs", "src/lib.rs"]);
        assert_eq!(rels(&d), ["src/lib.rs"]);
    }

    #[test]
    fn named_path_outside_the_root_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("conv");
        conv(&root);
        fs::create_dir(dir.path().join("outside")).unwrap();
        let err = scope_with(&root, &["../outside"], &[], &[], Includes::default())
            .err()
            .unwrap();
        assert!(matches!(err, AnalyzeError::OutsideRoot(_)));
        assert_eq!(
            err.to_string(),
            "path must be inside the workspace root: ../outside"
        );
    }

    #[test]
    fn missing_named_path_is_not_found_with_the_existing_text() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let err = scope_with(dir.path(), &["missing.rs"], &[], &[], Includes::default())
            .err()
            .unwrap();
        let expected =
            enrich_not_found_error("missing.rs", &dir.path().join("missing.rs"), dir.path());
        assert!(matches!(&err, AnalyzeError::NotFound(m) if *m == expected));
    }

    #[test]
    fn first_failing_named_path_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("conv");
        conv(&root);
        fs::create_dir(dir.path().join("x")).unwrap();
        let err = scope_with(&root, &["a.rs", "../x"], &[], &[], Includes::default())
            .err()
            .unwrap();
        assert!(
            matches!(&err, AnalyzeError::NotFound(m) if m.starts_with("The path 'a.rs' does not exist.")),
            "{err}"
        );
    }

    #[test]
    fn empty_paths_mean_the_root() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let scope = scope_with(dir.path(), &[], &[], &[], Includes::default()).unwrap();
        assert_eq!(scope.paths.len(), 1);
        let d = discover(dir.path(), &scope, &rust, &CancellationToken::new()).unwrap();
        assert_eq!(d.files.len(), 5);
    }

    #[test]
    fn marker_directories_and_conventional_file_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/ok.rs", b"fn ok() {}\n");
        write(
            root,
            "cache/CACHEDIR.TAG",
            b"Signature: 8a477f597d28d172789f06886806bc55\n",
        );
        write(root, "cache/x.rs", b"fn x() {}\n");
        write(root, "env/pyvenv.cfg", b"home = /usr\n");
        write(root, "env/lib.rs", b"fn y() {}\n");
        write(root, "Dockerfile", b"FROM scratch\n");
        let d = run(root, &[]);
        assert_eq!(
            excluded(&d),
            [
                (ExcludedKind::Generated, "cache/"),
                (ExcludedKind::Vendored, "env/"),
            ]
        );
        assert_eq!(rels(&d), ["Dockerfile", "src/ok.rs"]);
        assert_eq!(d.files[0].grammar, Grammar::Dockerfile);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        let outside = dir.path().join("outside");
        write(&outside, "t.rs", b"fn t() {}\n");
        write(&root, "src/ok.rs", b"fn ok() {}\n");
        symlink(outside.join("t.rs"), root.join("out.rs")).unwrap();
        symlink(outside.join("t.rs"), root.join("out.toml")).unwrap();
        symlink(root.join("src/ok.rs"), root.join("in.rs")).unwrap();
        symlink(root.join("src"), root.join("inner")).unwrap();
        symlink(&outside, root.join("src/shared")).unwrap();
        symlink(root.join("gone.rs"), root.join("b.rs")).unwrap();
        let d = run(&root, &[]);
        assert_eq!(rels(&d), ["src/ok.rs"]);
        assert!(d.excluded.is_empty());
        assert_eq!(d.unrecognized, 0);
        assert_eq!(
            skipped(&d),
            [
                ("b.rs", SkipKind::Unreadable, Some(Grammar::Rust)),
                (
                    "out.rs",
                    SkipKind::SymlinkOutsideWorkspace,
                    Some(Grammar::Rust)
                ),
                ("src/shared/", SkipKind::SymlinkOutsideWorkspace, None),
            ]
        );
        assert_no_absolute_path(&d, dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn named_symlink_to_a_file_outside_is_an_error() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        write(&dir.path().join("outside"), "t.rs", b"fn t() {}\n");
        fs::create_dir(&root).unwrap();
        symlink(dir.path().join("outside/t.rs"), root.join("out.rs")).unwrap();
        let err = scope_with(&root, &["out.rs"], &[], &[], Includes::default())
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            "path must be inside the workspace root: out.rs"
        );
    }

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unlistable_directory_is_a_walk_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/ok.rs", b"fn ok() {}\n");
        write(root, "locked/x.rs", b"fn x() {}\n");
        set_mode(&root.join("locked"), 0o000);
        if fs::read_dir(root.join("locked")).is_ok() {
            // The user can list any directory (root): nothing to test.
            set_mode(&root.join("locked"), 0o755);
            return;
        }
        let d = run(root, &[]);
        set_mode(&root.join("locked"), 0o755);
        assert_eq!(rels(&d), ["src/ok.rs"]);
        assert_eq!(skipped(&d), [("locked/", SkipKind::WalkError, None)]);
        assert_no_absolute_path(&d, root);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_header_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "x.rs", b"fn x() {}\n");
        set_mode(&root.join("x.rs"), 0o000);
        if fs::File::open(root.join("x.rs")).is_ok() {
            // The user can read any file (root): nothing to test.
            return;
        }
        let d = run(root, &[]);
        set_mode(&root.join("x.rs"), 0o644);
        assert!(d.files.is_empty());
        assert_eq!(
            skipped(&d),
            [("x.rs", SkipKind::Unreadable, Some(Grammar::Rust))]
        );
        assert_no_absolute_path(&d, root);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_file_name_is_skipped() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/ok.rs", b"fn ok() {}\n");
        let name = OsStr::from_bytes(b"bad\xff.rs");
        if fs::write(root.join(name), b"fn b() {}\n").is_err() {
            // The file system accepts only UTF-8 names: nothing to test.
            return;
        }
        let d = run(root, &[]);
        assert_eq!(rels(&d), ["src/ok.rs"]);
        assert_eq!(
            skipped(&d),
            [("bad\u{FFFD}.rs", SkipKind::NotUtf8, Some(Grammar::Rust))]
        );
    }

    #[cfg(unix)]
    #[test]
    fn fifo_is_a_candidate_and_is_not_opened() {
        let dir = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("mkfifo")
            .arg(dir.path().join("p.rs"))
            .status()
            .unwrap();
        assert!(status.success());
        let d = run(dir.path(), &[]);
        assert_eq!(rels(&d), ["p.rs"]);
    }

    #[test]
    fn no_file_count_cap() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10_001 {
            write(dir.path(), &format!("d{}/f{i}.rs", i % 100), b"fn f() {}\n");
        }
        let d = run(dir.path(), &[]);
        assert_eq!(d.files.len(), 10_001);
    }

    #[test]
    fn cancelled_token_stops_discovery() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        let scope = scope_with(dir.path(), &[], &[], &[], Includes::default()).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = discover(dir.path(), &scope, &rust, &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
    }

    #[test]
    fn removed_supported_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "Cargo.toml", b"[package]\n");
        assert!(!run(root, &[]).removed_supported);

        write(root, "src/ok.rs", b"fn ok() {}\n");
        let d = run_with(root, &[], &[], &["src/*.rs"], Includes::default());
        assert!(d.removed_supported);
        assert_eq!(d.filtered, 1);
        fs::remove_file(root.join("src/ok.rs")).unwrap();

        write(root, "src/gen.rs", b"// @generated\nfn g() {}\n");
        let d = run(root, &[]);
        assert!(d.removed_supported);
        assert_eq!(excluded(&d), [(ExcludedKind::Generated, "src/gen.rs")]);
        fs::remove_file(root.join("src/gen.rs")).unwrap();

        write(root, "tests/cli.rs", b"fn t() {}\n");
        assert!(run(root, &[]).removed_supported);
    }

    #[cfg(unix)]
    fn mkfifo(path: &Path) {
        let status = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[cfg(unix)]
    #[test]
    fn fifo_marker_is_not_opened() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(&root, "src/ok.rs", b"fn ok() {}\n");
        mkfifo(&root.join("src/CACHEDIR.TAG"));
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(run(&root, &[]).files.len());
        });
        let files = receive
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("discovery blocked on a FIFO marker");
        assert_eq!(files, 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_marker_and_ignore_files_are_not_followed() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        let outside = dir.path().join("outside");
        write(
            &outside,
            "CACHEDIR.TAG",
            b"Signature: 8a477f597d28d172789f06886806bc55\n",
        );
        write(&outside, "pyvenv.cfg", b"home = /usr\n");
        write(&outside, "ignore", b"src/\n");
        write(&root, "src/ok.rs", b"fn ok() {}\n");
        fs::create_dir(root.join(".git")).unwrap();
        symlink(outside.join("CACHEDIR.TAG"), root.join("src/CACHEDIR.TAG")).unwrap();
        symlink(outside.join("pyvenv.cfg"), root.join("src/pyvenv.cfg")).unwrap();
        symlink(outside.join("ignore"), root.join(".gitignore")).unwrap();
        let d = run(&root, &[]);
        assert_eq!(rels(&d), ["src/ok.rs"]);
        assert!(d.excluded.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn distinct_non_utf8_names_stay_distinct() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/ok.rs", b"fn ok() {}\n");
        let name = |bytes: &[u8]| root.join(OsStr::from_bytes(bytes));
        if fs::write(name(b"a\xff.rs"), b"fn a() {}\n").is_err() {
            // The file system accepts only UTF-8 names: nothing to test.
            return;
        }
        fs::write(name(b"a\xfe.rs"), b"fn a() {}\n").unwrap();
        for d in [&b"d\xff"[..], b"d\xfe"] {
            fs::create_dir(name(d)).unwrap();
            fs::write(name(d).join("x.rs"), b"fn x() {}\n").unwrap();
        }
        fs::write(name(b"t\xff.toml"), b"").unwrap();
        fs::write(name(b"t\xfe.toml"), b"").unwrap();
        let d = run(root, &[]);
        assert_eq!(
            skipped(&d),
            [
                ("a\u{FFFD}.rs", SkipKind::NotUtf8, Some(Grammar::Rust)),
                ("a\u{FFFD}.rs", SkipKind::NotUtf8, Some(Grammar::Rust)),
                ("d\u{FFFD}/x.rs", SkipKind::NotUtf8, Some(Grammar::Rust)),
                ("d\u{FFFD}/x.rs", SkipKind::NotUtf8, Some(Grammar::Rust)),
            ]
        );
        assert_eq!(rels(&d), ["src/ok.rs", "t\u{FFFD}.toml", "t\u{FFFD}.toml"]);
        let mut paths: Vec<_> = d.files.iter().map(|c| &c.path).collect();
        paths.dedup();
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn root_spelled_differently_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        write(&root, "src/ok.rs", b"fn ok() {}\n");
        let scope = scope_with(&root, &["src"], &[], &[], Includes::default()).unwrap();
        let other = root.join("..").join("ws");
        let err = discover(&other, &scope, &rust, &CancellationToken::new()).err();
        let Some(AnalyzeError::Failed { cause, .. }) = err else {
            panic!("expected a failure, got {err:?}");
        };
        assert!(cause.contains(&*root.to_string_lossy()), "{cause}");
        assert!(cause.contains(&*other.to_string_lossy()), "{cause}");
    }

    #[test]
    fn cancellation_is_checked_before_directory_markers() {
        let dir = tempfile::tempdir().unwrap();
        conv(dir.path());
        write(dir.path(), "a.toml", b"");
        for named in ["vendor", "vendor/dep/lib.rs"] {
            let scope = scope_with(
                dir.path(),
                &["a.toml", named],
                &[],
                &[],
                Includes::default(),
            )
            .unwrap();
            let cancel = CancellationToken::new();
            let cancel_on_first_file = |_: Grammar| {
                cancel.cancel();
                false
            };
            let result = discover(dir.path(), &scope, &cancel_on_first_file, &cancel);
            assert!(matches!(result, Err(AnalyzeError::Cancelled)), "{named}");
        }
    }
}
