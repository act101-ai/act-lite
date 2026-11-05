//! The `act-lite analyze` command group.

use super::run::{Analyzer, run};
use crate::cli::{AnalyzeArgs, OutputFormat, SharedArgs};
use crate::output::output;
use crate::query::resolve_workspace_root;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Runs the analyzer the subcommand names.
#[cfg_attr(not(debug_assertions), allow(unused_variables))]
pub fn execute(
    args: &AnalyzeArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
) -> anyhow::Result<()> {
    match args.command {
        #[cfg(debug_assertions)]
        crate::cli::AnalyzeCommand::Fixture(ref shared) => {
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
    let root = resolve_workspace_root(workspace);
    let request = shared.request(&root)?;
    let envelope = run(analyzer, &request, &CancellationToken::new())?;
    output(&envelope, format)
}
