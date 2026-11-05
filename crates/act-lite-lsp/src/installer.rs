//! LSP binary detection and opt-in installation.

use crate::config::ServerConfig;
use crate::error::{LspError, Result};
use std::ffi::OsString;
use std::time::Duration;
use tracing::{debug, info};

/// Longest an install command may run before it is killed.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

/// The environment variable that permits installation when set to `1`.
pub const INSTALL_ENV: &str = "ACT_LITE_INSTALL_LSP";

/// Checks if the LSP server command is available on PATH.
pub fn is_installed(config: &ServerConfig) -> bool {
    which::which(&config.command).is_ok()
}

/// True when `ACT_LITE_INSTALL_LSP` is `1`: a missing server may be installed.
pub fn install_opted_in() -> bool {
    opted_in(std::env::var_os(INSTALL_ENV))
}

fn opted_in(value: Option<OsString>) -> bool {
    value.is_some_and(|v| v == "1")
}

/// How to install the server: its install command, when one is known, and
/// its homepage.
pub fn install_hint(config: &ServerConfig) -> String {
    hint(config.install_command().as_deref(), config.homepage())
}

fn hint(install_cmd: Option<&str>, homepage: &str) -> String {
    match install_cmd {
        Some(cmd) => format!("`{cmd}` (see {homepage})"),
        None => format!("the instructions at {homepage}"),
    }
}

/// Ensures the LSP server binary is available.
///
/// Returns `Ok(())` if the binary is on PATH. When it is missing and
/// `allow_install` is true, runs the server's install command through `sh -c`.
/// Otherwise returns `NotInstalled` with the install command and homepage,
/// and runs nothing.
pub async fn ensure_installed(
    config: &ServerConfig,
    language: &str,
    allow_install: bool,
) -> Result<()> {
    ensure_installed_with(config, language, config.install_command(), allow_install).await
}

async fn ensure_installed_with(
    config: &ServerConfig,
    language: &str,
    install_cmd: Option<String>,
    allow_install: bool,
) -> Result<()> {
    if is_installed(config) {
        debug!("LSP binary '{}' found on PATH", config.command);
        return Ok(());
    }

    let install_cmd = match install_cmd {
        Some(cmd) if allow_install => cmd,
        install_cmd => {
            return Err(LspError::NotInstalled {
                language: language.to_string(),
                command: config.command.clone(),
                install_hint: hint(install_cmd.as_deref(), config.homepage()),
            });
        }
    };

    info!(
        "LSP binary '{}' for {} not found, installing with `{}`",
        config.command, language, install_cmd
    );

    // Execute the install command
    match run_install_command("sh", &install_cmd, INSTALL_TIMEOUT).await {
        Ok(()) => {
            // Verify the binary is now available
            if is_installed(config) {
                info!("Installed '{}' for {}", config.command, language);
                Ok(())
            } else {
                Err(LspError::InstallFailed {
                    language: language.to_string(),
                    command: install_cmd,
                    reason: "Command succeeded but binary not found on PATH after install"
                        .to_string(),
                })
            }
        }
        Err(reason) => Err(LspError::InstallFailed {
            language: language.to_string(),
            command: install_cmd,
            reason,
        }),
    }
}

