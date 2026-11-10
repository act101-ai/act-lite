//! Output formatting for CLI responses.

use crate::cli::OutputFormat;
use crate::stdout;
use serde::Serialize;
use std::path::Path;

/// `path` relative to `root` with `/` separators, the form every
/// workspace-relative path takes in output on every platform; `None` when
/// `path` is not under `root`.
pub fn rel_slash(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// `path` as written in output: on Windows without the verbatim prefix
/// (`\\?\C:\`, `\\?\UNC\server\`) that `canonicalize` adds.
pub fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    if cfg!(windows) {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    text
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_slash_joins_components_with_slash() {
        let root = Path::new("ws");
        assert_eq!(
            rel_slash(root, &root.join("a").join("b.rs")).as_deref(),
            Some("a/b.rs")
        );
        assert_eq!(rel_slash(root, Path::new("other/b.rs")), None);
    }

    #[cfg(windows)]
    #[test]
    fn rel_slash_writes_slash_for_a_windows_path() {
        assert_eq!(
            rel_slash(Path::new(r"C:\ws"), Path::new(r"C:\ws\a\b.rs")).as_deref(),
            Some("a/b.rs")
        );
    }

    #[cfg(windows)]
    #[test]
    fn display_path_drops_the_verbatim_prefix() {
        assert_eq!(display_path(Path::new(r"\\?\C:\ws\a.rs")), r"C:\ws\a.rs");
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share\a.rs")),
            r"\\server\share\a.rs"
        );
        assert_eq!(display_path(Path::new(r"C:\ws\a.rs")), r"C:\ws\a.rs");
    }
}
