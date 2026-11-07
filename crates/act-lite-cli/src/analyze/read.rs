//! Reading the files in scope: size bound, skipped files, parse failures and
//! the removal of embedded test code, one file at a time.

use super::classify::open_no_follow;
use super::discovery::{Candidate, Includes, Skip, left_out};
use super::embedded::{blank, test_regions};
use super::envelope::{AnalyzeError, ExcludedEntry, ParseFailure, SkipKind, SkippedFile, rel_path};
use act_lite_core::CompactRange;
use act_lite_parser::regions::{Region, RegionKind, RegionLanguage, RegionState, TIMEOUT_REASON};
use act_lite_parser::{Grammar, ParseOptions, ParserError, SourceFile};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tree_sitter::Node;

/// The largest file that is read, in bytes.
pub const MAX_FILE_BYTES: u64 = act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES;

/// A file read and parsed. With `include_tests` false the content has its
/// test regions blanked; every location is the file's own.
pub struct Parsed {
    pub rel: String,
    pub source: SourceFile,
    /// The original text of each test region that was blanked, in location
    /// order; empty when tests are included.
    pub removed: Vec<RemovedRegion>,
}

impl Parsed {
    /// The code an analyzer reads: for a component file, each script or
    /// frontmatter region tree in document order (never a style region or
    /// the outer tree); for any other file, its whole tree.
    pub fn code(&self) -> Vec<CodeUnit<'_>> {
        code_units(&self.source)
    }
}

/// One unit of code: a tree, its grammar and the text its positions refer
/// to. Every position is the file's own.
#[derive(Clone, Copy)]
pub struct CodeUnit<'a> {
    pub grammar: Grammar,
    pub tree: &'a tree_sitter::Tree,
    pub source: &'a str,
}

fn code_units(source: &SourceFile) -> Vec<CodeUnit<'_>> {
    let content = source.content();
    if !source.grammar().is_component() {
        return vec![CodeUnit {
            grammar: source.grammar(),
            tree: source.tree().inner(),
            source: content,
        }];
    }
    source
        .region_trees()
        .iter()
        .filter(|tree| is_script(&source.regions()[tree.region]))
        .map(|tree| CodeUnit {
            grammar: tree.grammar,
            tree: &tree.tree,
            source: content,
        })
        .collect()
}

/// Whether a region holds script code (a script or the frontmatter).
fn is_script(region: &Region) -> bool {
    region.kind != RegionKind::Style
}

/// A script or frontmatter region of a component file that is not
/// analyzed: its start tag and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionNotAnalyzed {
    pub file: String,
    pub range: CompactRange,
    pub reason: String,
}

/// A blanked test region and the text it held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedRegion {
    pub range: CompactRange,
    pub text: String,
}

/// The result of reading one file.
pub enum ReadResult {
    Parsed {
        parsed: Parsed,
        parse_failures: Vec<ParseFailure>,
        test_exclusions: Vec<ExcludedEntry>,
        /// The script regions of a component file that are not analyzed,
        /// in document order.
        regions_not_analyzed: Vec<RegionNotAnalyzed>,
        /// A component file with script regions, all of a language that is
        /// not parsed: it holds no code an analyzer reads.
        without_analyzed_code: bool,
    },
    Skipped(Skip),
    /// A file whose own attribute `#![cfg(test)]` makes it test code.
    TestFile(ExcludedEntry),
}

/// Reads and parses one file.
pub fn read_file(file: &Candidate, include_tests: bool) -> Result<ReadResult, AnalyzeError> {
    read_file_with(file, include_tests, &CancellationToken::new(), None)
}

/// Reads and parses one file as [`read_file`] does, and checks `cancel`
/// before each region parse of a component file. `region_timeout`, when
/// set, is the deadline of each parse of the file (the outer parse and
/// each region's) in place of the parser's own.
pub fn read_file_with(
    file: &Candidate,
    include_tests: bool,
    cancel: &CancellationToken,
    region_timeout: Option<Duration>,
) -> Result<ReadResult, AnalyzeError> {
    read_stopped(file, include_tests, region_timeout, token_stop(cancel))
}

/// A check that stops a load: `true` once the request is cancelled.
type Stop = Arc<dyn Fn() -> bool + Send + Sync>;

fn token_stop(cancel: &CancellationToken) -> Stop {
    let cancel = cancel.clone();
    Arc::new(move || cancel.is_cancelled())
}

