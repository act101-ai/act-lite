//! The `act-lite` binary.

#![cfg_attr(not(test), deny(clippy::print_stdout))]

use act_lite_cli::cli::{Cli, Command, McpCommand};
use act_lite_cli::output::output_error;
use act_lite_cli::stdout;
use anyhow::Context;
use clap::{CommandFactory, FromArgMatches};
use std::io::Write;

fn main() {
    let cli = parse_args();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build the tokio runtime");
    let result = rt.block_on(run(&cli));
    // Dropping a runtime waits for its blocking pool, and a blocking stdin
    // read cannot be cancelled: with stdin still open, a finished
    // `act-lite mcp serve` would hang here. Everything `run` owns has
    // already been torn down.
    rt.shutdown_background();
    if let Err(error) = result {
        // The command's own non-zero code wins over the broken-pipe code.
        output_error(&error, cli.format);
        std::process::exit(1);
    }
    // The command succeeded but its output was cut: stdout was closed by
    // the reader.
    if stdout::is_closed() {
        std::process::exit(stdout::BROKEN_PIPE_EXIT);
    }
}

async fn run(cli: &Cli) -> anyhow::Result<()> {
    match &cli.command {
        Command::Query(args) => {
            act_lite_cli::query::execute(
                args,
                cli.format,
                cli.workspace.as_deref(),
                !cli.disable_lsp,
            )
            .await
        }
        Command::Mcp(args) => match args.command {
            McpCommand::Serve => {
                act_lite_cli::mcp::serve(cli.workspace.as_deref(), !cli.disable_lsp).await
            }
        },
        Command::Env(args) => {
            act_lite_cli::env::execute(&args.subcommand, cli.workspace.as_deref())
                .context("Env command failed")
        }
    }
}

/// Parse the arguments without `Cli::parse()`: clap's `parse` prints help
/// and version itself and discards a stdout write error, so `--help | head`
/// would exit 0 into a closed pipe. Display kinds go through the
/// closed-pipe boundary instead; error kinds keep clap's own output.
fn parse_args() -> Cli {
    let cmd = Cli::command();
    let color = cmd.get_color();
    let matches = cmd
        .try_get_matches()
        .unwrap_or_else(|e| exit_for_clap(e, color));
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

/// Render clap text (help, version) the way clap prints it, as bytes: ANSI
/// escapes kept or stripped by the same rule clap applies to its own output
/// (`CLICOLOR`, `CLICOLOR_FORCE`, `NO_COLOR`, `TERM=dumb`, terminal check).
fn styled_bytes(color: clap::ColorChoice, text: &clap::builder::StyledStr) -> Vec<u8> {
    let choice = match color {
        clap::ColorChoice::Auto => anstream::AutoStream::choice(&std::io::stdout()),
        clap::ColorChoice::Always => anstream::ColorChoice::Always,
        clap::ColorChoice::Never => anstream::ColorChoice::Never,
    };
    let mut stream = anstream::AutoStream::new(Vec::new(), choice);
    // Writing to a Vec cannot fail.
    write!(stream, "{}", text.ansi()).expect("write to Vec");
    stream.into_inner()
}

/// Print a clap parse outcome and exit. Help and version go to stdout
/// through the closed-pipe boundary: a cut pipe exits
/// [`stdout::BROKEN_PIPE_EXIT`], any other write error is as loud as
/// `println!` was. Real errors keep clap's stderr output and exit 2.
fn exit_for_clap(e: clap::Error, color: clap::ColorChoice) -> ! {
    if e.use_stderr() {
        e.exit();
    }
    let bytes = styled_bytes(color, &e.render());
    stdout::try_with_stdout(|w| w.write_all(&bytes))
        .unwrap_or_else(|err| panic!("failed printing to stdout: {err}"));
    if stdout::is_closed() {
        std::process::exit(stdout::BROKEN_PIPE_EXIT);
    }
    std::process::exit(e.exit_code());
}
