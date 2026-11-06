//! The one source of each MCP tool's description. `tools/list` reads it, so
//! what the server advertises cannot drift from this table.

/// One MCP tool: its name and the description `tools/list` publishes.
pub struct ToolEntry {
    /// MCP tool name (snake_case, matches the `#[tool]` method name).
    pub name: &'static str,
    /// Description published in `tools/list`.
    pub description: &'static str,
}

/// Every tool act-lite serves.
pub const TOOLS: &[ToolEntry] = &[
    ToolEntry {
        name: "status",
        description: "Get act-lite status: the supported languages and, for each language server, whether it is ready or not installed. Params: none",
    },
    ToolEntry {
        name: "skeleton",
        description: "Get a file's structure — function signatures, class declarations, type definitions — without implementation bodies. Read it before the full file when you need only the API. Typically 5-10x fewer tokens than reading the raw file. Params: file (string)",
    },
    ToolEntry {
        name: "symbols",
        description: "List all symbols (functions, classes, variables, types, constants) defined in a file with their locations and kinds. Read it before the full file when you need to know what is defined in it; it costs much less. The file must be a path to a single source file, not a directory or an empty string. Params: file (string)",
    },
    ToolEntry {
        name: "symbols_batch",
        description: "Retrieve symbols from multiple files in a single call, instead of calling symbols() in a loop. Pass `pattern` (name regex) or `kinds` unless you genuinely need every symbol: three medium files unfiltered is ~10,000 tokens and the same query filtered is ~300. If you want one symbol's location, `symbols` on its file with a pattern is cheaper still. Params: none [, files (string[]), ids (string[]), kinds (string[]), pattern (string)] — provide files or ids",
    },
    ToolEntry {
        name: "repo_outline",
        description: "Get a compressed overview of the repository — file tree with languages and line counts. Scope it: unscoped on a medium repo this is ~10,000 tokens that stay in context for the rest of the session, where `depth`, `include` or `max_files` answer the same question for a few hundred. `symbols: true` adds every top-level name and multiplies the size again. Params: none [, path (string), depth (u32), include (glob), exclude (glob), symbols (bool), max_files (u32), relative (bool)]",
    },
    ToolEntry {
        name: "references",
        description: "Find every usage of a symbol across the entire codebase — all files, not just the current one. Use before changing or deleting a symbol to understand full impact. Uses the language server when one is available; without one it falls back to a name-level tree-sitter search over same-language files. The answer says which in `resolved_by` (e.g. `lsp:rust-analyzer`). Params: symbol (string), file (string)",
    },
    ToolEntry {
        name: "callers",
        description: "Find every call site of a function or method across the codebase. Use to understand who depends on a function before changing its signature. Requires a language server; the server starts on the first call, and the answer names it in `resolved_by` (e.g. `lsp:rust-analyzer`). Params: symbol (string), file (string)",
    },
    ToolEntry {
        name: "definition",
        description: "Jump to definition — find where a symbol is defined given a usage location. Uses AST-based parsing, no language server required. Params: symbol (string), file (string), line (u32), column (u32)",
    },
    ToolEntry {
        name: "analyze_patterns",
        description: "Find functions with structural problems: more than 50 code lines, nesting deeper than 4, cyclomatic complexity above 10, or more than 5 parameters. Each finding gives the function, its location, the metric, its value and the threshold it exceeds, as one JSON result with its evidence. Covers 23 programming languages; data and markup formats report unsupported. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100)]",
    },
    ToolEntry {
        name: "analyze_hotspots",
        description: "Rank files by where investigation should start: score = sum over a file's functions of complexity × lines. Each file comes with its top 5 functions, their locations and raw metrics; the formula is in `details`. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100)]",
    },
    ToolEntry {
        name: "analyze_clones",
        description: "Find duplicated code: exact and renamed duplicate syntax subtrees of at least 50 tokens, with member ranges, normalized size and variants. Exact matching ignores formatting and comments; renamed matching abstracts identifiers and literals consistently. A clone class shows the same syntax tree, not that the members behave the same. Use `focus` to report only the classes that touch given files. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100), focus (string[])]",
    },
    ToolEntry {
        name: "graph",
        description: "Get the file dependency edges of a scope: each edge is a source file, the workspace file it imports, the relationship kinds (import, require, reexport, module, include and others) and the import ranges as evidence. Without `focus`, every edge of the scope; with `focus`, the edges a traversal from the focus files reaches, `direction` out (what they depend on), in (what depends on them) or both, up to `max_depth` edges away. Imports that resolve outside the workspace are counted as external, the ones that cannot be resolved are listed as unresolved. Covers JavaScript, JSX, TypeScript, TSX, Python, Go and Rust. The README lists the relationships act-lite does not model. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100), focus (string[]), direction (in, out or both, default both; needs focus), max_depth (u32, 1 to 1000, default 1; needs focus)]",
    },
    ToolEntry {
        name: "analyze_cycles",
        description: "Find file-level dependency cycles: each finding is a set of files in which every file reaches every other one through imports, with its members and one shortest path around the cycle as import steps. A Rust `mod` declaration alone does not make a cycle. An unresolved import makes the result incomplete, because it could close a cycle. Covers JavaScript, JSX, TypeScript, TSX, Python, Go and Rust. The README lists the relationships act-lite does not model. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100)]",
    },
    ToolEntry {
        name: "analyze_coupling",
        description: "Count each file's dependencies: fan_in (distinct analyzed files that import it), fan_out (distinct workspace files it imports), degree = fan_in + fan_out and instability = fan_out / degree, with its external and unresolved import counts; the formulas are in `details`. fan_in counts only the analyzed files, so a scope narrower than the workspace undercounts it. Covers JavaScript, JSX, TypeScript, TSX, Python, Go and Rust. The README lists the relationships act-lite does not model. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100)]",
    },
    ToolEntry {
        name: "analyze_impact",
        description: "List the files that depend on the focus files, directly or through other files, each with its depth and the chain of imports that reaches a focus file. A dependent is a file that imports a focus file, directly or through other files. It is exposed to a change in the focus file. This is not proof that it will break, and a file that is not listed is not proof that it is safe: relationships that act-lite does not model (see the README) are not followed. Only files in `paths` are read as importers. Covers JavaScript, JSX, TypeScript, TSX, Python, Go and Rust. Params: focus (string[], at least one) [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100), max_depth (u32, 1 to 1000, default 1000)]",
    },
    #[cfg(debug_assertions)]
    ToolEntry {
        name: "analyze_fixture",
        description: "Test analyzer: report each Rust function of 5 or more lines, as one JSON result with its evidence. Served by builds with debug assertions only. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100)]",
    },
];