fn read_stopped(
    file: &Candidate,
    include_tests: bool,
    timeout: Option<Duration>,
    stop: Stop,
) -> Result<ReadResult, AnalyzeError> {
    let content = match read_content(file) {
        Ok(content) => content,
        Err(skip) => return Ok(ReadResult::Skipped(skip)),
    };
    let parse = |content: String| parse(file, content, timeout, &stop);
    let mut source = parse(content)?;
    let mut test_exclusions = Vec::new();
    let mut removed = Vec::new();
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
            removed = found
                .regions
                .iter()
                .map(|region| RemovedRegion {
                    range: region.range.clone(),
                    text: source.content()[region.bytes.clone()].to_string(),
                })
                .collect();
            source = parse(blank(source.content(), &found.regions))?;
        }
    }
    let (regions_not_analyzed, without_analyzed_code) = region_evidence(&file.rel, &source);
    // A file with no analyzed code is read as a file of a language that
    // is not supported: it adds no parse failure.
    let parse_failures = if without_analyzed_code {
        Vec::new()
    } else {
        first_failure(&source, &regions_not_analyzed)
            .map(|range| ParseFailure {
                file: file.rel.clone(),
                range,
            })
            .into_iter()
            .collect()
    };
    Ok(ReadResult::Parsed {
        parsed: Parsed {
            rel: file.rel.clone(),
            source,
            removed,
        },
        parse_failures,
        test_exclusions,
        regions_not_analyzed,
        without_analyzed_code,
    })
}

/// The script regions of a component file that are not analyzed, in
/// document order, and whether the file has script regions and all of them
/// are of a language that is not parsed. Style regions and empty regions
/// are never listed.
fn region_evidence(rel: &str, source: &SourceFile) -> (Vec<RegionNotAnalyzed>, bool) {
    let mut has_tree = vec![false; source.regions().len()];
    for tree in source.region_trees() {
        has_tree[tree.region] = true;
    }
    let mut listed = Vec::new();
    let (mut scripts, mut unsupported) = (0usize, 0usize);
    for (region, parsed) in source.regions().iter().zip(has_tree) {
        if !is_script(region) || region.state == RegionState::Empty {
            continue;
        }
        scripts += 1;
        let reason = match (&region.state, &region.language) {
            (RegionState::Malformed, _) => "malformed region".to_string(),
            (_, RegionLanguage::Unsupported(language)) => {
                unsupported += 1;
                format!("unsupported script language: {language}")
            }
            _ if !parsed => TIMEOUT_REASON.to_string(),
            _ => continue,
        };
        listed.push(RegionNotAnalyzed {
            file: rel.to_string(),
            range: region.start_tag.clone(),
            reason,
        });
    }
    (listed, scripts > 0 && unsupported == scripts)
}

/// The file's parse failure: the first ERROR or MISSING node over its code
/// units and, for a component file, its outer tree, or the start tag of a
/// script region whose parse timed out, whichever comes first.
fn first_failure(source: &SourceFile, not_analyzed: &[RegionNotAnalyzed]) -> Option<CompactRange> {
    let mut found: Vec<CompactRange> = code_units(source)
        .iter()
        .filter_map(|unit| first_error(unit.tree.root_node()))
        .collect();
    if source.grammar().is_component() {
        found.extend(first_error(source.tree().root_node()));
        found.extend(
            not_analyzed
                .iter()
                .filter(|region| region.reason == TIMEOUT_REASON)
                .map(|region| region.range.clone()),
        );
    }
    found
        .into_iter()
        .min_by_key(|range| (range.start_line(), range.start_col()))
}

/// What reading found besides the files themselves.
#[derive(Default)]
pub struct ReadOutcome {
    /// Sorted by `rel`; empty from [`Reader::finish`].
    pub parsed: Vec<Parsed>,
    pub skipped: Vec<Skip>,
    pub parse_failures: Vec<ParseFailure>,
    pub test_exclusions: Vec<ExcludedEntry>,
    /// The script regions of component files that are not analyzed, sorted
    /// by file, then location.
    pub regions_not_analyzed: Vec<RegionNotAnalyzed>,
    /// The component files that hold no analyzed code.
    pub files_without_analyzed_code: u64,
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
    /// The keys of `outcome.regions_not_analyzed`, so that a reread adds an
    /// entry in constant time.
    regions_seen: HashSet<RegionKey>,
}

