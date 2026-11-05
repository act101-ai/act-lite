//! The `clones` analyzer: syntax subtrees duplicated across the analyzed files.

use super::envelope::{AnalyzeError, Finding, Location, rel_path};
use super::read::{FileSource, Parsed};
use super::run::{Analysis, Analyzer, Request};
use crate::mcp::path_error::enrich_not_found_error;
use act_lite_core::CompactRange;
use act_lite_parser::Grammar;
use act_lite_parser::function_model::{self, is_comment, is_identifier_token};
use serde::Serialize;
use serde_json::json;
use siphasher::sip128::{Hasher128, SipHasher13};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hasher;
use tokio_util::sync::CancellationToken;
use tree_sitter::Node;

/// The smallest normalized size of a clone, in tokens.
pub const MIN_TOKENS: u32 = 50;

/// The most members a finding lists.
const MAX_MEMBERS: usize = 20;

/// The most entries of each focus list in `details`.
const MAX_FOCUS: usize = 30;

/// The candidates handled between two cancellation checks.
const CHECK_EVERY: usize = 4096;

/// The fixed keys of the form hash.
const KEYS: (u64, u64) = (0x6163_742d_6c69_7465, 0x636c_6f6e_6573_3031);

const EXACT_RULE: &str = "same syntax tree; formatting and comments ignored";
const RENAMED_RULE: &str = "identifiers renamed consistently by first appearance; string, character, numeric and symbol literals abstracted; booleans and null kept";

/// Reports classes of duplicated syntax subtrees: candidates of at least
/// [`MIN_TOKENS`] tokens with the same language and renamed form, outermost
/// classes only.
pub struct ClonesAnalyzer {
    /// The focus paths, workspace-relative, in byte order.
    focus: Vec<String>,
}

impl ClonesAnalyzer {
    /// Checks each `focus` path in order: it exists, it is inside the
    /// workspace root, and it is at or below a path of `request`.
    pub fn new(request: &Request, focus: &[String]) -> Result<ClonesAnalyzer, AnalyzeError> {
        let root = &request.root;
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
            if !request.scope.paths.iter().any(|p| resolved.starts_with(p)) {
                return Err(AnalyzeError::FocusOutsidePaths(input.clone()));
            }
            paths.push(rel_path(root, &resolved, target.is_dir())?);
        }
        paths.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        paths.dedup();
        Ok(ClonesAnalyzer { focus: paths })
    }

    fn in_focus(&self, file: &str) -> bool {
        self.focus.iter().any(|f| under(file, f))
    }
}

/// Whether `file` is at or below the workspace-relative path `path`.
fn under(file: &str, path: &str) -> bool {
    path == "." || file == path || (path.ends_with('/') && file.starts_with(path))
}

/// The variety of a clone class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneKind {
    /// Every member has the same exact form.
    Exact,
    /// The members differ in identifier names or literal values.
    Renamed,
}

/// One member of a clone class: its node's location and its variant.
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    #[serde(flatten)]
    pub location: Location,
    pub variant: u32,
}

/// One clone class. The subject is its first member.
#[derive(Debug, Clone, Serialize)]
pub struct CloneFinding {
    #[serde(flatten)]
    pub location: Location,
    pub kind: CloneKind,
    pub language: &'static str,
    /// The normalized size of each member.
    pub tokens: u32,
    /// In location order, members in focus files first; at most 20.
    pub members: Vec<Member>,
    pub members_total: u64,
    /// The number of distinct exact forms.
    pub variants: u32,
    /// The subject's node count: orders a wrapper before the node it wraps
    /// at the same location.
    #[serde(skip)]
    nodes: u32,
}

impl Finding for CloneFinding {
    fn location(&self) -> &Location {
        &self.location
    }

    /// Larger first, then more members, then the subject's location, then
    /// the outer node first.
    fn order(&self, other: &Self) -> Ordering {
        other
            .tokens
            .cmp(&self.tokens)
            .then_with(|| other.members_total.cmp(&self.members_total))
            .then_with(|| self.location.cmp(&other.location))
            .then_with(|| other.nodes.cmp(&self.nodes))
    }
}

