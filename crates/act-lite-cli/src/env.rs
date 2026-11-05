//! `act-lite env` subcommand dispatch — sequestered secrets.

use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cli::EnvSubcommand;

pub fn execute(sub: &EnvSubcommand, workspace: Option<&Path>) -> Result<()> {
    let mut secrets = match workspace {
        Some(ws) => {
            let root = ws.canonicalize().with_context(|| {
                format!(
                    "--workspace {} does not exist or is not readable; pass an existing project directory",
                    ws.display()
                )
            })?;
            act_lite_secrets::Secrets::open_root(&root)
        }
        None => {
            let cwd = std::env::current_dir().context("failed to read the current directory")?;
            act_lite_secrets::Secrets::open(&cwd)
        }
    }
    .context("failed to open the secrets store")?;

    match sub {
        EnvSubcommand::Import { path, force } => {
            let out = secrets
                .import(path, *force)
                .with_context(|| format!("failed to import {}", path.display()))?;
            for key in &out.missing {
                eprintln!(
                    "warning: {key} is marked #act:sequestered in {} but the store has no value for it; use `act-lite env add {key}` to enter one",
                    path.display()
                );
            }
            for key in &out.sequestered_keys {
                if let Some(value) = secrets.get(key) {
                    warn_if_unredactable(key, &value);
                }
            }
            eprintln!(
                "Sequestered {} secret(s) from {}; {} key(s) were already sequestered.",
                out.sequestered,
                path.display(),
                out.already_sequestered
            );
            Ok(())
        }
        EnvSubcommand::Add { name, stdin } => {
            let value = read_secret_value(*stdin, name)?;
            secrets
                .add(name, &value)
                .with_context(|| format!("failed to store {name}"))?;
            warn_if_unredactable(name, &value);
            eprintln!("Stored {name}.");
            Ok(())
        }
        EnvSubcommand::Rm { name } => {
            secrets
                .remove(name)
                .with_context(|| format!("failed to remove {name}"))?;
            eprintln!("Removed {name}.");
            Ok(())
        }
        EnvSubcommand::List => {
            for key in secrets.keys() {
                crate::stdout::try_with_stdout(|w| writeln!(w, "{key}"))?;
            }
            Ok(())
        }
        EnvSubcommand::Get { name, reveal } => {
            let value = secrets
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("secret '{name}' is not in this project's store"))?;
            if *reveal {
                crate::stdout::try_with_stdout(|w| writeln!(w, "{value}"))?;
            } else {
                crate::stdout::try_with_stdout(|w| {
                    writeln!(w, "***  (redacted; pass --reveal to print the value)")
                })?;
            }
            Ok(())
        }
        EnvSubcommand::Mv {
            name,
            restore,
            force,
        } => {
            let outcome = secrets
                .mv(name, *restore, *force)
                .with_context(|| format!("failed to mv {name}"))?;
            let msg = match outcome {
                act_lite_secrets::MvOutcome::Sequestered => {
                    if let Some(value) = secrets.get(name) {
                        warn_if_unredactable(name, &value);
                    }
                    format!("Sequestered {name}; .env now points to the store.")
                }
                act_lite_secrets::MvOutcome::AlreadySequestered => {
                    format!("{name} is already sequestered.")
                }
                act_lite_secrets::MvOutcome::Restored => {
                    format!("Restored {name} into .env; removed it from the store.")
                }
            };
            eprintln!("{msg}");
            Ok(())
        }
        EnvSubcommand::Run { argv } => {
            // `run_command` streams the child's (redacted) output live; we only
            // need to propagate its exit code.
            let outcome = act_lite_secrets::run::run_command(&secrets, argv)
                .context("failed to run command")?;
            if outcome.stdout_closed {
                crate::stdout::mark_closed();
            }
            if outcome.code != 0 {
                std::process::exit(outcome.code);
            }
            Ok(())
        }
    }
}

/// Warn on stderr when `value` is too short for `act-lite env run` to redact.
fn warn_if_unredactable(name: &str, value: &str) {
    if value.len() < act_lite_secrets::redact::MIN_REDACT_LEN {
        eprintln!(
            "warning: {name} is shorter than {} characters and will not be redacted from `act-lite env run` output",
            act_lite_secrets::redact::MIN_REDACT_LEN
        );
    }
}

/// Read a secret value without it ever appearing in argv: from stdin when
/// `--stdin`, otherwise via a hidden interactive prompt.
fn read_secret_value(use_stdin: bool, name: &str) -> Result<String> {
    if use_stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read the value from stdin")?;
        return Ok(buf.trim_end_matches(['\n', '\r']).to_string());
    }
    if !std::io::stdin().is_terminal() {
        bail!(
            "`act-lite env add {name}` needs a terminal to prompt; pipe the value with `--stdin` instead"
        );
    }
    rpassword::prompt_password(format!("Value for {name} (hidden): "))
        .context("failed to read the secret value")
}