/// File, range (start line and column, end line and column) and reason.
type RegionKey = (String, (u32, u32, u32, u32), String);

fn region_key(region: &RegionNotAnalyzed) -> RegionKey {
    let r = &region.range;
    (
        region.file.clone(),
        (r.start_line(), r.start_col(), r.end_line(), r.end_col()),
        region.reason.clone(),
    )
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
            regions_seen: HashSet::new(),
        }
    }

    /// The component files pulled so far that hold no analyzed code.
    pub fn files_without_analyzed_code(&self) -> u64 {
        self.outcome.files_without_analyzed_code
    }

    /// The skips, parse failures, test exclusions and regions not analyzed
    /// of every file pulled so far, each list sorted.
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
        outcome.regions_not_analyzed.sort_by(|a, b| {
            let key = |r: &RegionNotAnalyzed| {
                let range = &r.range;
                (
                    range.start_line(),
                    range.start_col(),
                    range.end_line(),
                    range.end_col(),
                )
            };
            a.file
                .as_bytes()
                .cmp(b.file.as_bytes())
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
            match read_file_with(file, self.include_tests, &self.cancel, None)? {
                ReadResult::Parsed {
                    parsed,
                    parse_failures,
                    test_exclusions,
                    regions_not_analyzed,
                    without_analyzed_code,
                } => {
                    self.outcome.parse_failures.extend(parse_failures);
                    self.outcome.test_exclusions.extend(test_exclusions);
                    self.add_regions(regions_not_analyzed);
                    self.outcome.files_without_analyzed_code += u64::from(without_analyzed_code);
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
        match read_file_with(file, self.include_tests, &self.cancel, None)? {
            ReadResult::Parsed {
                parsed,
                parse_failures,
                test_exclusions,
                regions_not_analyzed,
                ..
            } => {
                add_new(&mut self.outcome.parse_failures, parse_failures);
                add_new(&mut self.outcome.test_exclusions, test_exclusions);
                self.add_regions(regions_not_analyzed);
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

impl Reader {
    /// Adds each region entry that is not listed yet.
    fn add_regions(&mut self, found: Vec<RegionNotAnalyzed>) {
        for region in found {
            if self.regions_seen.insert(region_key(&region)) {
                self.outcome.regions_not_analyzed.push(region);
            }
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

/// The range of the first ERROR or MISSING node of a tree in document order.
fn first_error(root: Node) -> Option<CompactRange> {
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
    let node = find(root)?;
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
    read_text(&file.path).map_err(|(kind, reason)| skip(file, kind, reason))
}

/// The text of the regular file at `path`, bounded as [`read_content`] says,
/// or the kind and reason of the failure.
fn read_text(path: &Path) -> Result<String, (SkipKind, String)> {
    let unreadable = |reason: String| (SkipKind::Unreadable, reason);
    let linked =
        fs::symlink_metadata(path).map_err(|err| unreadable(format!("cannot read file: {err}")))?;
    if !linked.is_file() {
        return Err(unreadable("not a regular file".to_string()));
    }
    let handle =
        open_no_follow(path).map_err(|err| unreadable(format!("cannot read file: {err}")))?;
    let meta = handle
        .metadata()
        .map_err(|err| unreadable(format!("cannot stat file: {err}")))?;
    if !meta.is_file() {
        return Err(unreadable("not a regular file".to_string()));
    }
    let too_big = |len: u64| {
        (
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
    String::from_utf8(bytes)
        .map_err(|_| (SkipKind::NotUtf8, "content is not valid UTF-8".to_string()))
}

/// Why a support read or listing failed. Each variant but `Cancelled` holds
/// the workspace-relative path as asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportError {
    NotFound(String),
    /// The path and the cause.
    Unreadable(String, String),
    TooLarge(String),
    NotUtf8(String),
    /// Outside the workspace, or left out by discovery's directory and
    /// ignore steps under the request's includes.
    Refused(String),
    /// A whole-file test (`#![cfg(test)]`) read while tests are excluded.
    TestFile(String),
    Cancelled,
}

impl fmt::Display for SupportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupportError::NotFound(path) => write!(f, "{path}: not found"),
            SupportError::Unreadable(path, cause) => write!(f, "{path}: {cause}"),
            SupportError::TooLarge(path) => {
                write!(f, "{path}: larger than {MAX_FILE_BYTES} bytes")
            }
            SupportError::NotUtf8(path) => write!(f, "{path}: content is not valid UTF-8"),
            SupportError::Refused(path) => {
                write!(f, "{path}: outside the workspace or left out by discovery")
            }
            SupportError::TestFile(path) => write!(f, "{path}: excluded as tests"),
            SupportError::Cancelled => f.write_str("analysis cancelled"),
        }
    }
}

/// What a directory entry is; a symlink is not followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

/// Where a symlink points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymlinkTarget {
    /// An existing entry inside the workspace, workspace-relative.
    Inside(String),
    /// An existing entry outside the workspace.
    Outside,
    /// Nothing: the target does not exist or cannot be resolved.
    Broken,
}

/// One entry of a listed directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// The name as it is on disk, compared byte for byte.
    pub name: String,
    pub kind: EntryKind,
    /// For a symlink, where it points; `None` for any other entry.
    pub target: Option<SymlinkTarget>,
}

/// Reads files that an analysis needs but does not analyze: source files for
/// their declarations, and configuration files. Every read stays inside the
/// workspace, has the bounds of an analyzed read, and adds nothing to the
/// result's evidence.
pub struct SupportReader {
    root: PathBuf,
    canonical_root: Option<PathBuf>,
    includes: Includes,
    cancel: CancellationToken,
}

impl SupportReader {
    /// A reader for the workspace `root` under the request's `includes`.
    pub fn new(root: &Path, includes: &Includes, cancel: &CancellationToken) -> SupportReader {
        SupportReader {
            root: root.to_path_buf(),
            canonical_root: root.canonicalize().ok(),
            includes: *includes,
            cancel: cancel.clone(),
        }
    }

    /// The request's includes, which every read and listing applies.
    pub fn includes(&self) -> &Includes {
        &self.includes
    }

    /// Reads a source file and parses it with `grammar`, whatever its
    /// extension, blanking its test regions as an analyzed read does. A file
    /// below a pruned directory is refused.
    pub fn source(&self, rel: &str, grammar: Grammar) -> Result<Parsed, SupportError> {
        let (path, written) = self.resolve(rel, true)?;
        let file = Candidate {
            path,
            rel: written,
            grammar,
        };
        match read_file(&file, self.includes.tests) {
            Ok(ReadResult::Parsed { parsed, .. }) => Ok(parsed),
            Ok(ReadResult::Skipped(skip)) => Err(skip_error(rel, skip.file.kind, skip.file.reason)),
            Ok(ReadResult::TestFile(_)) => Err(SupportError::TestFile(rel.to_string())),
            Err(err) => Err(SupportError::Unreadable(rel.to_string(), err.to_string())),
        }
    }

    /// Reads a configuration file as text. A file below a pruned directory is
    /// refused.
    pub fn text(&self, rel: &str) -> Result<String, SupportError> {
        let (path, _) = self.resolve(rel, true)?;
        read_text(&path).map_err(|(kind, reason)| skip_error(rel, kind, reason))
    }

    /// Lists a directory, also below a pruned directory (names and kinds only;
    /// no file is read), in byte order of name.
    pub fn list_dir(&self, rel: &str) -> Result<Vec<DirEntry>, SupportError> {
        let (path, _) = self.resolve(rel, false)?;
        let entries = fs::read_dir(&path)
            .map_err(|err| SupportError::Unreadable(rel.to_string(), err.to_string()))?;
        let mut out = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|err| SupportError::Unreadable(rel.to_string(), err.to_string()))?;
            // A name that is not UTF-8 cannot be matched byte for byte by a
            // probe written in UTF-8; listing it lossily would let `\u{FFFD}`
            // match it. Discovery skips such names too.
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let file_type = entry
                .file_type()
                .map_err(|err| SupportError::Unreadable(rel.to_string(), err.to_string()))?;
            let (kind, target) = if file_type.is_symlink() {
                (EntryKind::Symlink, Some(self.target(&entry.path())))
            } else if file_type.is_dir() {
                (EntryKind::Dir, None)
            } else if file_type.is_file() {
                (EntryKind::File, None)
            } else {
                // A FIFO, socket or device is never a source file or input.
                continue;
            };
            out.push(DirEntry { name, kind, target });
        }
        out.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
        Ok(out)
    }

    /// The canonical path of `rel` and its written form, after the cancel
    /// check; refused when it leaves the workspace, or, with `unpruned`, when
    /// it lies below a pruned directory.
    fn resolve(&self, rel: &str, unpruned: bool) -> Result<(PathBuf, String), SupportError> {
        if self.cancel.is_cancelled() {
            return Err(SupportError::Cancelled);
        }
        let refused = || SupportError::Refused(rel.to_string());
        let mut depth = 0usize;
        for part in Path::new(rel).components() {
            match part {
                Component::Normal(_) => depth += 1,
                Component::CurDir => {}
                Component::ParentDir if depth > 0 => depth -= 1,
                _ => return Err(refused()),
            }
        }
        let joined = self.root.join(rel);
        let Ok(path) = joined.canonicalize() else {
            return Err(SupportError::NotFound(rel.to_string()));
        };
        let Some(inside) = self
            .canonical_root
            .as_ref()
            .and_then(|root| path.strip_prefix(root).ok())
        else {
            return Err(refused());
        };
        if unpruned {
            let out = left_out(
                &self.root,
                inside,
                path.is_dir(),
                &self.includes,
                &self.cancel,
            )
            .map_err(|_| SupportError::Cancelled)?;
            if out {
                return Err(refused());
            }
        }
        let written = rel_path(&self.root, &self.root.join(inside), path.is_dir())
            .map_err(|err| SupportError::Unreadable(rel.to_string(), err.to_string()))?;
        Ok((path, written))
    }

    /// Where the symlink at `path` points.
    fn target(&self, path: &Path) -> SymlinkTarget {
        let Ok(target) = path.canonicalize() else {
            return SymlinkTarget::Broken;
        };
        let inside = self
            .canonical_root
            .as_ref()
            .and_then(|root| target.strip_prefix(root).ok());
        match inside
            .and_then(|rel| rel_path(&self.root, &self.root.join(rel), target.is_dir()).ok())
        {
            Some(rel) => SymlinkTarget::Inside(rel),
            None => SymlinkTarget::Outside,
        }
    }
}

/// The support error for a read that failed with a skip of `kind`.
fn skip_error(rel: &str, kind: SkipKind, reason: String) -> SupportError {
    match kind {
        SkipKind::SizeLimit => SupportError::TooLarge(rel.to_string()),
        SkipKind::NotUtf8 => SupportError::NotUtf8(rel.to_string()),
        _ => SupportError::Unreadable(rel.to_string(), reason),
    }
}

fn parse(
    file: &Candidate,
    content: String,
    timeout: Option<Duration>,
    stop: &Stop,
) -> Result<SourceFile, AnalyzeError> {
    let stop = Arc::clone(stop);
    let options = ParseOptions {
        timeout,
        stop: Some(Box::new(move || stop())),
        ..ParseOptions::default()
    };
    SourceFile::from_content_with(&file.path, content, file.grammar, options).map_err(|err| {
        match err {
            ParserError::Cancelled => AnalyzeError::Cancelled,
            err => AnalyzeError::Failed {
                analysis: "analyze",
                cause: format!("cannot parse {}: {err}", file.rel),
            },
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
                ..
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

    #[test]
    fn a_token_cancelled_after_the_first_region_parse_stops_before_the_second() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Two.vue",
            b"<script setup lang=\"ts\">\nconst a = 1\n</script>\n<script setup lang=\"ts\">\nconst b = 2\n</script>\n",
        );
        let file = Candidate {
            path: dir.path().join("Two.vue"),
            rel: "Two.vue".to_string(),
            grammar: Grammar::Vue,
        };
        let token = CancellationToken::new();
        let checks = Arc::new(AtomicUsize::new(0));
        let stop: Stop = {
            let (check, token, checks) = (token_stop(&token), token.clone(), checks.clone());
            Arc::new(move || {
                let stopped = check();
                // The token is cancelled while the first region is parsed.
                if checks.fetch_add(1, Ordering::SeqCst) == 0 {
                    token.cancel();
                }
                stopped
            })
        };
        assert!(matches!(
            read_stopped(&file, false, None, stop),
            Err(AnalyzeError::Cancelled)
        ));
        assert_eq!(
            checks.load(Ordering::SeqCst),
            2,
            "one check per region parse"
        );
        assert!(matches!(
            read_file_with(&file, false, &CancellationToken::new(), None),
            Ok(ReadResult::Parsed { .. })
        ));
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
