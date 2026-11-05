//! The result envelope every analyzer returns: status, findings cut to a
//! limit, coverage, exclusions, incomplete evidence and the evidence sources,
//! each list in one total order.

use act_lite_core::CompactRange;
use serde::Serialize;
use std::cmp::Ordering;
use std::path::{Component, Path};

/// The maximum number of entries in each list of `excluded` and `incomplete`.
pub const MAX_EVIDENCE_ENTRIES: usize = 100;

/// A source location: a workspace-relative file and, unless the item
/// concerns the whole file, a range in it.
///
/// Ordered by file (byte order), then by start line, start column, end line
/// and end column; a missing range sorts first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<CompactRange>,
}

impl Ord for Location {
    fn cmp(&self, other: &Self) -> Ordering {
        self.file
            .as_bytes()
            .cmp(other.file.as_bytes())
            .then_with(|| cmp_opt_range(self.range.as_ref(), other.range.as_ref()))
    }
}

impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn range_key(r: &CompactRange) -> (u32, u32, u32, u32) {
    (r.start_line(), r.start_col(), r.end_line(), r.end_col())
}

fn cmp_range(a: &CompactRange, b: &CompactRange) -> Ordering {
    range_key(a).cmp(&range_key(b))
}

fn cmp_opt_range(a: Option<&CompactRange>, b: Option<&CompactRange>) -> Ordering {
    a.map(range_key).cmp(&b.map(range_key))
}

/// Writes `path` relative to `root`: `/` separators, no leading `./`, each
/// invalid UTF-8 sequence replaced by U+FFFD, `"."` for the root itself and a
/// trailing `/` for a directory.
pub fn rel_path(root: &Path, path: &Path, is_dir: bool) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        return ".".to_string();
    }
    let mut out = parts.join("/");
    if is_dir {
        out.push('/');
    }
    out
}

/// One result item of an analyzer.
pub trait Finding: Serialize {
    /// The subject location; findings are ordered and cut by it.
    fn location(&self) -> &Location;

    /// The total order of findings: the analyzer's rank if it defines one,
    /// then the location, then the analyzer's last key.
    fn order(&self, other: &Self) -> Ordering;

    /// The evidence source that produced this finding, required when the
    /// result has more than one source.
    fn source(&self) -> Option<&str> {
        None
    }
}

/// The outcome of an analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Findings,
    NoFindings,
    Unsupported,
    Incomplete,
}

impl Status {
    /// Derives the status from the result, checking the rules in order:
    /// unsupported, findings, incomplete, no findings.
    pub fn derive(total: u64, complete: bool, analyzed_files: u64) -> Status {
        if total == 0 && complete && analyzed_files == 0 {
            Status::Unsupported
        } else if total >= 1 {
            Status::Findings
        } else if !complete {
            Status::Incomplete
        } else {
            Status::NoFindings
        }
    }
}

/// The maximum number of findings returned, 0 to 1000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Limit(u32);

impl Limit {
    pub const DEFAULT: Limit = Limit(100);
    const MAX: u32 = 1000;

    pub fn new(n: u32) -> Result<Limit, AnalyzeError> {
        if n > Self::MAX {
            return Err(AnalyzeError::LimitOutOfRange(n));
        }
        Ok(Limit(n))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// The files of one recognized language in scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageCoverage {
    pub language: &'static str,
    pub files: u64,
    pub supported: bool,
}

/// What the analysis covered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coverage {
    pub analyzed_files: u64,
    pub languages: Vec<LanguageCoverage>,
    pub unrecognized_files: u64,
}

impl Coverage {
    /// Sorts `languages` by language name (byte order).
    pub fn new(
        mut languages: Vec<LanguageCoverage>,
        analyzed_files: u64,
        unrecognized_files: u64,
    ) -> Coverage {
        languages.sort_by(|a, b| a.language.as_bytes().cmp(b.language.as_bytes()));
        Coverage {
            analyzed_files,
            languages,
            unrecognized_files,
        }
    }
}

/// Why discovery left an entry out of the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcludedKind {
    Tests,
    Generated,
    Vendored,
    Ignored,
}

/// An excluded file, directory (`"<dir>/"`) or test region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExcludedEntry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<CompactRange>,
}

/// An excluded entry and its category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exclusion {
    pub kind: ExcludedKind,
    pub entry: ExcludedEntry,
}

/// The number of entries per exclusion category, before the cut.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExcludedCounts {
    pub tests: u64,
    pub generated: u64,
    pub vendored: u64,
    pub ignored: u64,
}

/// The entries that discovery left out, per category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Excluded {
    pub tests: Vec<ExcludedEntry>,
    pub generated: Vec<ExcludedEntry>,
    pub vendored: Vec<ExcludedEntry>,
    pub ignored: Vec<ExcludedEntry>,
    pub counts: ExcludedCounts,
    pub truncated: bool,
}

