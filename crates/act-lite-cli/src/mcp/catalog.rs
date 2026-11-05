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
    #[cfg(debug_assertions)]
    ToolEntry {
        name: "analyze_fixture",
        description: "Test analyzer: report each Rust function of 5 or more lines, as one JSON result with its evidence. Served by builds with debug assertions only. Params: none [, paths (string[]), include (glob[]), exclude (glob[]), include_tests (bool), include_generated (bool), include_vendored (bool), include_ignored (bool), limit (u32, 0 to 1000, default 100)]",
    },
];
