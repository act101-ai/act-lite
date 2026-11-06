//! The `act-lite analyze` command group.

use super::clones::ClonesAnalyzer;
use super::coupling::CouplingAnalyzer;
use super::cycles::CyclesAnalyzer;
use super::envelope::AnalyzeError;
use super::graph::GraphAnalyzer;
use super::hotspots::HotspotsAnalyzer;
use super::impact::ImpactAnalyzer;
use super::patterns::PatternsAnalyzer;
use super::run::{Analyzer, Request, run};
use crate::cli::{
    AnalyzeArgs, AnalyzeCommand, ClonesArgs, GraphArgs, ImpactArgs, OutputFormat, SharedArgs,
};
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
        AnalyzeCommand::Graph(ref a) => run_graph(a, format, workspace),
        AnalyzeCommand::Cycles(ref a) => run_cli(&CyclesAnalyzer, &a.shared, format, workspace),
        AnalyzeCommand::Coupling(ref a) => run_cli(&CouplingAnalyzer, &a.shared, format, workspace),
        AnalyzeCommand::Impact(ref a) => run_impact(a, format, workspace),
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

/// Runs `analyze graph`: the shared parameters are checked first, then
/// `--max-depth`, `--direction` and `--focus`.
fn run_graph(
    args: &GraphArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
) -> anyhow::Result<()> {
    let request = cli_request(&args.shared, workspace)?;
    let analyzer = GraphAnalyzer::new(
        &request,
        &args.focus,
        args.direction.as_deref(),
        args.max_depth,
    )?;
    print(&analyzer, &request, format)
}

/// Runs `analyze impact`: the shared parameters are checked first, then
/// `--max-depth` and `--focus`.
fn run_impact(
    args: &ImpactArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
) -> anyhow::Result<()> {
    let request = cli_request(&args.shared, workspace)?;
    let analyzer = ImpactAnalyzer::new(&request, &args.focus, args.max_depth)?;
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