impl Excluded {
    /// Sorts each category by path, then range, and cuts it to
    /// [`MAX_EVIDENCE_ENTRIES`].
    pub fn new(entries: Vec<Exclusion>) -> Excluded {
        let (mut tests, mut generated, mut vendored, mut ignored) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for Exclusion { kind, entry } in entries {
            match kind {
                ExcludedKind::Tests => tests.push(entry),
                ExcludedKind::Generated => generated.push(entry),
                ExcludedKind::Vendored => vendored.push(entry),
                ExcludedKind::Ignored => ignored.push(entry),
            }
        }
        let order = |a: &ExcludedEntry, b: &ExcludedEntry| {
            a.path
                .as_bytes()
                .cmp(b.path.as_bytes())
                .then_with(|| cmp_opt_range(a.range.as_ref(), b.range.as_ref()))
        };
        let mut truncated = false;
        let counts = ExcludedCounts {
            tests: bound(&mut tests, order, &mut truncated),
            generated: bound(&mut generated, order, &mut truncated),
            vendored: bound(&mut vendored, order, &mut truncated),
            ignored: bound(&mut ignored, order, &mut truncated),
        };
        Excluded {
            tests,
            generated,
            vendored,
            ignored,
            counts,
            truncated,
        }
    }
}

/// Sorts `list`, cuts it to [`MAX_EVIDENCE_ENTRIES`] and returns its length
/// before the cut; sets `truncated` when it cut.
fn bound<T>(list: &mut Vec<T>, order: impl Fn(&T, &T) -> Ordering, truncated: &mut bool) -> u64 {
    list.sort_by(order);
    let total = list.len();
    if total > MAX_EVIDENCE_ENTRIES {
        list.truncate(MAX_EVIDENCE_ENTRIES);
        *truncated = true;
    }
    total as u64
}

/// Why a file in scope was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipKind {
    SizeLimit,
    NotUtf8,
    Unreadable,
    SymlinkOutsideWorkspace,
    WalkError,
}

impl SkipKind {
    fn as_str(self) -> &'static str {
        match self {
            SkipKind::SizeLimit => "size_limit",
            SkipKind::NotUtf8 => "not_utf8",
            SkipKind::Unreadable => "unreadable",
            SkipKind::SymlinkOutsideWorkspace => "symlink_outside_workspace",
            SkipKind::WalkError => "walk_error",
        }
    }
}

/// A file in scope that was not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedFile {
    pub file: String,
    pub kind: SkipKind,
    pub reason: String,
}

/// An analyzed file whose parse tree has an error: the first ERROR or
/// MISSING node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseFailure {
    pub file: String,
    pub range: CompactRange,
}

/// A relationship the analysis needed and could not resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unresolved {
    pub file: String,
    pub range: CompactRange,
    pub kind: String,
    pub target: String,
    pub reason: String,
}

/// An analysis bound that stopped the analysis before it was complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LimitHit {
    pub limit: String,
    pub value: u64,
    pub detail: String,
}

/// The number of entries per incomplete-evidence list, before the cut.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IncompleteCounts {
    pub skipped_files: u64,
    pub parse_failures: u64,
    pub unresolved: u64,
    pub limits_hit: u64,
}

/// The evidence the analysis is missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Incomplete {
    pub skipped_files: Vec<SkippedFile>,
    pub parse_failures: Vec<ParseFailure>,
    pub unresolved: Vec<Unresolved>,
    pub limits_hit: Vec<LimitHit>,
    pub counts: IncompleteCounts,
    pub truncated: bool,
}

impl Incomplete {
    /// Sorts each list and cuts it to [`MAX_EVIDENCE_ENTRIES`]. Skipped files
    /// sort by file, then kind; parse failures by file; unresolved entries by
    /// file, range, kind, then target; limit hits by limit, then value. The
    /// remaining fields break any tie, so the order is total.
    pub fn new(
        mut skipped_files: Vec<SkippedFile>,
        mut parse_failures: Vec<ParseFailure>,
        mut unresolved: Vec<Unresolved>,
        mut limits_hit: Vec<LimitHit>,
    ) -> Incomplete {
        let mut truncated = false;
        let counts = IncompleteCounts {
            skipped_files: bound(
                &mut skipped_files,
                |a, b| {
                    a.file
                        .as_bytes()
                        .cmp(b.file.as_bytes())
                        .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
                        .then_with(|| a.reason.cmp(&b.reason))
                },
                &mut truncated,
            ),
            parse_failures: bound(
                &mut parse_failures,
                |a, b| {
                    a.file
                        .as_bytes()
                        .cmp(b.file.as_bytes())
                        .then_with(|| cmp_range(&a.range, &b.range))
                },
                &mut truncated,
            ),
            unresolved: bound(
                &mut unresolved,
                |a, b| {
                    a.file
                        .as_bytes()
                        .cmp(b.file.as_bytes())
                        .then_with(|| cmp_range(&a.range, &b.range))
                        .then_with(|| a.kind.cmp(&b.kind))
                        .then_with(|| a.target.cmp(&b.target))
                        .then_with(|| a.reason.cmp(&b.reason))
                },
                &mut truncated,
            ),
            limits_hit: bound(
                &mut limits_hit,
                |a, b| {
                    a.limit
                        .cmp(&b.limit)
                        .then_with(|| a.value.cmp(&b.value))
                        .then_with(|| a.detail.cmp(&b.detail))
                },
                &mut truncated,
            ),
        };
        Incomplete {
            skipped_files,
            parse_failures,
            unresolved,
            limits_hit,
            counts,
            truncated,
        }
    }

    /// True when no evidence is missing: every count is 0.
    pub fn is_complete(&self) -> bool {
        self.counts == IncompleteCounts::default()
    }
}

/// What happened to the language server for one language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Used,
    Disabled,
    NotInstalled,
    Failed,
}

