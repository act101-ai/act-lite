//! MCP tool parameter structs and shared response helpers.

use crate::analyze::discovery::Includes;
use crate::analyze::envelope::{AnalyzeError, Excluded, Incomplete, MAX_EVIDENCE_ENTRIES};
use crate::analyze::run::Request;
use rmcp::model::{CallToolResult, ContentBlock};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Parameters for the skeleton tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkeletonParams {
    /// Path to the file to analyze
    pub file: String,
}

/// Parameters for the symbols tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SymbolsParams {
    /// Path to the file to analyze
    pub file: String,
    /// Maximum number of symbols to return. Default: 50.
    #[serde(
        default = "default_symbols_limit",
        deserialize_with = "deserialize_u32_lax"
    )]
    pub limit: u32,
    /// Regex filter on symbol name (case-insensitive). Invalid regex returns INVALID_PARAMS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Filter by symbol kind, e.g. ["function", "class"]. Case-insensitive. Matches any in list.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_string_vec_lax"
    )]
    pub kind: Option<Vec<String>>,
}

/// Parameters for the symbols_batch tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SymbolsBatchParams {
    /// File paths to retrieve all symbols from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    /// Stable symbol IDs to retrieve
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
    /// Filter by symbol kind
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_string_vec_lax"
    )]
    pub kinds: Option<Vec<String>>,
    /// Name regex, as `symbols` takes. Without it this returns every symbol in
    /// every file listed — three files of Textualize/rich is ~10,000 tokens,
    /// which then sits in context for the rest of the session. Filter here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Parameters for the repo_outline tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoOutlineParams {
    /// Maximum directory depth
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// Glob pattern to include
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    /// Glob pattern to exclude
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<String>,
    /// Include top-level symbol names per file
    #[serde(default)]
    pub symbols: bool,
    /// Maximum files to include
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files: Option<u32>,

    /// Subtree path to traverse, relative to the workspace root (e.g. "crates/act-lite-parser").
    /// Omit to traverse the full workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// When true, returned file paths are relative to `path` instead of the workspace root.
    /// Default: false (paths are workspace-relative).
    #[serde(default)]
    pub relative: bool,
}

/// Parameters for the references tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReferencesParams {
    /// Symbol name to find references for
    pub symbol: String,
    /// File path containing the symbol — required to locate it for LSP
    pub file: String,
}

/// Parameters for the definition tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DefinitionParams {
    /// Symbol name to find definition for
    pub symbol: String,
    /// File path where the symbol is used
    pub file: String,
    /// Line number of the symbol usage (1-indexed)
    #[serde(deserialize_with = "deserialize_u32_lax")]
    pub line: u32,
    /// Column number of the symbol usage (1-indexed)
    #[serde(deserialize_with = "deserialize_u32_lax")]
    pub column: u32,
}

/// Convert a skeleton result to compact JSON: no file paths, no byte offsets, no columns, no imports.
pub(crate) fn compact_skeleton(result: &act_lite_parser::QueryResult) -> serde_json::Value {
    if let act_lite_parser::QueryResult::Skeleton(skeleton) = result {
        let declarations: Vec<serde_json::Value> = skeleton
            .declarations
            .iter()
            .filter(|d| d.kind != "import")
            .map(|d| {
                serde_json::json!({
                    "kind": d.kind,
                    "name": d.name,
                    "line": d.range.start().line(),
                    "end_line": d.range.end().line(),
                })
            })
            .collect();
        serde_json::json!({ "declarations": declarations })
    } else {
        serde_json::json!({ "error": "unexpected result type" })
    }
}

/// Build a successful tool result with data and a contextual hint about related tools.
pub(crate) fn success_with_hint(json: String, hint: &str) -> CallToolResult {
    CallToolResult::success(vec![
        ContentBlock::text(json),
        ContentBlock::text(format!("\n---\n_Hint: {hint}_")),
    ])
}

pub(crate) fn default_symbols_limit() -> u32 {
    50
}

