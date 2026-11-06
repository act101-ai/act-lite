//! CLI argument parsing with clap.

use crate::analyze::discovery::Includes;
use crate::analyze::envelope::AnalyzeError;
use crate::analyze::run::Request;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

/// act-lite — code queries, code-health analysis and sequestered secrets for AI agents.
#[derive(Parser, Debug)]
#[command(name = "act-lite")]
#[command(version)]
#[command(about = "Code queries, code-health analysis and sequestered secrets for AI agents")]
#[command(long_about = None)]
pub struct Cli {
    /// Output format
    #[arg(long, global = true, default_value = "json")]
    pub format: OutputFormat,

    /// Set workspace root explicitly
    #[arg(long, global = true)]
    pub workspace: Option<PathBuf>,

    /// Disable LSP servers (LSP is enabled by default)
    #[arg(
        long,
        global = true,
        env = "ACT_LITE_DISABLE_LSP",
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub disable_lsp: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Output format for CLI responses.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Machine-readable JSON output (default)
    #[default]
    Json,
    /// Human-readable, indented output
    Text,
}

/// Top-level commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Query code structure and symbol relationships
    Query(QueryArgs),
    /// Model Context Protocol server
    Mcp(McpArgs),
    /// Sequestered secrets: import/add/rm/run/get/list/mv (values never enter logs)
    Env(EnvArgs),
    /// Analyze a repository: findings with their evidence, as one JSON object
    Analyze(AnalyzeArgs),
}

/// Arguments for the analyze command.
#[derive(clap::Args, Debug)]
pub struct AnalyzeArgs {
    #[command(subcommand)]
    pub command: AnalyzeCommand,
}

/// One subcommand per analyzer.
#[derive(Subcommand, Debug)]
pub enum AnalyzeCommand {
    /// Functions whose size, nesting, branching or parameter count exceeds a fixed threshold
    Patterns(PatternsArgs),
    /// Files ranked by the sum of complexity × lines over their functions
    Hotspots(HotspotsArgs),
    /// Exact and renamed duplicate syntax subtrees
    Clones(ClonesArgs),
    /// File dependency edges, or the edges a traversal from focus files reaches
    Graph(GraphArgs),
    /// File-level dependency cycles, each with one path around it
    Cycles(CyclesArgs),
    /// Incoming and outgoing file dependency counts per file
    Coupling(CouplingArgs),
    /// The files that depend on the focus files, directly or transitively
    Impact(ImpactArgs),
    /// The test analyzer: reports each Rust function of 5 or more lines
    #[cfg(debug_assertions)]
    #[command(hide = true)]
    Fixture(SharedArgs),
}

/// Arguments for `analyze patterns`.
#[derive(clap::Args, Debug)]
pub struct PatternsArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
}

/// Arguments for `analyze hotspots`.
#[derive(clap::Args, Debug)]
pub struct HotspotsArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
}

/// Arguments for `analyze clones`.
#[derive(clap::Args, Debug)]
pub struct ClonesArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
    /// Report only the clone classes with a member at or below this path (repeatable)
    #[arg(long, value_name = "PATH")]
    pub focus: Vec<String>,
}

/// Arguments for `analyze graph`.
#[derive(clap::Args, Debug)]
pub struct GraphArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
    /// Report the edges a traversal from the files at or below this path reaches (repeatable)
    #[arg(long, value_name = "PATH")]
    pub focus: Vec<String>,
    /// With --focus: follow what the focus files depend on (out), what depends on them (in), or both [default: both]
    #[arg(long, value_parser = ["in", "out", "both"])]
    pub direction: Option<String>,
    /// With --focus: the most edges from a focus file to follow, 1 to 1000 [default: 1]
    #[arg(long, value_name = "N")]
    pub max_depth: Option<u32>,
}

/// Arguments for `analyze cycles`.
#[derive(clap::Args, Debug)]
pub struct CyclesArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
}

/// Arguments for `analyze coupling`.
#[derive(clap::Args, Debug)]
pub struct CouplingArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
}

/// Arguments for `analyze impact`.
#[derive(clap::Args, Debug)]
pub struct ImpactArgs {
    #[command(flatten)]
    pub shared: SharedArgs,
    /// Report the dependents of the files at or below this path (repeatable, at least one)
    #[arg(long, value_name = "PATH")]
    pub focus: Vec<String>,
    /// The most edges from a focus file to follow, 1 to 1000 [default: 1000]
    #[arg(long, value_name = "N")]
    pub max_depth: Option<u32>,
}

/// The arguments every analyzer takes.
#[derive(clap::Args, Debug)]
pub struct SharedArgs {
    /// Files or directories to analyze, relative to the workspace root (default: the root)
    #[arg(value_name = "PATH")]
    pub paths: Vec<String>,
    /// Analyze only files that match at least one glob (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,
    /// Leave out files that match any glob (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,
    /// Analyze test files and embedded test code
    #[arg(long)]
    pub include_tests: bool,
    /// Analyze generated files and directories
    #[arg(long)]
    pub include_generated: bool,
    /// Analyze vendored directories
    #[arg(long)]
    pub include_vendored: bool,
    /// Analyze ignored paths
    #[arg(long)]
    pub include_ignored: bool,
    /// The most findings to return, 0 to 1000
    #[arg(long, default_value_t = 100)]
    pub limit: u32,
}