/// What is kept of a candidate node between the read and the grouping.
struct Candidate {
    file: u32,
    grammar: Grammar,
    range: CompactRange,
    tokens: u32,
    /// The nodes of the subtree, comments left out; a replaced literal's
    /// nodes all count, so a proper ancestor always has more.
    nodes: u32,
    exact: u128,
    blind: u128,
}

/// An analyzed file.
struct File {
    rel: String,
    /// The index of its first candidate.
    first: usize,
}

impl Analyzer for ClonesAnalyzer {
    const NAME: &'static str = "clones";
    const HINT: &'static str = "Narrow the result with paths, include or exclude, or change limit.";
    type Finding = CloneFinding;

    fn supports(&self, grammar: Grammar) -> bool {
        function_model::supports(grammar)
    }

    fn analyze(
        &self,
        source: &mut dyn FileSource,
        cancel: &CancellationToken,
    ) -> Result<Analysis<CloneFinding>, AnalyzeError> {
        let mut files: Vec<File> = Vec::new();
        let mut candidates: Vec<Candidate> = Vec::new();
        while let Some(parsed) = source.next_file()? {
            let file = files.len() as u32;
            let first = candidates.len();
            walk(&parsed, cancel, |_, c| {
                candidates.push(Candidate { file, ..c });
                Ok(())
            })?;
            files.push(File {
                rel: parsed.rel,
                first,
            });
        }
        checkpoint(cancel)?;

        let renamed = renamed_hashes(source, &files, &candidates, cancel)?;
        let mut classes: HashMap<(Grammar, u128), Vec<usize>> = HashMap::new();
        for (n, (&i, &hash)) in renamed.iter().enumerate() {
            if n % CHECK_EVERY == 0 {
                checkpoint(cancel)?;
            }
            classes
                .entry((candidates[i].grammar, hash))
                .or_default()
                .push(i);
        }
        let mut classes: Vec<Vec<usize>> = classes.into_values().filter(|m| m.len() > 1).collect();
        let location = |i: usize| {
            let c = &candidates[i];
            (c.file, range_key(&c.range))
        };
        for members in &mut classes {
            members.sort_by_key(|&i| location(i));
        }
        // A proper ancestor has more nodes than its descendant, so a class
        // is decided before every class with a member inside its largest.
        let largest = |m: &[usize]| m.iter().map(|&i| candidates[i].nodes).max();
        classes.sort_by(|a, b| {
            largest(b)
                .cmp(&largest(a))
                .then_with(|| location(a[0]).cmp(&location(b[0])))
        });

        let mut reported: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut findings = Vec::new();
        for members in classes {
            checkpoint(cancel)?;
            let inside = |&i: &usize| {
                let m = &candidates[i];
                reported.get(&m.file).is_some_and(|outer| {
                    outer
                        .iter()
                        .any(|&o| contains(&candidates[o], m) && candidates[o].nodes > m.nodes)
                })
            };
            if members.iter().all(inside) {
                continue;
            }
            for &i in &members {
                reported.entry(candidates[i].file).or_default().push(i);
            }
            if self.focus.is_empty()
                || members
                    .iter()
                    .any(|&i| self.in_focus(&files[candidates[i].file as usize].rel))
            {
                findings.push(self.finding(&members, &candidates, &files));
            }
        }
        checkpoint(cancel)?;

        let not_analyzed: Vec<&String> = self
            .focus
            .iter()
            .filter(|f| !files.iter().any(|file| under(&file.rel, f)))
            .collect();
        let details = json!({
            "min_tokens": MIN_TOKENS,
            "exact": EXACT_RULE,
            "renamed": RENAMED_RULE,
            "candidates": candidates.len(),
            "focus": self.focus.iter().take(MAX_FOCUS).collect::<Vec<_>>(),
            "focus_total": self.focus.len(),
            "focus_not_analyzed": not_analyzed.iter().take(MAX_FOCUS).collect::<Vec<_>>(),
            "focus_not_analyzed_total": not_analyzed.len(),
        });
        Ok(Analysis {
            findings,
            unresolved: Vec::new(),
            limits_hit: Vec::new(),
            details: details.as_object().cloned(),
        })
    }
}