/// Deserializes a `u32` that may arrive as a JSON number or a JSON string.
/// AI models sometimes emit `"line": "13"` instead of `"line": 13`.
pub(crate) fn deserialize_u32_lax<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Unexpected, Visitor};

    struct U32OrString;

    impl<'de> Visitor<'de> for U32OrString {
        type Value = u32;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a u32 or a string containing a u32")
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u32, E> {
            u32::try_from(v).map_err(|_| E::invalid_value(Unexpected::Unsigned(v), &self))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u32, E> {
            u32::try_from(v).map_err(|_| E::invalid_value(Unexpected::Signed(v), &self))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<u32, E> {
            v.parse::<u32>()
                .map_err(|_| E::invalid_value(Unexpected::Str(v), &self))
        }
    }

    deserializer.deserialize_any(U32OrString)
}

/// Deserializes an `Option<Vec<String>>` that may arrive as a JSON array or a
/// stringified JSON array. AI models sometimes emit `"kind": "[\"function\"]"`
/// instead of `"kind": ["function"]`.
pub(crate) fn deserialize_string_vec_lax<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};

    struct StringVecOrString;

    impl<'de> Visitor<'de> for StringVecOrString {
        type Value = Option<Vec<String>>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a JSON array of strings or a stringified JSON array")
        }

        fn visit_none<E: de::Error>(self) -> Result<Option<Vec<String>>, E> {
            Ok(None)
        }

        fn visit_some<D2: serde::Deserializer<'de>>(
            self,
            deserializer: D2,
        ) -> Result<Option<Vec<String>>, D2::Error> {
            deserializer.deserialize_any(StringVecOrString)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Option<Vec<String>>, A::Error> {
            let mut v = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                v.push(s);
            }
            Ok(Some(v))
        }

        fn visit_str<E: de::Error>(self, s: &str) -> Result<Option<Vec<String>>, E> {
            let v: Vec<String> = serde_json::from_str(s)
                .map_err(|_| E::custom(format!("expected a JSON array of strings, got: {s:?}")))?;
            Ok(Some(v))
        }
    }

    deserializer.deserialize_option(StringVecOrString)
}

/// Parameters for the callers tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CallersParams {
    /// Name of the function/method to find callers for
    pub symbol: String,
    /// File path containing the symbol — required to locate it for the language server
    pub file: String,
}

/// The parameters every analyzer takes. An analyzer's own parameter struct
/// embeds this one with `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeParams {
    /// Files or directories to analyze, relative to the workspace root. Default: the root.
    #[serde(default, deserialize_with = "deserialize_string_vec_or_empty")]
    pub paths: Vec<String>,
    /// Analyze only files that match at least one of these globs
    #[serde(default, deserialize_with = "deserialize_string_vec_or_empty")]
    pub include: Vec<String>,
    /// Leave out files that match any of these globs
    #[serde(default, deserialize_with = "deserialize_string_vec_or_empty")]
    pub exclude: Vec<String>,
    /// Analyze test files and embedded test code. Default: false.
    #[serde(default)]
    pub include_tests: bool,
    /// Analyze generated files and directories. Default: false.
    #[serde(default)]
    pub include_generated: bool,
    /// Analyze vendored directories. Default: false.
    #[serde(default)]
    pub include_vendored: bool,
    /// Analyze ignored paths. Default: false.
    #[serde(default)]
    pub include_ignored: bool,
    /// The most findings to return, 0 to 1000. Default: 100.
    #[serde(
        default = "default_analyze_limit",
        deserialize_with = "deserialize_u32_lax"
    )]
    pub limit: u32,
}

/// Parameters for the analyze_patterns tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PatternsParams {
    #[serde(flatten)]
    pub shared: AnalyzeParams,
}

/// Parameters for the analyze_hotspots tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HotspotsParams {
    #[serde(flatten)]
    pub shared: AnalyzeParams,
}

/// Parameters for the analyze_clones tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClonesParams {
    #[serde(flatten)]
    pub shared: AnalyzeParams,
    /// Report only the clone classes with a member at or below one of these paths,
    /// relative to the workspace root. Default: every class.
    #[serde(default, deserialize_with = "deserialize_string_vec_or_empty")]
    pub focus: Vec<String>,
}