impl SharedArgs {
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

#[derive(clap::Args, Debug)]
pub struct EnvArgs {
    #[command(subcommand)]
    pub subcommand: EnvSubcommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum EnvSubcommand {
    /// Sequester every plaintext line of a .env into the store, marking each line in place
    Import {
        /// Path to the .env file to sequester
        path: PathBuf,
        /// Overwrite a stored value that differs from the file's
        #[arg(long)]
        force: bool,
    },
    /// Add or update one secret in the store (value via hidden prompt or --stdin, never argv; the repo .env is not touched)
    Add {
        /// The secret's name, e.g. GH_TOKEN
        name: String,
        /// Read the value from stdin instead of prompting (pipes/scripts)
        #[arg(long)]
        stdin: bool,
    },
    /// Remove one secret from the store (the repo .env is not touched)
    Rm {
        /// The secret's name
        name: String,
    },
    /// Run a command with secrets injected into its environment; output redacted as it streams
    Run {
        /// The command and its arguments, after `--`
        #[arg(trailing_var_arg = true, required = true)]
        argv: Vec<String>,
    },
    /// Print key names only (never values)
    List,
    /// Print ONE secret; redacted unless --reveal is given
    Get {
        /// The secret's name
        name: String,
        /// Actually print the plaintext value into your terminal/context
        #[arg(long)]
        reveal: bool,
    },
    /// Sequester ONE existing .env key into the store (or, with --restore, write it back)
    Mv {
        /// The secret's name, e.g. CLOUDFLARE_API_KEY
        name: String,
        /// Reverse: write the stored value back into .env and drop it from the store
        #[arg(long)]
        restore: bool,
        /// Proceed even if it would overwrite a different value at the destination
        #[arg(long)]
        force: bool,
    },
}

/// Arguments for the mcp command.
#[derive(Parser, Debug)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

/// MCP subcommands.
#[derive(Subcommand, Debug)]
pub enum McpCommand {
    /// Serve the navigation tools over stdio
    Serve,
}

/// Arguments for the query command.
#[derive(Parser, Debug)]
pub struct QueryArgs {
    /// Query type to execute
    #[command(subcommand)]
    pub query_type: QueryType,
}

/// Query types (10 total).
#[derive(Subcommand, Debug, Clone)]
pub enum QueryType {
    /// Get file structure without bodies
    Skeleton {
        /// File to query
        file: PathBuf,
    },
    /// Get single symbol details
    Symbol {
        /// Symbol name to find
        name: String,
        /// File to search in
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Get all symbols in a file
    Symbols {
        /// File to query
        file: PathBuf,
    },
    /// Batch symbol retrieval by files or stable IDs
    #[command(name = "symbols-batch")]
    SymbolsBatch {
        /// Comma-separated file paths
        #[arg(long)]
        files: Option<String>,
        /// Comma-separated stable symbol IDs
        #[arg(long)]
        ids: Option<String>,
        /// Filter by symbol kind(s)
        #[arg(long)]
        kinds: Option<String>,
    },
    /// Get repository file tree with languages and line counts
    #[command(name = "repo-outline")]
    RepoOutline {
        /// Maximum directory depth
        #[arg(long)]
        depth: Option<u32>,
        /// Glob pattern to include
        #[arg(long)]
        include: Option<String>,
        /// Glob pattern to exclude
        #[arg(long)]
        exclude: Option<String>,
        /// Include top-level symbol names per file
        #[arg(long)]
        symbols: bool,
        /// Maximum files to include
        #[arg(long, default_value_t = 500)]
        max_files: u32,
    },
    /// Get symbol definition location
    Definition {
        /// Symbol name
        name: String,
        /// File containing the reference
        #[arg(long)]
        file: PathBuf,
        /// Line number
        #[arg(long)]
        line: u32,
        /// Column number
        #[arg(long)]
        column: u32,
    },
    /// Find all references to a symbol (LSP when a language server is
    /// available; otherwise a cross-file name-level — not binding-level —
    /// tree-sitter baseline over same-grammar files in the workspace). The
    /// JSON output carries `source` naming the answering instrument: `lsp`
    /// when a live language server answered (empty answers included), or
    /// `tree-sitter` for the baseline.
    References {
        /// Symbol name
        name: String,
        /// File containing the symbol
        #[arg(long)]
        file: PathBuf,
        /// Line number
        #[arg(long)]
        line: u32,
        /// Column number
        #[arg(long)]
        column: u32,
    },
    /// Find functions that call a symbol (LSP)
    Callers {
        /// Symbol name
        name: String,
        /// File containing the symbol
        #[arg(long)]
        file: PathBuf,
    },
    /// Find functions called by a symbol (LSP)
    Callees {
        /// Symbol name
        name: String,
        /// File containing the symbol
        #[arg(long)]
        file: PathBuf,
    },
    /// Find implementations of interface/trait (LSP)
    Implementations {
        /// Interface/trait name
        name: String,
        /// File containing the definition
        #[arg(long)]
        file: PathBuf,
    },
}