impl ClonesAnalyzer {
    /// The finding of a reported class whose members are in location order.
    fn finding(&self, members: &[usize], candidates: &[Candidate], files: &[File]) -> CloneFinding {
        let mut forms: Vec<u128> = Vec::new();
        let mut listed: Vec<(bool, Member)> = members
            .iter()
            .map(|&i| {
                let c = &candidates[i];
                let variant = match forms.iter().position(|&f| f == c.exact) {
                    Some(p) => p + 1,
                    None => {
                        forms.push(c.exact);
                        forms.len()
                    }
                };
                let file = &files[c.file as usize].rel;
                let member = Member {
                    location: Location {
                        file: file.clone(),
                        range: Some(c.range.clone()),
                    },
                    variant: variant as u32,
                };
                (self.in_focus(file), member)
            })
            .collect();
        listed.sort_by_key(|(focused, _)| !focused);
        let first = &candidates[members[0]];
        let mut members: Vec<Member> = listed.into_iter().map(|(_, m)| m).collect();
        let members_total = members.len() as u64;
        members.truncate(MAX_MEMBERS);
        CloneFinding {
            location: members[0].location.clone(),
            kind: if forms.len() == 1 {
                CloneKind::Exact
            } else {
                CloneKind::Renamed
            },
            language: first.grammar.name(),
            tokens: first.tokens,
            members,
            members_total,
            variants: forms.len() as u32,
            nodes: first.nodes,
        }
    }
}

/// The renamed-form hash of each candidate whose blind hash another
/// candidate of its language shares, by candidate index. Only the files
/// holding such candidates are read again.
fn renamed_hashes(
    source: &mut dyn FileSource,
    files: &[File],
    candidates: &[Candidate],
    cancel: &CancellationToken,
) -> Result<BTreeMap<usize, u128>, AnalyzeError> {
    let mut blind: HashMap<(Grammar, u128), u32> = HashMap::new();
    for (n, c) in candidates.iter().enumerate() {
        if n % CHECK_EVERY == 0 {
            checkpoint(cancel)?;
        }
        *blind.entry((c.grammar, c.blind)).or_default() += 1;
    }
    let shared = |c: &Candidate| blind[&(c.grammar, c.blind)] > 1;
    let mut renamed = BTreeMap::new();
    for (n, file) in files.iter().enumerate() {
        let end = files.get(n + 1).map_or(candidates.len(), |next| next.first);
        let own = &candidates[file.first..end];
        if !own.iter().any(shared) {
            continue;
        }
        let parsed = source.reread(&file.rel)?;
        let mut ordinal = 0;
        walk(&parsed, cancel, |node, found| {
            let index = file.first + ordinal;
            let Some(kept) = own.get(ordinal) else {
                return Err(changed(&file.rel));
            };
            if kept.range != found.range || kept.exact != found.exact {
                return Err(changed(&file.rel));
            }
            if shared(kept) {
                renamed.insert(index, renamed_hash(node, &parsed));
            }
            ordinal += 1;
            Ok(())
        })?;
        if ordinal != own.len() {
            return Err(changed(&file.rel));
        }
    }
    Ok(renamed)
}

fn changed(rel: &str) -> AnalyzeError {
    AnalyzeError::Failed {
        analysis: "clones",
        cause: format!("{rel} changed while it was analyzed"),
    }
}

fn checkpoint(cancel: &CancellationToken) -> Result<(), AnalyzeError> {
    if cancel.is_cancelled() {
        Err(AnalyzeError::Cancelled)
    } else {
        Ok(())
    }
}

fn range_key(r: &CompactRange) -> (u32, u32, u32, u32) {
    (r.start_line(), r.start_col(), r.end_line(), r.end_col())
}

/// Whether `outer`'s range contains `inner`'s, in one file.
fn contains(outer: &Candidate, inner: &Candidate) -> bool {
    let (o, i) = (range_key(&outer.range), range_key(&inner.range));
    outer.file == inner.file && (o.0, o.1) <= (i.0, i.1) && (i.2, i.3) <= (o.2, o.3)
}

