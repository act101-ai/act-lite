//! The `act-lite analyze` command group.

use super::clones::ClonesAnalyzer;
use super::envelope::AnalyzeError;
use super::hotspots::HotspotsAnalyzer;
use super::patterns::PatternsAnalyzer;
use super::run::{Analyzer, Request, run};
use crate::cli::{AnalyzeArgs, AnalyzeCommand, ClonesArgs, OutputFormat, SharedArgs};
use crate::output::output;
use crate::query::resolve_workspace_root;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Runs the analyzer the subcommand names.
pub fn execute(
    args: &AnalyzeArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
) -> anyhow::Result<()> {
    match args.command {
        AnalyzeCommand::Patterns(ref a) => run_cli(&PatternsAnalyzer, &a.shared, format, workspace),
        AnalyzeCommand::Hotspots(ref a) => run_cli(&HotspotsAnalyzer, &a.shared, format, workspace),
        AnalyzeCommand::Clones(ref a) => run_clones(a, format, workspace),
        #[cfg(debug_assertions)]
        AnalyzeCommand::Fixture(ref shared) => {
            run_cli(&super::fixture::FixtureAnalyzer, shared, format, workspace)
        }
    }
}

/// Runs `analyzer` on the request `shared` describes and prints the result.
/// The root is `workspace`, else the current directory. Nothing cancels the
/// run: an interrupt ends the process.
pub fn run_cli<A: Analyzer>(
    analyzer: &A,
    shared: &SharedArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
) -> anyhow::Result<()> {
    let request = cli_request(shared, workspace)?;
    print(analyzer, &request, format)
}

/// Runs `analyze clones`: the shared parameters are checked first, then
/// `--focus`.
fn run_clones(
    args: &ClonesArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
) -> anyhow::Result<()> {
    let request = cli_request(&args.shared, workspace)?;
    let analyzer = ClonesAnalyzer::new(&request, &args.focus)?;
    print(&analyzer, &request, format)
}

/// The checked request; the root is `workspace`, else the current directory.
fn cli_request(shared: &SharedArgs, workspace: Option<&Path>) -> Result<Request, AnalyzeError> {
    shared.request(&resolve_workspace_root(workspace))
}

/// Runs the analysis and prints the result.
fn print<A: Analyzer>(analyzer: &A, request: &Request, format: OutputFormat) -> anyhow::Result<()> {
    let envelope = run(analyzer, request, &CancellationToken::new())?;
    output(&envelope, format)
}