/// Parameters for the analyze_fixture tool.
#[cfg(debug_assertions)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FixtureParams {
    #[serde(flatten)]
    pub shared: AnalyzeParams,
}

impl AnalyzeParams {
    /// The checked request for the workspace `root`.
    pub fn request(&self, root: &Path) -> Result<Request, AnalyzeError> {
        Request::new(
            root,
            &self.paths,
            &self.include,
            &self.exclude,
            Includes {
                tests: self.include_tests,
                generated: self.include_generated,
                vendored: self.include_vendored,
                ignored: self.include_ignored,
            },
            self.limit,
        )
    }
}

fn default_analyze_limit() -> u32 {
    100
}

/// Like [`deserialize_string_vec_lax`], with an absent or null array read as
/// empty.
fn deserialize_string_vec_or_empty<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_string_vec_lax(deserializer).map(Option::unwrap_or_default)
}

/// The hint shown with an analyzer's result: how to narrow a truncated
/// result, and the counts of any cut evidence list; else `usual`.
pub fn analyze_hint(
    total: u64,
    truncated: bool,
    excluded: &Excluded,
    incomplete: &Incomplete,
    usual: &str,
) -> String {
    let mut parts = Vec::new();
    if truncated {
        parts.push(format!(
            "The result holds {total} findings in total and is cut to `limit`. Narrow it with `paths`, `include` or `exclude`, or change `limit` (0 to 1000)."
        ));
    }
    if excluded.truncated {
        let c = &excluded.counts;
        parts.push(format!(
            "The `excluded` lists are cut at {MAX_EVIDENCE_ENTRIES} entries each; the counts are tests {}, generated {}, vendored {}, ignored {}.",
            c.tests, c.generated, c.vendored, c.ignored
        ));
    }
    if incomplete.truncated {
        let c = &incomplete.counts;
        parts.push(format!(
            "The `incomplete` lists are cut at {MAX_EVIDENCE_ENTRIES} entries each; the counts are skipped_files {}, parse_failures {}, unresolved {}, limits_hit {}.",
            c.skipped_files, c.parse_failures, c.unresolved, c.limits_hit
        ));
    }
    if parts.is_empty() {
        usual.to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::envelope::{ExcludedEntry, ExcludedKind, Exclusion, SkipKind, SkippedFile};

    fn excluded(tests: usize) -> Excluded {
        Excluded::new(
            (0..tests)
                .map(|i| Exclusion {
                    kind: ExcludedKind::Tests,
                    entry: ExcludedEntry {
                        path: format!("tests/t{i:03}.rs"),
                        range: None,
                    },
                })
                .collect(),
        )
    }

    fn incomplete(skipped: usize) -> Incomplete {
        let skipped = (0..skipped)
            .map(|i| SkippedFile {
                file: format!("s{i:03}.rs"),
                kind: SkipKind::Unreadable,
                reason: "no access".to_string(),
            })
            .collect();
        Incomplete::new(skipped, Vec::new(), Vec::new(), Vec::new())
    }

    #[test]
    fn untruncated_result_gets_the_usual_hint() {
        let hint = analyze_hint(3, false, &excluded(100), &incomplete(100), "usual");
        assert_eq!(hint, "usual");
    }

    #[test]
    fn truncated_result_names_the_narrowing_parameters_and_total() {
        let hint = analyze_hint(3, true, &excluded(0), &incomplete(0), "usual");
        for word in ["`paths`", "`include`", "`exclude`", "`limit`", "3 findings"] {
            assert!(hint.contains(word), "{word}: {hint}");
        }
        assert!(!hint.contains("usual"), "{hint}");
    }

    #[test]
    fn cut_evidence_lists_give_their_counts() {
        let hint = analyze_hint(0, false, &excluded(101), &incomplete(102), "usual");
        assert!(
            hint.contains("tests 101, generated 0, vendored 0, ignored 0"),
            "{hint}"
        );
        assert!(
            hint.contains("skipped_files 102, parse_failures 0, unresolved 0, limits_hit 0"),
            "{hint}"
        );
        assert!(!hint.contains("`limit`"), "{hint}");
    }
}