fn hasher() -> SipHasher13 {
    SipHasher13::new_with_keys(KEYS.0, KEYS.1)
}

fn put(h: &mut SipHasher13, bytes: &[u8]) {
    h.write_u64(bytes.len() as u64);
    h.write(bytes);
}

/// Starts the form of a node: its tag, whether it is named, and its kind.
fn start(tag: u8, node: Node) -> SipHasher13 {
    let mut h = hasher();
    h.write_u8(tag);
    h.write_u8(node.is_named() as u8);
    put(&mut h, node.kind().as_bytes());
    h
}

/// The form of a replaced literal.
fn literal() -> u128 {
    let mut h = hasher();
    h.write_u8(b'$');
    h.finish128().as_u128()
}

/// A node's forms while its children are added.
struct Open<'t> {
    node: Node<'t>,
    exact: SipHasher13,
    blind: SipHasher13,
    tokens: u32,
    nodes: u32,
    identifier: bool,
}

/// The summary of a finished node.
struct Summary {
    exact: u128,
    blind: u128,
    tokens: u32,
    nodes: u32,
    identifier: bool,
}

impl Open<'_> {
    fn add(&mut self, child: &Summary) {
        self.exact.write_u128(child.exact);
        self.blind.write_u128(child.blind);
        self.tokens += child.tokens;
        self.nodes += child.nodes;
        self.identifier |= child.identifier;
    }
}

/// Hashes every node of the file bottom-up, comments left out, and calls
/// `found` for each candidate in post-order: a named node other than the
/// root, with no ERROR or MISSING node, of at least [`MIN_TOKENS`] tokens.
fn walk<'t>(
    parsed: &'t Parsed,
    cancel: &CancellationToken,
    mut found: impl FnMut(Node<'t>, Candidate) -> Result<(), AnalyzeError>,
) -> Result<(), AnalyzeError> {
    let grammar = parsed.source.grammar();
    let text = parsed.source.content();
    let literals = literal_kinds(grammar);
    let lit = literal();
    let open = |node: Node<'t>| {
        let mut exact = start(b'(', node);
        let mut blind = start(b'(', node);
        if is_literal(node, literals) {
            hidden_text(node, literals, text, &mut exact);
            hidden_text(node, literals, text, &mut blind);
        }
        Open {
            node,
            exact,
            blind,
            tokens: 0,
            nodes: 1,
            identifier: false,
        }
    };
    let root = parsed.source.tree().root_node();
    let mut cursor = root.walk();
    let mut stack = vec![open(root)];
    let mut count = 0;
    loop {
        let mut moved = cursor.goto_first_child();
        loop {
            if moved {
                let node = cursor.node();
                if is_comment(node.kind()) {
                    moved = cursor.goto_next_sibling();
                    continue;
                }
                if node.child_count() != 0 {
                    stack.push(open(node));
                    break;
                }
                let mut exact = start(b'L', node);
                put(&mut exact, text[node.byte_range()].as_bytes());
                let exact = exact.finish128().as_u128();
                let identifier = is_identifier_token(grammar, &node, text);
                let blind = if identifier {
                    start(b'I', node).finish128().as_u128()
                } else if is_literal(node, literals) {
                    lit
                } else {
                    exact
                };
                let leaf = Summary {
                    exact,
                    blind,
                    tokens: 1,
                    nodes: 1,
                    identifier,
                };
                stack.last_mut().expect("an open parent").add(&leaf);
                moved = cursor.goto_next_sibling();
                continue;
            }
            let done = stack.pop().expect("an open node");
            let node = done.node;
            let mut summary = Summary {
                exact: done.exact.finish128().as_u128(),
                blind: done.blind.finish128().as_u128(),
                tokens: done.tokens,
                nodes: done.nodes,
                identifier: done.identifier,
            };
            if is_literal(node, literals) && !summary.identifier {
                summary.blind = lit;
                summary.tokens = 1;
            }
            let Some(parent) = stack.last_mut() else {
                return Ok(());
            };
            parent.add(&summary);
            if node.is_named() && summary.tokens >= MIN_TOKENS && !node.has_error() {
                count += 1;
                if count % CHECK_EVERY == 0 {
                    checkpoint(cancel)?;
                }
                let candidate = Candidate {
                    file: 0,
                    grammar,
                    range: range(node),
                    tokens: summary.tokens,
                    nodes: summary.nodes,
                    exact: summary.exact,
                    blind: summary.blind,
                };
                found(node, candidate)?;
            }
            cursor.goto_parent();
            moved = cursor.goto_next_sibling();
        }
    }
}

