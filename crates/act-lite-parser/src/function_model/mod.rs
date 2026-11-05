//! The function model: the functions of a source file and four metrics of each.
//!
//! A function is a function construct of its language that has a body: named
//! functions, methods, constructors, accessors and anonymous functions. A
//! function nested in another is a separate function, and a function's own
//! body is its subtree minus the subtrees of the functions nested in it. The
//! `Data` formats have no functions.
//!
//! Metrics, each over the own body:
//! - `lines`: the distinct lines on which a token starts, ends or spans. A
//!   token is a leaf that is not inside a comment; a string literal counts
//!   every line it spans; a Python docstring does not count.
//! - `nesting`: the deepest chain of nesting constructs (if, multi-way, loop,
//!   try). An if that is the else branch of another if adds no level.
//! - `complexity`: 1 + the decision points (each if and else-if, each arm that
//!   is not a default arm, each loop, each handler, each conditional
//!   expression, each logical operator).
//! - `parameters`: the declared parameters; `None` for Bash.

mod table;
mod tokens;

use act_lite_core::CompactRange;
use act_lite_core::registry::LanguageCategory;
use tree_sitter::Node;

use crate::{Grammar, SourceFile};
use table::{Ctx, Role};
pub use tokens::{is_comment, is_identifier_token};

/// The four metrics of one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub lines: u32,
    pub nesting: u32,
    pub complexity: u32,
    /// `None` where the metric does not apply (Bash).
    pub parameters: Option<u32>,
}

/// One function of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: Option<String>,
    /// The function construct's range; for a Dart top-level accessor, from the
    /// signature's start to the body's end.
    pub range: CompactRange,
    pub metrics: Metrics,
}

/// Whether the function model covers the grammar: every grammar that is not
/// a `Data` format.
pub fn supports(grammar: Grammar) -> bool {
    !matches!(grammar.category(), LanguageCategory::Data) && table::row(grammar).is_some()
}

/// The named node kinds the model reads for a grammar; empty for a grammar
/// that `supports` rejects.
pub fn node_kinds(grammar: Grammar) -> Vec<&'static str> {
    table::row(grammar)
        .map(|row| row.kinds())
        .unwrap_or_default()
}

/// Every function of the file, in location order.
pub fn functions(file: &SourceFile) -> Functions<'_> {
    let grammar = file.grammar();
    let source = file.content();
    let ctx = table::row(grammar)
        .filter(|_| supports(grammar))
        .map(|row| Ctx {
            grammar,
            row,
            source,
        });
    let stack = if ctx.is_some() {
        vec![file.tree().root_node()]
    } else {
        Vec::new()
    };
    Functions {
        ctx,
        stack,
        line_starts: std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
    }
}

/// The functions of a file. Each call to `next` finds and measures one function.
pub struct Functions<'a> {
    ctx: Option<Ctx<'a>>,
    stack: Vec<Node<'a>>,
    line_starts: Vec<usize>,
}

impl Iterator for Functions<'_> {
    type Item = Function;

    fn next(&mut self) -> Option<Function> {
        let ctx = self.ctx.as_ref()?;
        while let Some(node) = self.stack.pop() {
            let mut cursor = node.walk();
            let children: Vec<Node> = node.children(&mut cursor).collect();
            self.stack.extend(children.into_iter().rev());
            if ctx.is_function(node) {
                return Some(self.measure(ctx, node));
            }
        }
        None
    }
}

impl Functions<'_> {
    fn row_of(&self, byte: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= byte) - 1
    }

    fn measure(&self, ctx: &Ctx, node: Node) -> Function {
        let body = ctx.dart_accessor_body(node);
        let end = body.unwrap_or(node).end_position();
        let start = node.start_position();
        let first_row = start.row;
        let mut rows = vec![false; end.row - first_row + 1];
        let mut mark = |from: usize, to: usize| {
            for row in &mut rows[from - first_row..=to - first_row] {
                *row = true;
            }
        };
        let docstring = ctx.docstring(node);
        let mut nesting = 0;
        let mut complexity = 1;
        let mut stack: Vec<(Node, u32)> = vec![(node, 0)];
        stack.extend(body.map(|b| (b, 0)));
        while let Some((n, depth)) = stack.pop() {
            if (n != node && ctx.is_function(n)) || is_comment(n.kind()) || Some(n) == docstring {
                continue;
            }
            let mut depth = depth;
            match ctx.role(n) {
                Role::If => {
                    complexity += 1;
                    if !ctx.is_else_branch(n) {
                        depth += 1;
                    }
                }
                Role::Loop => {
                    complexity += 1;
                    depth += 1;
                }
                Role::MultiWay | Role::Try => depth += 1,
                Role::ElseIf | Role::Arm | Role::Handler | Role::Conditional => complexity += 1,
                Role::DefaultArm | Role::Other => {}
            }
            nesting = nesting.max(depth);
            complexity += ctx.logical_operators(n);

            if n.child_count() == 0 {
                mark(n.start_position().row, n.end_position().row);
            } else if ctx.is_string(n) {
                // Every line of a string counts, except the lines of a
                // function nested in it; its own tokens there mark their lines.
                let nested = nested_functions(ctx, n);
                for row in n.start_position().row..=n.end_position().row {
                    let inside =
                        |f: &Node| (f.start_position().row..=f.end_position().row).contains(&row);
                    if !nested.iter().any(inside) {
                        mark(row, row);
                    }
                }
            }
            if n.child_count() == 0 {
                continue;
            }
            // Text that no child covers (string contents in some grammars) is
            // part of a token too.
            let mut cursor = n.walk();
            let children: Vec<Node> = n.children(&mut cursor).collect();
            let mut covered = n.start_byte();
            for child in &children {
                if let Some((from, to)) = self.text_rows(ctx.source, covered, child.start_byte()) {
                    mark(from, to);
                }
                covered = covered.max(child.end_byte());
            }
            if let Some((from, to)) = self.text_rows(ctx.source, covered, n.end_byte()) {
                mark(from, to);
            }
            stack.extend(children.into_iter().rev().map(|c| (c, depth)));
        }
        Function {
            name: ctx.name(node),
            range: CompactRange::new(
                start.row as u32 + 1,
                start.column as u32 + 1,
                end.row as u32 + 1,
                end.column as u32 + 1,
            ),
            metrics: Metrics {
                lines: rows.iter().filter(|&&r| r).count() as u32,
                nesting,
                complexity,
                parameters: ctx.parameters(node),
            },
        }
    }

    /// The rows spanned by the non-whitespace text of `source[from..to]`.
    fn text_rows(&self, source: &str, from: usize, to: usize) -> Option<(usize, usize)> {
        if from >= to {
            return None;
        }
        let gap = &source[from..to];
        let first = gap.find(|c: char| !c.is_whitespace())?;
        let last = gap.rfind(|c: char| !c.is_whitespace())?;
        Some((self.row_of(from + first), self.row_of(from + last)))
    }
}

/// The outermost functions inside `node`.
fn nested_functions<'t>(ctx: &Ctx, node: Node<'t>) -> Vec<Node<'t>> {
    let mut found = Vec::new();
    let mut cursor = node.walk();
    let mut stack: Vec<Node> = node.children(&mut cursor).collect();
    while let Some(n) = stack.pop() {
        if ctx.is_function(n) {
            found.push(n);
        } else {
            stack.extend(n.children(&mut cursor));
        }
    }
    found
}