/// The language server of one language and its state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageServerUse {
    pub language: &'static str,
    pub server: &'static str,
    pub state: ServerState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The instruments that produced the result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub sources: Vec<String>,
    pub language_servers: Vec<LanguageServerUse>,
}

impl Evidence {
    /// Sorts and deduplicates `sources`; sorts `language_servers` by language.
    pub fn new(mut sources: Vec<String>, mut language_servers: Vec<LanguageServerUse>) -> Evidence {
        sources.sort();
        sources.dedup();
        language_servers.sort_by(|a, b| a.language.as_bytes().cmp(b.language.as_bytes()));
        Evidence {
            sources,
            language_servers,
        }
    }
}

/// The result of one analyzer.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope<F: Finding> {
    pub analysis: &'static str,
    pub status: Status,
    pub complete: bool,
    pub findings: Vec<F>,
    pub total: u64,
    pub limit: Limit,
    pub truncated: bool,
    pub coverage: Coverage,
    pub excluded: Excluded,
    pub incomplete: Incomplete,
    pub evidence: Evidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Map<String, serde_json::Value>>,
}

impl<F: Finding> Envelope<F> {
    /// Sorts the findings by [`Finding::order`], cuts them to `limit`, and
    /// derives `total`, `truncated`, `complete` and `status`.
    ///
    /// Fails when `evidence` has more than one source and a finding has no
    /// source, or when an array in `details` has no sibling integer
    /// `<name>_total`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        analysis: &'static str,
        mut findings: Vec<F>,
        limit: Limit,
        coverage: Coverage,
        excluded: Excluded,
        incomplete: Incomplete,
        evidence: Evidence,
        details: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<Envelope<F>, AnalyzeError> {
        let failed = |cause: String| AnalyzeError::Failed { analysis, cause };
        if evidence.sources.len() > 1
            && let Some(f) = findings.iter().find(|f| f.source().is_none())
        {
            return Err(failed(format!(
                "the finding in {} has no source, and the result has {} sources",
                f.location().file,
                evidence.sources.len()
            )));
        }
        if let Some(details) = &details {
            check_totals(details).map_err(failed)?;
        }
        findings.sort_by(|a, b| a.order(b));
        let total = findings.len() as u64;
        findings.truncate(limit.get() as usize);
        let complete = incomplete.is_complete();
        Ok(Envelope {
            analysis,
            status: Status::derive(total, complete, coverage.analyzed_files),
            complete,
            truncated: total > findings.len() as u64,
            findings,
            total,
            limit,
            coverage,
            excluded,
            incomplete,
            evidence,
            details,
        })
    }
}

/// Checks that every array in `map`, and in the objects it holds, has a
/// sibling integer `<name>_total`.
fn check_totals(map: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    for (name, value) in map {
        match value {
            serde_json::Value::Array(items) => {
                let total = format!("{name}_total");
                if !map.get(&total).is_some_and(|t| t.is_u64()) {
                    return Err(format!(
                        "details array `{name}` has no integer `{total}` beside it"
                    ));
                }
                for item in items {
                    if let serde_json::Value::Object(inner) = item {
                        check_totals(inner)?;
                    }
                }
            }
            serde_json::Value::Object(inner) => check_totals(inner)?,
            _ => {}
        }
    }
    Ok(())
}

/// Rounds `x` half to even to 3 decimal places. An integral result is an
/// integer; otherwise the number has no trailing zeros.
///
/// # Panics
///
/// Panics when `x` is not finite.
pub fn decimal(x: f64) -> serde_json::Number {
    assert!(x.is_finite(), "decimal requires a finite value, got {x}");
    let text = format!("{x:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text.parse::<i64>() {
        Ok(n) => n.into(),
        Err(_) => serde_json::Number::from_f64(text.parse().expect("formatted float"))
            .expect("finite value"),
    }
}

/// The entries that left a scope empty, for the empty-scope error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyScope {
    pub tests: u64,
    pub generated: u64,
    pub vendored: u64,
    pub ignored: u64,
    pub filtered: u64,
    pub unrecognized: u64,
}

/// A request error of an analyzer. `Display` is the message both transports
/// show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzeError {
    /// A named path does not exist; holds the not-found message.
    NotFound(String),
    /// A named path is outside the workspace root; holds the path.
    OutsideRoot(String),
    LimitOutOfRange(u32),
    EmptyScope(EmptyScope),
    Failed {
        analysis: &'static str,
        cause: String,
    },
    Cancelled,
}

impl AnalyzeError {
    /// True for an error in the request; false for a failure or a
    /// cancellation.
    pub fn is_invalid_params(&self) -> bool {
        !matches!(self, AnalyzeError::Failed { .. } | AnalyzeError::Cancelled)
    }
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyzeError::NotFound(message) => f.write_str(message),
            AnalyzeError::OutsideRoot(path) => {
                write!(f, "path must be inside the workspace root: {path}")
            }
            AnalyzeError::LimitOutOfRange(n) => {
                write!(f, "limit must be between 0 and {}, got {n}", Limit::MAX)
            }
            AnalyzeError::EmptyScope(s) => write!(
                f,
                "no files to analyze in the requested scope: {} excluded as tests, {} as generated, {} as vendored, {} as ignored; {} left out by include or exclude; {} unrecognized. Re-run with the matching include option (tests, generated, vendored, ignored).",
                s.tests, s.generated, s.vendored, s.ignored, s.filtered, s.unrecognized
            ),
            AnalyzeError::Failed { analysis, cause } => write!(f, "{analysis} failed: {cause}"),
            AnalyzeError::Cancelled => f.write_str("analysis cancelled"),
        }
    }
}