fn range(node: Node) -> CompactRange {
    let (start, end) = (node.start_position(), node.end_position());
    CompactRange::new(
        start.row as u32 + 1,
        start.column as u32 + 1,
        end.row as u32 + 1,
        end.column as u32 + 1,
    )
}

/// The hash of `node`'s renamed form, in one pass over its subtree: each
/// identifier token numbered by the first appearance of its text, each
/// literal that holds no identifier token replaced.
fn renamed_hash(node: Node, parsed: &Parsed) -> u128 {
    let grammar = parsed.source.grammar();
    let text = parsed.source.content();
    let literals = literal_kinds(grammar);
    let mut names: HashMap<&str, u32> = HashMap::new();
    let mut h = hasher();
    let mut cursor = node.walk();
    loop {
        let n = cursor.node();
        let mut descend = false;
        if is_comment(n.kind()) {
        } else if n.child_count() == 0 {
            if is_identifier_token(grammar, &n, text) {
                let next = names.len() as u32 + 1;
                let k = *names.entry(&text[n.byte_range()]).or_insert(next);
                h.write_u8(b'I');
                h.write_u8(n.is_named() as u8);
                put(&mut h, n.kind().as_bytes());
                h.write_u32(k);
            } else if is_literal(n, literals) {
                h.write_u8(b'$');
            } else {
                h.write_u8(b'L');
                h.write_u8(n.is_named() as u8);
                put(&mut h, n.kind().as_bytes());
                put(&mut h, text[n.byte_range()].as_bytes());
            }
        } else if is_literal(n, literals) && !holds_identifier(n, grammar, text) {
            h.write_u8(b'$');
        } else {
            h.write_u8(b'(');
            h.write_u8(n.is_named() as u8);
            put(&mut h, n.kind().as_bytes());
            if is_literal(n, literals) {
                hidden_text(n, literals, text, &mut h);
            }
            descend = cursor.goto_first_child();
        }
        if descend {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return h.finish128().as_u128();
            }
            h.write_u8(b')');
        }
    }
}

/// Writes the text of literal `node` that none of its children covers, with
/// the number of non-comment children before it: string content that some
/// grammars keep out of the tree (Kotlin, Dart, Scala). Comments and the
/// whitespace next to them are left out, and so is whitespace between two
/// quoted parts of one literal (`"a" "b"`): both are formatting.
fn hidden_text(node: Node, literals: &[&str], text: &str, h: &mut SipHasher13) {
    let quote = |c: &Node| !c.is_named() && (c.kind().ends_with('\'') || c.kind().ends_with('"'));
    let mut cursor = node.walk();
    let (comments, children): (Vec<Node>, Vec<Node>) = node
        .children(&mut cursor)
        .partition(|c| is_comment(c.kind()));
    let mut end = node.start_byte();
    let mut quotes = 0;
    for i in 0..=children.len() {
        let next = children.get(i);
        let start = next.map_or(node.end_byte(), |c| c.start_byte());
        let mut pieces = Vec::new();
        let mut at = end;
        for c in comments
            .iter()
            .filter(|c| c.start_byte() >= end && c.end_byte() <= start)
        {
            pieces.push(&text[at..c.start_byte()]);
            at = c.end_byte();
        }
        pieces.push(&text[at..start]);
        let last = pieces.len() - 1;
        let gap: String = pieces
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let p = if k > 0 { p.trim_start() } else { p };
                if k < last { p.trim_end() } else { p }
            })
            .collect();
        let part = |c: Option<&Node>| c.is_some_and(|c| is_literal(*c, literals));
        let outside = quotes % 2 == 0
            && (quotes > 0 || part(i.checked_sub(1).and_then(|p| children.get(p))) || part(next));
        if !gap.is_empty() && !(outside && gap.trim().is_empty()) {
            h.write_u8(b'"');
            h.write_u32(i as u32);
            put(h, gap.as_bytes());
        }
        if let Some(c) = next {
            quotes += quote(c) as u32;
            end = c.end_byte();
        }
    }
}

