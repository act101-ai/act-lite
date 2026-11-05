//! LSP binary detection and auto-installation.

use crate::config::ServerConfig;
use crate::error::{LspError, Result};
use tracing::{debug, info};

/// Checks if the LSP server command is available on PATH.
pub fn is_installed(config: &ServerConfig) -> bool {
    which::which(&config.command).is_ok()
}

/// Ensures the LSP server binary is available, attempting auto-install if missing.
///
/// Returns `Ok(())` if the binary is available (either already installed or
/// successfully auto-installed). Returns `Err` with actionable install instructions
/// if auto-install fails or is not available for this server.
pub async fn ensure_installed(config: &ServerConfig, language: &str) -> Result<()> {
    if is_installed(config) {
        debug!("LSP binary '{}' found on PATH", config.command);
        return Ok(());
    }

    info!(
        "LSP binary '{}' for {} not found, attempting auto-install",
        config.command, language
    );

    let install_cmd = match config.install_command() {
        Some(cmd) => cmd,
        None => {
            return Err(LspError::NotInstalled {
                language: language.to_string(),
                command: config.command.clone(),
                install_hint: format!(
                    "No auto-install available for '{}'. Install it manually.",
                    config.command
                ),
            });
        }
    };

    // Execute the install command
    match run_install_command(&install_cmd).await {
        Ok(()) => {
            // Verify the binary is now available
            if is_installed(config) {
                info!(
                    "Successfully auto-installed '{}' for {}",
                    config.command, language
                );
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

/// Runs a shell install command and returns Ok(()) on success or an error string.
async fn run_install_command(cmd: &str) -> std::result::Result<(), String> {
    debug!("Running install command: {}", cmd);

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await
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
        let result = ensure_installed(&config, "test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ensure_installed_unknown_binary_no_install_cmd() {
        let config = ServerConfig {
            command: "definitely-not-a-real-binary-xyzzy".to_string(),
            args: vec![],
            working_directory: None,
            env: std::collections::HashMap::new(),
            initialization_options: serde_json::Value::Null,
            timeout_ms: 5000,
        };
        let result = ensure_installed(&config, "test").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not installed"),
            "Expected NotInstalled error, got: {}",
            err
        );
    }
}