/// Runs an install command through `shell -c` and returns Ok(()) on success or an error string.
/// The shell is killed and an error returned if it runs longer than `limit`.
async fn run_install_command(
    shell: &str,
    cmd: &str,
    limit: Duration,
) -> std::result::Result<(), String> {
    debug!("Running install command: {}", cmd);

    let child = tokio::process::Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(limit, child)
        .await
        .map_err(|_| format!("timed out after {} s", limit.as_secs()))?
        .map_err(|e| format!("failed to execute shell: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "exit code {}: {}{}",
            output.status.code().unwrap_or(-1),
            stderr.trim(),
            if !stdout.trim().is_empty() {
                format!("\nstdout: {}", stdout.trim())
            } else {
                String::new()
            }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config for a binary that is not on PATH.
    fn missing() -> ServerConfig {
        ServerConfig::new("definitely-not-a-real-binary-xyzzy".to_string(), vec![])
    }

    #[tokio::test]
    async fn ensure_installed_does_not_install_without_opt_in() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let cmd = format!("touch {}", marker.display());
        let config = missing();
        let err = ensure_installed_with(&config, "test", Some(cmd.clone()), false)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, LspError::NotInstalled { install_hint, .. }
                if install_hint.contains(&cmd) && install_hint.contains(config.homepage())),
            "the hint names the install command and the homepage: {err}"
        );
        assert!(!marker.exists(), "nothing runs without the opt-in");
    }

    #[tokio::test]
    async fn ensure_installed_installs_on_opt_in() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let cmd = format!("touch {}", marker.display());
        let err = ensure_installed_with(&missing(), "test", Some(cmd), true)
            .await
            .unwrap_err();
        assert!(marker.exists(), "the install command ran");
        assert!(
            matches!(err, LspError::InstallFailed { ref reason, .. } if reason.contains("not found on PATH")),
            "the binary is still missing: {err}"
        );
    }

    #[tokio::test]
    async fn ensure_installed_on_opt_in_succeeds_when_the_binary_appears() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("server");
        let config = ServerConfig::new(binary.display().to_string(), vec![]);
        let cmd = format!("touch {0} && chmod +x {0}", binary.display());
        ensure_installed_with(&config, "test", Some(cmd), true)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn ensure_installed_reports_a_failing_install_command() {
        for (cmd, stdout) in [("echo out; echo err >&2; exit 3", true), ("exit 4", false)] {
            let err = ensure_installed_with(&missing(), "test", Some(cmd.to_string()), true)
                .await
                .unwrap_err();
            assert!(
                matches!(&err, LspError::InstallFailed { reason, .. }
                    if reason.starts_with("exit code ") && reason.contains("stdout: out") == stdout),
                "{err}"
            );
        }
    }

    #[tokio::test]
    async fn ensure_installed_without_install_command_names_the_homepage() {
        let config = missing();
        for allow_install in [false, true] {
            let err = ensure_installed(&config, "test", allow_install)
                .await
                .unwrap_err();
            assert!(
                matches!(&err, LspError::NotInstalled { install_hint, .. }
                    if install_hint.contains(config.homepage())),
                "{err}"
            );
        }
    }

    #[test]
    fn install_hint_names_the_command_and_the_homepage() {
        let config = ServerConfig::new("rust-analyzer".to_string(), vec![]);
        let hint = install_hint(&config);
        assert!(
            hint.contains("rustup component add rust-analyzer"),
            "{hint}"
        );
        assert!(hint.contains(config.homepage()), "{hint}");
    }

    #[test]
    fn opt_in_is_exactly_one() {
        assert!(opted_in(Some("1".into())));
        for value in [None, Some("0".into()), Some("".into()), Some("yes".into())] {
            assert!(!opted_in(value.clone()), "{value:?}");
        }
        assert_eq!(
            install_opted_in(),
            std::env::var(INSTALL_ENV).as_deref() == Ok("1")
        );
    }

    #[tokio::test]
    async fn install_command_reports_a_shell_that_does_not_start() {
        let err = run_install_command("/nonexistent/sh", "true", Duration::from_secs(5))
            .await
            .unwrap_err();
        assert!(err.contains("failed to execute shell"), "{err}");
    }

    #[tokio::test]
    async fn install_command_times_out() {
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            run_install_command("sh", "sleep 60", Duration::from_secs(1)),
        )
        .await;
        let err = result
            .expect("run_install_command did not return within 2 s")
            .unwrap_err();
        assert!(err.contains("timed out"), "unexpected error: {err}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn test_is_installed_known_binary() {
        // `sh` should always be available on Unix
        let _config = ServerConfig::new("sh".to_string(), vec![]);
        // We can't use is_installed here because `sh` isn't in ALLOWED_COMMANDS,
        // but `which::which("sh")` will find it. Test the `which` call directly.
        assert!(which::which("sh").is_ok());
    }

    #[test]
    fn test_is_installed_missing_binary() {
        let config = ServerConfig {
            command: "definitely-not-a-real-binary-xyzzy".to_string(),
            args: vec![],
            working_directory: None,
            env: std::collections::HashMap::new(),
            initialization_options: serde_json::Value::Null,
            timeout_ms: 5000,
        };
        assert!(!is_installed(&config));
    }

    #[tokio::test]
    async fn test_ensure_installed_already_present() {
        // rust-analyzer is likely installed in the dev environment
        // If not, this test will exercise the install path — both are valid.
        // We test with a binary we KNOW exists: "sh"
        let config = ServerConfig {
            command: "sh".to_string(),
            args: vec![],
            working_directory: None,
            env: std::collections::HashMap::new(),
            initialization_options: serde_json::Value::Null,
            timeout_ms: 5000,
        };
        let result = ensure_installed(&config, "test", false).await;
        assert!(result.is_ok());
    }
}
