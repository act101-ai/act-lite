//! Output formatting for CLI responses.

use crate::cli::OutputFormat;
use crate::stdout;
use serde::Serialize;

/// Outputs data in the specified format.
pub fn output<T: Serialize>(data: &T, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            stdout::try_with_stdout(|w| writeln!(w, "{}", serde_json::to_string(data)?))?;
        }
        OutputFormat::Text => {
            stdout::try_with_stdout(|w| writeln!(w, "{}", serde_json::to_string_pretty(data)?))?;
        }
    }
    Ok(())
}

/// Outputs an error in the specified format.
pub fn output_error(error: &anyhow::Error, format: OutputFormat) {
    let error_obj = serde_json::json!({
        "error": {
            "message": error.to_string(),
            "chain": error.chain().skip(1).map(|e| e.to_string()).collect::<Vec<_>>()
        }
    });

    match format {
        OutputFormat::Json => {
            eprintln!("{}", serde_json::to_string(&error_obj).unwrap_or_default());
        }
        OutputFormat::Text => {
            eprintln!("Error: {}", error);
            for cause in error.chain().skip(1) {
                eprintln!("  Caused by: {}", cause);
            }
        }
    }
}