/// Whether `node` is a literal: a named node of a Table L kind. A keyword
/// token of the same kind (TypeScript `string`) is not.
fn is_literal(node: Node, literals: &[&str]) -> bool {
    node.is_named() && literals.contains(&node.kind())
}

/// Whether `node`'s subtree holds an identifier token outside comments.
fn holds_identifier(node: Node, grammar: Grammar, text: &str) -> bool {
    let mut cursor = node.walk();
    loop {
        let n = cursor.node();
        if !is_comment(n.kind()) {
            if is_identifier_token(grammar, &n, text) {
                return true;
            }
            if cursor.goto_first_child() {
                continue;
            }
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return false;
            }
        }
    }
}

/// The literal kinds of each language (string, character, numeric and symbol
/// literals). Booleans and null are not literals here.
fn literal_kinds(grammar: Grammar) -> &'static [&'static str] {
    match grammar {
        Grammar::Bash => &[
            "string",
            "raw_string",
            "ansi_c_string",
            "translated_string",
            "number",
        ],
        Grammar::C => &[
            "string_literal",
            "concatenated_string",
            "char_literal",
            "number_literal",
        ],
        Grammar::Cpp => &[
            "string_literal",
            "raw_string_literal",
            "concatenated_string",
            "char_literal",
            "number_literal",
            "user_defined_literal",
        ],
        Grammar::CSharp => &[
            "string_literal",
            "verbatim_string_literal",
            "raw_string_literal",
            "interpolated_string_expression",
            "character_literal",
            "integer_literal",
            "real_literal",
        ],
        Grammar::Dart => &[
            "string_literal",
            "symbol_literal",
            "decimal_integer_literal",
            "hex_integer_literal",
            "decimal_floating_point_literal",
        ],
        Grammar::Elixir => &[
            "string",
            "charlist",
            "sigil",
            "char",
            "integer",
            "float",
            "atom",
            "quoted_atom",
        ],
        Grammar::Go => &[
            "interpreted_string_literal",
            "raw_string_literal",
            "rune_literal",
            "int_literal",
            "float_literal",
            "imaginary_literal",
        ],
        Grammar::Java => &[
            "string_literal",
            "character_literal",
            "decimal_integer_literal",
            "hex_integer_literal",
            "octal_integer_literal",
            "binary_integer_literal",
            "decimal_floating_point_literal",
            "hex_floating_point_literal",
        ],
        Grammar::JavaScript | Grammar::Jsx | Grammar::TypeScript | Grammar::Tsx => {
            &["string", "template_string", "number", "regex"]
        }
        Grammar::Kotlin => &[
            "string_literal",
            "character_literal",
            "integer_literal",
            "long_literal",
            "hex_literal",
            "bin_literal",
            "unsigned_literal",
            "real_literal",
        ],
        Grammar::Lua => &["string", "number"],
        Grammar::Php => &[
            "string",
            "encapsed_string",
            "heredoc",
            "nowdoc",
            "integer",
            "float",
        ],
        Grammar::Python => &["string", "concatenated_string", "integer", "float"],
        Grammar::Ruby => &[
            "string",
            "chained_string",
            "heredoc_body",
            "character",
            "simple_symbol",
            "delimited_symbol",
            "hash_key_symbol",
            "regex",
            "integer",
            "float",
            "rational",
            "complex",
        ],
        Grammar::Rust => &[
            "string_literal",
            "raw_string_literal",
            "char_literal",
            "integer_literal",
            "float_literal",
        ],
        Grammar::Scala => &[
            "string",
            "interpolated_string",
            "character_literal",
            "integer_literal",
            "floating_point_literal",
        ],
        Grammar::Swift => &[
            "line_string_literal",
            "multi_line_string_literal",
            "raw_string_literal",
            "regex_literal",
            "integer_literal",
            "hex_literal",
            "oct_literal",
            "bin_literal",
            "real_literal",
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::discovery::{Includes, discover};
    use crate::analyze::read::{Parsed, Reader};
    use act_lite_parser::SourceFile;
    use std::fs;
    use std::path::Path;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn request(root: &Path) -> Request {
        Request::new(root, &[], &[], &[], Includes::default(), 100).unwrap()
    }

    /// A [`FileSource`] over the scope of `request`.
    fn reader(request: &Request, cancel: &CancellationToken) -> Reader {
        let supported = |g: Grammar| function_model::supports(g);
        let found = discover(&request.root, &request.scope, &supported, cancel).unwrap();
        Reader::new(&found.files, &supported, false, cancel)
    }

    #[test]
    fn every_literal_kind_exists_in_its_grammar() {
        let mut missing = Vec::new();
        let mut covered = 0;
        for grammar in Grammar::all() {
            if !function_model::supports(grammar) {
                continue;
            }
            covered += 1;
            let language = grammar.language_support().ts_language();
            assert!(!literal_kinds(grammar).is_empty(), "{grammar:?}");
            for kind in literal_kinds(grammar) {
                let id = language.id_for_node_kind(kind, true);
                if tree_sitter::Query::new(&language, &format!("({kind}) @probe")).is_err()
                    || id == 0
                    || !language.node_kind_is_named(id)
                {
                    missing.push(format!("{grammar:?}: {kind}"));
                }
            }
        }
        assert_eq!(covered, 20);
        assert!(missing.is_empty(), "{missing:#?}");
    }

    /// Records each file read again.
    struct Rereads {
        inner: Reader,
        rereads: Vec<String>,
    }

    impl FileSource for Rereads {
        fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
            self.inner.next_file()
        }

        fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
            self.rereads.push(rel.to_string());
            self.inner.reread(rel)
        }
    }

    const BASE: &str = "def summarize(orders, rate):
    total = 0
    count = 0
    for order in orders:
        if order.status == \"paid\" and order.amount > 0:
            total = total + order.amount * rate
            count = count + 1
        elif order.status == \"refunded\":
            total = total - order.amount
    if count == 0:
        return None
    return {\"total\": total, \"count\": count}
";

    #[test]
    fn only_files_with_a_shared_blind_hash_are_read_again() {
        let dir = tempfile::tempdir().unwrap();
        // The C2 workspace: the base, a reformatted copy with comments, and
        // a consistently renamed copy.
        write(dir.path(), "a.py", BASE);
        let reformatted = BASE
            .replace("    total = 0\n", "    # running sum\n    total = 0\n\n")
            .replace(
                "        elif",
                "        # refunds reduce the sum\n        elif",
            )
            .replace(
                "{\"total\": total, \"count\": count}",
                "{\"total\": total,\n            \"count\": count}",
            );
        write(dir.path(), "b.py", &reformatted);
        let renamed = BASE
            .replace("summarize", "tally")
            .replace("orders", "rows")
            .replace("rate", "factor")
            .replace("count = 0", "n = 1")
            .replace("total", "acc")
            .replace("count", "n")
            .replace("order", "row")
            .replace("paid", "settled")
            .replace("refunded", "returned");
        write(dir.path(), "c.py", &renamed);
        write(
            dir.path(),
            "d.py",
            "class Unique:\n    def run(self, items):\n        return [item.value for item in items if item.ready] + [len(items), self.limit, 42, \"x\", (1, 2), {\"k\": items}]\n",
        );
        let cancel = CancellationToken::new();
        // The file without a duplicate holds candidates of its own.
        let alone = Request::new(
            dir.path(),
            &["d.py".to_string()],
            &[],
            &[],
            Includes::default(),
            100,
        )
        .unwrap();
        let analysis = ClonesAnalyzer::new(&alone, &[])
            .unwrap()
            .analyze(&mut reader(&alone, &cancel), &cancel)
            .unwrap();
        assert!(analysis.details.unwrap()["candidates"].as_u64().unwrap() >= 1);

        let request = request(dir.path());
        let mut source = Rereads {
            inner: reader(&request, &cancel),
            rereads: Vec::new(),
        };
        let analyzer = ClonesAnalyzer::new(&request, &[]).unwrap();
        let analysis = analyzer.analyze(&mut source, &cancel).unwrap();
        assert_eq!(analysis.findings.len(), 1);
        assert_eq!(source.rereads, ["a.py", "b.py", "c.py"]);
    }

    /// Gives its files, then cancels the token when it has no more.
    struct CancelAtEnd {
        files: Vec<Parsed>,
        cancel: CancellationToken,
    }

    impl FileSource for CancelAtEnd {
        fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
            let file = self.files.pop();
            if file.is_none() {
                self.cancel.cancel();
            }
            Ok(file)
        }

        fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
            panic!("unexpected reread of {rel}")
        }
    }

    #[test]
    fn cancel_after_the_last_read_stops_the_analysis() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.py", BASE);
        let cancel = CancellationToken::new();
        let source =
            SourceFile::from_content(dir.path().join("a.py"), BASE.to_string(), Grammar::Python)
                .unwrap();
        let mut files = CancelAtEnd {
            files: vec![Parsed {
                rel: "a.py".to_string(),
                source,
                removed: Vec::new(),
            }],
            cancel: cancel.clone(),
        };
        let analyzer = ClonesAnalyzer::new(&request(dir.path()), &[]).unwrap();
        let result = analyzer.analyze(&mut files, &cancel);
        assert!(matches!(result, Err(AnalyzeError::Cancelled)));
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

    /// Records the resident set after each file it gives.
    #[cfg(unix)]
    struct Recording {
        inner: Reader,
        samples: Vec<u64>,
    }

    #[cfg(unix)]
    impl FileSource for Recording {
        fn next_file(&mut self) -> Result<Option<Parsed>, AnalyzeError> {
            let file = self.inner.next_file();
            self.samples.push(rss_kib());
            file
        }

        fn reread(&mut self, rel: &str) -> Result<Parsed, AnalyzeError> {
            self.inner.reread(rel)
        }
    }

    #[cfg(unix)]
    #[test]
    fn no_tree_is_kept_past_its_file() {
        // Other tests of the process allocate at the same time, so the
        // measurement runs in a child process that runs only this test.
        const NAME: &str = "analyze::clones::tests::no_tree_is_kept_past_its_file";
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
            let text = "x".repeat(3 << 20);
            write(
                dir.path(),
                &format!("src/f{n}.rs"),
                &format!("fn f{n}() -> &'static str {{\n    \"{text}\"\n}}\n"),
            );
        }
        let cancel = CancellationToken::new();
        let analyzer = ClonesAnalyzer::new(&request(dir.path()), &[]).unwrap();
        // The allocator keeps the pages of the first parse; analyze one file
        // first so that only growth by retained files counts.
        let first = Request::new(
            dir.path(),
            &["src/f0.rs".to_string()],
            &[],
            &[],
            Includes::default(),
            100,
        )
        .unwrap();
        let mut warm = reader(&first, &cancel);
        analyzer.analyze(&mut warm, &cancel).unwrap();
        drop(warm);
        let mut recording = Recording {
            inner: reader(&request(dir.path()), &cancel),
            samples: Vec::new(),
        };
        analyzer.analyze(&mut recording, &cancel).unwrap();
        assert_eq!(recording.samples.len(), 4);
        // Growth over the samples, from the one taken at the first file.
        let first = recording.samples[0];
        let peak = recording.samples.iter().copied().max().unwrap();
        assert!(
            peak < first + 2048,
            "resident set grew from {first} KiB to {peak} KiB: {:?}",
            recording.samples
        );
    }
}