impl std::error::Error for AnalyzeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::{Value, json};

    #[derive(Serialize)]
    struct TestFinding {
        #[serde(flatten)]
        location: Location,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        rule: &'static str,
    }

    impl Finding for TestFinding {
        fn location(&self) -> &Location {
            &self.location
        }
        fn order(&self, other: &Self) -> Ordering {
            self.location
                .cmp(&other.location)
                .then_with(|| self.rule.cmp(other.rule))
        }
        fn source(&self) -> Option<&str> {
            self.source.as_deref()
        }
    }

    fn range(r: (u32, u32, u32, u32)) -> CompactRange {
        CompactRange::new(r.0, r.1, r.2, r.3)
    }

    fn finding(file: &str, r: Option<(u32, u32, u32, u32)>) -> TestFinding {
        TestFinding {
            location: Location {
                file: file.to_string(),
                range: r.map(range),
            },
            source: None,
            rule: "rule",
        }
    }

    fn incomplete_none() -> Incomplete {
        Incomplete::new(vec![], vec![], vec![], vec![])
    }

    fn envelope(
        findings: Vec<TestFinding>,
        limit: Limit,
        analyzed_files: u64,
        incomplete: Incomplete,
    ) -> Result<Envelope<TestFinding>, AnalyzeError> {
        Envelope::new(
            "patterns",
            findings,
            limit,
            Coverage::new(vec![], analyzed_files, 0),
            Excluded::new(vec![]),
            incomplete,
            Evidence::new(vec![], vec![]),
            None,
        )
    }

    fn keys(v: &Value) -> Vec<&str> {
        v.as_object().unwrap().keys().map(String::as_str).collect()
    }

    const TOP_KEYS: [&str; 11] = [
        "analysis",
        "status",
        "complete",
        "findings",
        "total",
        "limit",
        "truncated",
        "coverage",
        "excluded",
        "incomplete",
        "evidence",
    ];

    #[test]
    fn empty_envelope_has_every_key_in_order() {
        let env = envelope(vec![], Limit::DEFAULT, 0, incomplete_none()).unwrap();
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(keys(&v), TOP_KEYS);
        assert_eq!(
            keys(&v["coverage"]),
            ["analyzed_files", "languages", "unrecognized_files"]
        );
        assert_eq!(
            keys(&v["excluded"]),
            [
                "tests",
                "generated",
                "vendored",
                "ignored",
                "counts",
                "truncated"
            ]
        );
        assert_eq!(
            keys(&v["excluded"]["counts"]),
            ["tests", "generated", "vendored", "ignored"]
        );
        assert_eq!(
            keys(&v["incomplete"]),
            [
                "skipped_files",
                "parse_failures",
                "unresolved",
                "limits_hit",
                "counts",
                "truncated"
            ]
        );
        assert_eq!(
            keys(&v["incomplete"]["counts"]),
            [
                "skipped_files",
                "parse_failures",
                "unresolved",
                "limits_hit"
            ]
        );
        assert_eq!(keys(&v["evidence"]), ["sources", "language_servers"]);
    }

    #[test]
    fn details_adds_only_details() {
        let mut details = serde_json::Map::new();
        details.insert("formula".into(), json!("a*b"));
        let env = Envelope::<TestFinding>::new(
            "hotspots",
            vec![],
            Limit::DEFAULT,
            Coverage::new(vec![], 1, 0),
            Excluded::new(vec![]),
            incomplete_none(),
            Evidence::new(vec![], vec![]),
            Some(details),
        )
        .unwrap();
        let v = serde_json::to_value(&env).unwrap();
        let mut expected = TOP_KEYS.to_vec();
        expected.push("details");
        assert_eq!(keys(&v), expected);
        assert_eq!(v["details"], json!({"formula": "a*b"}));
    }

    #[test]
    fn status_derive_rules_in_order() {
        assert_eq!(Status::derive(0, true, 0), Status::Unsupported);
        assert_eq!(Status::derive(1, true, 0), Status::Findings);
        assert_eq!(Status::derive(2, false, 3), Status::Findings);
        assert_eq!(Status::derive(0, false, 0), Status::Incomplete);
        assert_eq!(Status::derive(0, false, 4), Status::Incomplete);
        assert_eq!(Status::derive(0, true, 1), Status::NoFindings);
        assert_eq!(
            serde_json::to_value(Status::NoFindings).unwrap(),
            json!("no_findings")
        );
    }

    fn each_missing_evidence() -> Vec<Incomplete> {
        vec![
            Incomplete::new(
                vec![SkippedFile {
                    file: "src/big.rs".into(),
                    kind: SkipKind::SizeLimit,
                    reason: "too big".into(),
                }],
                vec![],
                vec![],
                vec![],
            ),
            Incomplete::new(
                vec![],
                vec![ParseFailure {
                    file: "src/broken.rs".into(),
                    range: range((4, 5, 4, 6)),
                }],
                vec![],
                vec![],
            ),
            Incomplete::new(
                vec![],
                vec![],
                vec![Unresolved {
                    file: "src/a.ts".into(),
                    range: range((1, 1, 1, 20)),
                    kind: "import".into(),
                    target: "./missing".into(),
                    reason: "not found".into(),
                }],
                vec![],
            ),
            Incomplete::new(
                vec![],
                vec![],
                vec![],
                vec![LimitHit {
                    limit: "max_depth".into(),
                    value: 3,
                    detail: "stopped at depth 3".into(),
                }],
            ),
        ]
    }

    #[test]
    fn missing_evidence_is_never_no_findings() {
        for incomplete in each_missing_evidence() {
            assert!(!incomplete.is_complete());
            let env = envelope(vec![], Limit::DEFAULT, 1, incomplete).unwrap();
            assert_eq!(env.status, Status::Incomplete);
            assert!(!env.complete);
        }
    }

    #[test]
    fn missing_evidence_with_a_finding_is_findings_not_complete() {
        for incomplete in each_missing_evidence() {
            let env = envelope(
                vec![finding("src/a.rs", None)],
                Limit::DEFAULT,
                1,
                incomplete,
            )
            .unwrap();
            assert_eq!(env.status, Status::Findings);
            assert!(!env.complete);
        }
    }

    #[test]
    fn limit_out_of_range() {
        assert_eq!(Limit::DEFAULT.get(), 100);
        assert_eq!(Limit::new(1000).unwrap().get(), 1000);
        let err = Limit::new(1001).unwrap_err();
        assert!(matches!(err, AnalyzeError::LimitOutOfRange(1001)));
        assert_eq!(
            err.to_string(),
            "limit must be between 0 and 1000, got 1001"
        );
    }

    fn three_findings() -> Vec<TestFinding> {
        vec![
            finding("src/more.rs", Some((1, 1, 8, 2))),
            finding("src/broken.rs", Some((1, 1, 6, 2))),
            finding("src/lib.rs", Some((1, 1, 8, 2))),
        ]
    }

    #[test]
    fn limit_zero_returns_counts_only() {
        let env = envelope(
            three_findings(),
            Limit::new(0).unwrap(),
            3,
            incomplete_none(),
        )
        .unwrap();
        assert!(env.findings.is_empty());
        assert_eq!(env.total, 3);
        assert!(env.truncated);
        assert_eq!(env.status, Status::Findings);
        assert_eq!(serde_json::to_value(&env).unwrap()["limit"], json!(0));
    }

    #[test]
    fn limit_keeps_the_first_of_the_full_order() {
        let full = envelope(
            three_findings(),
            Limit::new(1000).unwrap(),
            3,
            incomplete_none(),
        )
        .unwrap();
        let cut = envelope(
            three_findings(),
            Limit::new(2).unwrap(),
            3,
            incomplete_none(),
        )
        .unwrap();
        assert!(!full.truncated);
        assert!(cut.truncated);
        assert_eq!(cut.total, 3);
        let full_v = serde_json::to_value(&full).unwrap();
        let cut_v = serde_json::to_value(&cut).unwrap();
        assert_eq!(
            cut_v["findings"].as_array().unwrap()[..],
            full_v["findings"].as_array().unwrap()[..2]
        );
    }

    #[test]
    fn excluded_list_is_cut_to_100_in_path_order() {
        assert_eq!(MAX_EVIDENCE_ENTRIES, 100);
        let paths: Vec<String> = (0..101).map(|i| format!("tests/t{i:03}.rs")).collect();
        let entries = paths
            .iter()
            .rev()
            .map(|p| Exclusion {
                kind: ExcludedKind::Tests,
                entry: ExcludedEntry {
                    path: p.clone(),
                    range: None,
                },
            })
            .collect();
        let excluded = Excluded::new(entries);
        let got: Vec<&str> = excluded.tests.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(got, paths[..100]);
        assert_eq!(excluded.counts.tests, 101);
        assert!(excluded.truncated);
        let v = serde_json::to_value(&excluded).unwrap();
        assert_eq!(v["counts"]["tests"], json!(101));
        assert_eq!(v["tests"].as_array().unwrap().len(), 100);
    }

    #[test]
    fn skipped_files_list_is_cut_to_100_in_path_order() {
        let files: Vec<String> = (0..101).map(|i| format!("src/f{i:03}.rs")).collect();
        let skipped = files
            .iter()
            .rev()
            .map(|f| SkippedFile {
                file: f.clone(),
                kind: SkipKind::Unreadable,
                reason: "permission denied".into(),
            })
            .collect();
        let incomplete = Incomplete::new(skipped, vec![], vec![], vec![]);
        let got: Vec<&str> = incomplete
            .skipped_files
            .iter()
            .map(|s| s.file.as_str())
            .collect();
        assert_eq!(got, files[..100]);
        assert_eq!(incomplete.counts.skipped_files, 101);
        assert!(incomplete.truncated);
        assert!(!incomplete.is_complete());
    }

    #[test]
    fn paths_compare_as_bytes() {
        let env = envelope(
            vec![finding("a/b.rs", None), finding("a-b/x.rs", None)],
            Limit::DEFAULT,
            2,
            incomplete_none(),
        )
        .unwrap();
        assert_eq!(env.findings[0].location.file, "a-b/x.rs");
        assert_eq!(env.findings[1].location.file, "a/b.rs");
    }

    #[test]
    fn missing_range_sorts_first() {
        let env = envelope(
            vec![finding("a.rs", Some((1, 1, 2, 1))), finding("a.rs", None)],
            Limit::DEFAULT,
            1,
            incomplete_none(),
        )
        .unwrap();
        assert!(env.findings[0].location.range.is_none());
        assert!(env.findings[1].location.range.is_some());
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["findings"][0], json!({"file": "a.rs", "rule": "rule"}));
    }

    #[derive(Clone, Copy)]
    enum InputOrder {
        Given,
        Reversed,
        /// Odd positions first, then even positions, then rotated by one.
        Shuffled,
    }

    fn rich_envelope(order: InputOrder) -> String {
        fn permute<T>(v: Vec<T>, order: InputOrder) -> Vec<T> {
            match order {
                InputOrder::Given => v,
                InputOrder::Reversed => v.into_iter().rev().collect(),
                InputOrder::Shuffled => {
                    let (mut odd, even): (Vec<_>, Vec<_>) =
                        v.into_iter().enumerate().partition(|(i, _)| i % 2 == 1);
                    odd.extend(even);
                    if !odd.is_empty() {
                        odd.rotate_left(1);
                    }
                    odd.into_iter().map(|(_, x)| x).collect()
                }
            }
        }
        let findings = vec![
            finding("b.rs", Some((3, 1, 4, 1))),
            finding("b.rs", Some((1, 1, 9, 1))),
            finding("b.rs", Some((1, 1, 2, 1))),
            finding("a/b.rs", None),
            finding("a-b/x.rs", Some((1, 2, 1, 3))),
        ];
        let languages = vec![
            LanguageCoverage {
                language: "TOML",
                files: 1,
                supported: false,
            },
            LanguageCoverage {
                language: "Rust",
                files: 4,
                supported: true,
            },
            LanguageCoverage {
                language: "Python",
                files: 2,
                supported: true,
            },
        ];
        let exclusion = |kind, path: &str, r: Option<(u32, u32, u32, u32)>| Exclusion {
            kind,
            entry: ExcludedEntry {
                path: path.into(),
                range: r.map(range),
            },
        };
        let exclusions = vec![
            exclusion(ExcludedKind::Tests, "tests/z.rs", None),
            exclusion(ExcludedKind::Tests, "src/lib.rs", Some((10, 1, 14, 2))),
            exclusion(ExcludedKind::Tests, "src/lib.rs", Some((3, 1, 5, 2))),
            exclusion(ExcludedKind::Ignored, "out/", None),
            exclusion(ExcludedKind::Vendored, "vendor/", None),
            exclusion(ExcludedKind::Generated, "src/gen.rs", None),
            exclusion(ExcludedKind::Generated, "gen/", None),
        ];
        let skipped = vec![
            SkippedFile {
                file: "src/x.rs".into(),
                kind: SkipKind::Unreadable,
                reason: "r".into(),
            },
            SkippedFile {
                file: "src/x.rs".into(),
                kind: SkipKind::NotUtf8,
                reason: "r".into(),
            },
            SkippedFile {
                file: "lib/".into(),
                kind: SkipKind::WalkError,
                reason: "r".into(),
            },
        ];
        let failures = vec![
            ParseFailure {
                file: "z.rs".into(),
                range: range((1, 1, 1, 1)),
            },
            ParseFailure {
                file: "a.rs".into(),
                range: range((2, 1, 2, 1)),
            },
        ];
        let unresolved = vec![
            Unresolved {
                file: "a.ts".into(),
                range: range((1, 1, 1, 9)),
                kind: "import".into(),
                target: "./b".into(),
                reason: "r".into(),
            },
            Unresolved {
                file: "a.ts".into(),
                range: range((1, 1, 1, 9)),
                kind: "import".into(),
                target: "./a".into(),
                reason: "r".into(),
            },
        ];
        let limits = vec![
            LimitHit {
                limit: "max_depth".into(),
                value: 5,
                detail: "d".into(),
            },
            LimitHit {
                limit: "max_depth".into(),
                value: 3,
                detail: "d".into(),
            },
        ];
        let servers = vec![
            LanguageServerUse {
                language: "Rust",
                server: "rust-analyzer",
                state: ServerState::Disabled,
                reason: None,
            },
            LanguageServerUse {
                language: "Python",
                server: "pyright",
                state: ServerState::Failed,
                reason: Some("exited".into()),
            },
        ];
        let env = Envelope::new(
            "patterns",
            permute(findings, order),
            Limit::new(4).unwrap(),
            Coverage::new(permute(languages, order), 6, 1),
            Excluded::new(permute(exclusions, order)),
            Incomplete::new(
                permute(skipped, order),
                permute(failures, order),
                permute(unresolved, order),
                permute(limits, order),
            ),
            Evidence::new(
                permute(vec!["tree-sitter".to_string()], order),
                permute(servers, order),
            ),
            None,
        )
        .unwrap();
        serde_json::to_string(&env).unwrap()
    }

    #[test]
    fn output_is_independent_of_input_order() {
        let identity = rich_envelope(InputOrder::Given);
        let reversed = rich_envelope(InputOrder::Reversed);
        let shuffled = rich_envelope(InputOrder::Shuffled);
        assert_eq!(identity, reversed);
        assert_eq!(identity, shuffled);
        let v: Value = serde_json::from_str(&identity).unwrap();
        assert_eq!(v["findings"][0]["file"], json!("a-b/x.rs"));
        assert_eq!(v["coverage"]["languages"][0]["language"], json!("Python"));
        assert_eq!(v["incomplete"]["skipped_files"][0]["file"], json!("lib/"));
        assert_eq!(
            v["incomplete"]["skipped_files"][1]["kind"],
            json!("not_utf8")
        );
        assert_eq!(v["incomplete"]["unresolved"][0]["target"], json!("./a"));
        assert_eq!(v["incomplete"]["limits_hit"][0]["value"], json!(3));
        assert_eq!(v["excluded"]["tests"][0]["range"], json!("3:1-5:2"));
        assert_eq!(
            v["evidence"]["language_servers"][0],
            json!({"language": "Python", "server": "pyright", "state": "failed", "reason": "exited"})
        );
        assert_eq!(
            v["evidence"]["language_servers"][1],
            json!({"language": "Rust", "server": "rust-analyzer", "state": "disabled"})
        );
    }

    #[test]
    fn decimal_rounds_half_to_even() {
        let s = |x| serde_json::to_string(&decimal(x)).unwrap();
        assert_eq!(s(0.0625), "0.062");
        assert_eq!(s(0.1875), "0.188");
        assert_eq!(s(2.0), "2");
        assert_eq!(s(1.5), "1.5");
        assert!(decimal(2.0).is_u64());
    }

    fn worked_example_findings() -> Envelope<TestFinding> {
        let exclusion = |kind, path: &str, r: Option<(u32, u32, u32, u32)>| Exclusion {
            kind,
            entry: ExcludedEntry {
                path: path.into(),
                range: r.map(range),
            },
        };
        let mut findings = three_findings();
        for f in &mut findings {
            f.rule = "";
        }
        Envelope::new(
            "patterns",
            findings,
            Limit::new(2).unwrap(),
            Coverage::new(
                vec![
                    LanguageCoverage {
                        language: "TOML",
                        files: 1,
                        supported: false,
                    },
                    LanguageCoverage {
                        language: "Rust",
                        files: 4,
                        supported: true,
                    },
                ],
                3,
                2,
            ),
            Excluded::new(vec![
                exclusion(ExcludedKind::Ignored, "build-out/", None),
                exclusion(ExcludedKind::Tests, "tests/cli.rs", None),
                exclusion(ExcludedKind::Vendored, "vendor/", None),
                exclusion(ExcludedKind::Generated, "src/gen.rs", None),
                exclusion(ExcludedKind::Tests, "src/lib.rs", Some((10, 1, 14, 2))),
            ]),
            Incomplete::new(
                vec![SkippedFile {
                    file: "src/big.rs".into(),
                    kind: SkipKind::SizeLimit,
                    reason: "exceeds size cap: 8388609 bytes > cap of 8388608 bytes".into(),
                }],
                vec![ParseFailure {
                    file: "src/broken.rs".into(),
                    range: range((4, 5, 4, 6)),
                }],
                vec![],
                vec![],
            ),
            Evidence::new(vec!["tree-sitter".into()], vec![]),
            None,
        )
        .unwrap()
    }

    /// A finding object with only the common keys, as the worked examples show.
    fn strip_rule(mut v: Value) -> Value {
        for f in v["findings"].as_array_mut().unwrap() {
            f.as_object_mut().unwrap().remove("rule");
        }
        v
    }

    #[test]
    fn worked_example_findings_truncated_incomplete() {
        let expected: Value = serde_json::from_str(
            r#"{
  "analysis": "patterns",
  "status": "findings",
  "complete": false,
  "findings": [
    {"file": "src/broken.rs", "range": "1:1-6:2"},
    {"file": "src/lib.rs", "range": "1:1-8:2"}
  ],
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
    "skipped_files": [
      {"file": "src/big.rs", "kind": "size_limit", "reason": "exceeds size cap: 8388609 bytes > cap of 8388608 bytes"}
    ],
    "parse_failures": [{"file": "src/broken.rs", "range": "4:5-4:6"}],
    "unresolved": [],
    "limits_hit": [],
    "counts": {"skipped_files": 1, "parse_failures": 1, "unresolved": 0, "limits_hit": 0},
    "truncated": false
  },
  "evidence": {"sources": ["tree-sitter"], "language_servers": []}
}"#,
        )
        .unwrap();
        let got = strip_rule(serde_json::to_value(worked_example_findings()).unwrap());
        assert_eq!(got, expected);
        assert_eq!(keys(&got), TOP_KEYS);
    }

    #[test]
    fn worked_example_unsupported() {
        let expected: Value = serde_json::from_str(
            r#"{"analysis":"patterns","status":"unsupported","complete":true,"findings":[],"total":0,"limit":100,"truncated":false,
 "coverage":{"analyzed_files":0,"languages":[{"language":"TOML","files":1,"supported":false}],"unrecognized_files":0},
 "excluded":{"tests":[],"generated":[],"vendored":[],"ignored":[],"counts":{"tests":0,"generated":0,"vendored":0,"ignored":0},"truncated":false},
 "incomplete":{"skipped_files":[],"parse_failures":[],"unresolved":[],"limits_hit":[],"counts":{"skipped_files":0,"parse_failures":0,"unresolved":0,"limits_hit":0},"truncated":false},
 "evidence":{"sources":[],"language_servers":[]}}"#,
        )
        .unwrap();
        let env = Envelope::<TestFinding>::new(
            "patterns",
            vec![],
            Limit::DEFAULT,
            Coverage::new(
                vec![LanguageCoverage {
                    language: "TOML",
                    files: 1,
                    supported: false,
                }],
                0,
                0,
            ),
            Excluded::new(vec![]),
            incomplete_none(),
            Evidence::new(vec![], vec![]),
            None,
        )
        .unwrap();
        let got = serde_json::to_value(&env).unwrap();
        assert_eq!(got, expected);
        assert_eq!(keys(&got), TOP_KEYS);
    }

    #[test]
    fn evidence_sources_sorted_and_deduplicated() {
        let evidence = Evidence::new(
            vec![
                "tree-sitter".into(),
                "lsp:rust-analyzer".into(),
                "tree-sitter".into(),
            ],
            vec![],
        );
        assert_eq!(
            serde_json::to_value(&evidence).unwrap(),
            json!({"sources": ["lsp:rust-analyzer", "tree-sitter"], "language_servers": []})
        );
    }

    fn two_sources(findings: Vec<TestFinding>) -> Result<Envelope<TestFinding>, AnalyzeError> {
        Envelope::new(
            "patterns",
            findings,
            Limit::DEFAULT,
            Coverage::new(vec![], 1, 0),
            Excluded::new(vec![]),
            incomplete_none(),
            Evidence::new(
                vec!["tree-sitter".into(), "lsp:rust-analyzer".into()],
                vec![],
            ),
            None,
        )
    }

    #[test]
    fn several_sources_need_a_source_on_every_finding() {
        let mut with = finding("a.rs", None);
        with.source = Some("tree-sitter".into());
        let without = finding("b.rs", None);
        let err = two_sources(vec![with, without]).err().unwrap();
        assert!(matches!(
            err,
            AnalyzeError::Failed {
                analysis: "patterns",
                ..
            }
        ));
        assert!(!err.is_invalid_params());

        let mut a = finding("a.rs", None);
        a.source = Some("tree-sitter".into());
        let mut b = finding("b.rs", None);
        b.source = Some("lsp:rust-analyzer".into());
        let v = serde_json::to_value(two_sources(vec![a, b]).unwrap()).unwrap();
        assert_eq!(v["findings"][1]["source"], json!("lsp:rust-analyzer"));
    }

    fn with_details(details: Value) -> Result<Envelope<TestFinding>, AnalyzeError> {
        Envelope::new(
            "hotspots",
            vec![],
            Limit::DEFAULT,
            Coverage::new(vec![], 1, 0),
            Excluded::new(vec![]),
            incomplete_none(),
            Evidence::new(vec![], vec![]),
            Some(details.as_object().unwrap().clone()),
        )
    }

    #[test]
    fn details_array_needs_a_total() {
        let err = with_details(json!({"weights": [1, 2]})).err().unwrap();
        assert!(matches!(
            err,
            AnalyzeError::Failed {
                analysis: "hotspots",
                ..
            }
        ));
        assert!(with_details(json!({"weights": [1, 2], "weights_total": "2"})).is_err());
        assert!(with_details(json!({"inner": {"weights": [1]}})).is_err());
        let env = with_details(json!({"weights": [1, 2], "weights_total": 2})).unwrap();
        assert_eq!(
            serde_json::to_value(&env).unwrap()["details"],
            json!({"weights": [1, 2], "weights_total": 2})
        );
    }

    #[test]
    fn rel_path_form() {
        let root = Path::new("/w");
        assert_eq!(rel_path(root, Path::new("/w/src/a.rs"), false), "src/a.rs");
        assert_eq!(
            rel_path(root, Path::new("/w/./src/a.rs"), false),
            "src/a.rs"
        );
        assert_eq!(rel_path(root, Path::new("/w"), true), ".");
        assert_eq!(rel_path(root, Path::new("/w/vendor"), true), "vendor/");
        assert_eq!(
            rel_path(Path::new("."), Path::new("./src/x/a.rs"), false),
            "src/x/a.rs"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rel_path_replaces_invalid_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new("/w/src").join(OsStr::from_bytes(b"bad\xff.rs"));
        assert_eq!(
            rel_path(Path::new("/w"), &path, false),
            "src/bad\u{FFFD}.rs"
        );
    }

    #[test]
    fn error_messages() {
        let not_found = AnalyzeError::NotFound("The path 'x.rs' does not exist.".into());
        assert_eq!(not_found.to_string(), "The path 'x.rs' does not exist.");
        assert!(not_found.is_invalid_params());

        let outside = AnalyzeError::OutsideRoot("../x.rs".into());
        assert_eq!(
            outside.to_string(),
            "path must be inside the workspace root: ../x.rs"
        );
        assert!(outside.is_invalid_params());

        let limit = AnalyzeError::LimitOutOfRange(4294967295);
        assert_eq!(
            limit.to_string(),
            "limit must be between 0 and 1000, got 4294967295"
        );
        assert!(limit.is_invalid_params());

        let empty = AnalyzeError::EmptyScope(EmptyScope {
            tests: 1,
            generated: 0,
            vendored: 0,
            ignored: 0,
            filtered: 0,
            unrecognized: 0,
        });
        assert_eq!(
            empty.to_string(),
            "no files to analyze in the requested scope: 1 excluded as tests, 0 as generated, 0 as vendored, 0 as ignored; 0 left out by include or exclude; 0 unrecognized. Re-run with the matching include option (tests, generated, vendored, ignored)."
        );
        assert!(empty.is_invalid_params());

        let failed = AnalyzeError::Failed {
            analysis: "patterns",
            cause: "worker panicked".into(),
        };
        assert_eq!(failed.to_string(), "patterns failed: worker panicked");
        assert!(!failed.is_invalid_params());

        assert!(!AnalyzeError::Cancelled.is_invalid_params());
    }
}
