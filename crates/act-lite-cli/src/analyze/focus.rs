//! Focus paths: the files whose connections or dependents an analysis
//! reports, analyzed or not.

use super::discovery::{Discovered, Scope};
use super::envelope::{AnalyzeError, ExcludedKind, rel_path};
use super::read::ReadOutcome;
use crate::mcp::path_error::enrich_not_found_error;
use act_lite_parser::Grammar;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

/// Checks each `focus` path in order: it exists, it is inside the workspace
/// root, and it is at or below a path of `scope`. Returns the paths resolved
/// inside the root, in `focus` order.
pub fn check_focus(
    root: &Path,
    scope: &Scope,
    focus: &[String],
) -> Result<Vec<PathBuf>, AnalyzeError> {
    let canonical_root = root.canonicalize().map_err(|err| AnalyzeError::Failed {
        analysis: "analyze",
        cause: format!("cannot resolve the workspace root: {err}"),
    })?;
    let mut paths = Vec::new();
    for input in focus {
        let joined = root.join(input);
        let Ok(target) = joined.canonicalize() else {
            return Err(AnalyzeError::NotFound(enrich_not_found_error(
                input, &joined, root,
            )));
        };
        let Ok(rel) = target.strip_prefix(&canonical_root) else {
            return Err(AnalyzeError::OutsideRoot(input.clone()));
        };
        let resolved = root.join(rel);
        if !scope.paths.iter().any(|p| resolved.starts_with(p)) {
            return Err(AnalyzeError::FocusOutsidePaths(input.clone()));
        }
        paths.push(resolved);
    }
    Ok(paths)
}

/// The focus paths and the files at or below them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSet {
    /// The focus paths, workspace-relative (a directory ends with `/`), in
    /// byte order.
    pub paths: Vec<String>,
    /// The focus files, in byte order.
    pub files: Vec<FocusFile>,
    /// The focus paths under which no file is analyzed, in byte order.
    pub paths_not_analyzed: Vec<String>,
}

/// One focus file, and why it is not analyzed when it is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusFile {
    pub rel: String,
    pub analyzed: bool,
    pub reason: Option<String>,
}

/// The files at or below `focus`: a named file as itself, a directory walked
/// with discovery's directory pruning below it (`.git` and the directories
/// discovery excluded). Each file's status is what discovery found;
/// [`FocusSet::settle`] adds what the read found.
pub fn focus_files(
    root: &Path,
    focus: &[PathBuf],
    discovered: &Discovered,
    supported: &dyn Fn(Grammar) -> bool,
    cancel: &CancellationToken,
) -> Result<FocusSet, AnalyzeError> {
    let mut paths = Vec::new();
    let mut rels = Vec::new();
    for path in focus {
        let is_dir = fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
        paths.push(rel_path(root, path, is_dir)?);
        if is_dir {
            walk(root, path, discovered, cancel, &mut rels)?;
        } else {
            rels.push(rel_path(root, path, false)?);
        }
    }
    sort_dedup(&mut paths);
    sort_dedup(&mut rels);
    let files = rels
        .into_iter()
        .map(|rel| {
            let reason = reason(&rel, discovered, supported);
            FocusFile {
                analyzed: reason.is_none(),
                rel,
                reason,
            }
        })
        .collect();
    let mut set = FocusSet {
        paths,
        files,
        paths_not_analyzed: Vec::new(),
    };
    set.count_not_analyzed();
    Ok(set)
}

impl FocusSet {
    /// Marks the focus files that the read skipped, or found to be test files
    /// as a whole, as not analyzed.
    pub fn settle(&mut self, read: &ReadOutcome) {
        for file in self.files.iter_mut().filter(|f| f.analyzed) {
            let reason = if let Some(skip) = read.skipped.iter().find(|s| s.file.file == file.rel) {
                Some(format!("skipped: {}", skip_kind(skip.file.kind)))
            } else if read
                .test_exclusions
                .iter()
                .any(|e| e.path == file.rel && e.range.is_none())
            {
                Some("excluded as tests".to_string())
            } else {
                None
            };
            if reason.is_some() {
                file.analyzed = false;
                file.reason = reason;
            }
        }
        self.count_not_analyzed();
    }

    fn count_not_analyzed(&mut self) {
        self.paths_not_analyzed = self
            .paths
            .iter()
            .filter(|path| {
                !self
                    .files
                    .iter()
                    .any(|file| file.analyzed && under(&file.rel, path))
            })
            .cloned()
            .collect();
    }
}

/// Adds every regular file below `dir`, without entering `.git` or a
/// directory that discovery excluded.
fn walk(
    root: &Path,
    dir: &Path,
    discovered: &Discovered,
    cancel: &CancellationToken,
    out: &mut Vec<String>,
) -> Result<(), AnalyzeError> {
    if cancel.is_cancelled() {
        return Err(AnalyzeError::Cancelled);
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let rel = rel_path(root, &path, true)?;
            let excluded = discovered.excluded.iter().any(|e| e.entry.path == rel);
            if path.file_name() != Some(OsStr::new(".git")) && !excluded {
                walk(root, &path, discovered, cancel, out)?;
            }
        } else if file_type.is_file() {
            out.push(rel_path(root, &path, false)?);
        }
    }
    Ok(())
}

/// Why discovery leaves `rel` unanalyzed, or `None` when it is analyzed.
fn reason(
    rel: &str,
    discovered: &Discovered,
    supported: &dyn Fn(Grammar) -> bool,
) -> Option<String> {
    if let Some(file) = discovered.files.iter().find(|f| f.rel == rel) {
        return (!supported(file.grammar))
            .then(|| format!("language not supported: {}", file.grammar.name()));
    }
    if let Some(e) = discovered
        .excluded
        .iter()
        .find(|e| e.entry.range.is_none() && under(rel, &e.entry.path))
    {
        return Some(format!("excluded as {}", excluded_kind(e.kind)));
    }
    if let Some(skip) = discovered.skipped.iter().find(|s| under(rel, &s.file.file)) {
        return Some(format!("skipped: {}", skip_kind(skip.file.kind)));
    }
    if Grammar::from_path(Path::new(rel)).is_none() {
        return Some("unrecognized".to_string());
    }
    Some("left out by include or exclude".to_string())
}

fn excluded_kind(kind: ExcludedKind) -> &'static str {
    match kind {
        ExcludedKind::Tests => "tests",
        ExcludedKind::Generated => "generated",
        ExcludedKind::Vendored => "vendored",
        ExcludedKind::Ignored => "ignored",
    }
}

fn skip_kind(kind: super::envelope::SkipKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Whether `file` is at or below the workspace-relative path `path`.
fn under(file: &str, path: &str) -> bool {
    path == "." || file == path || (path.ends_with('/') && file.starts_with(path))
}

fn sort_dedup(list: &mut Vec<String>) {
    list.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    list.dedup();
}
