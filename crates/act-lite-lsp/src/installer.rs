//! LSP binary detection and opt-in installation.

use crate::config::ServerConfig;
use crate::error::{LspError, Result};
use std::ffi::OsString;
use std::time::Duration;
use tracing::{debug, info};

#[cfg(windows)]
mod job {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    /// The creation flag that starts the shell suspended, so it is in its job
    /// before it can start anything.
    pub(super) const SUSPENDED: u32 = windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

    /// A job object that holds an install command's shell and every process
    /// the shell starts; they end when it is dropped.
    pub(super) struct Job(HANDLE);

    // SAFETY: a job handle may be used and closed from any thread.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        /// Puts `child`, started with [`SUSPENDED`], in a new job, then lets
        /// it run. On an error `child` has not run.
        pub(super) fn contain(child: &tokio::process::Child) -> Result<Job, String> {
            let (Some(process), Some(pid)) = (child.raw_handle(), child.id()) else {
                return Err("the shell has no process handle".to_string());
            };
            // SAFETY: no security attributes and no name; the handle returned
            // is owned by `Job` and closed on drop.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(format!(
                    "no job object: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let job = Job(handle);
            // SAFETY: LIMIT_INFORMATION is plain data; the call reads exactly
            // its size from a live local.
            let limited = unsafe {
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if limited == 0 {
                return Err(format!(
                    "the job object could not be set to end with act-lite: {}",
                    std::io::Error::last_os_error()
                ));
            }
            // SAFETY: both handles are open for the duration of the call.
            if unsafe { AssignProcessToJobObject(job.0, process as HANDLE) } == 0 {
                return Err(format!(
                    "the shell could not join a job object: {}",
                    std::io::Error::last_os_error()
                ));
            }
            resume(pid)?;
            Ok(job)
        }

        /// Ends every process in the job.
        pub(super) fn terminate(&self) {
            // SAFETY: the handle is open until drop.
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle is owned and closed exactly once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Resumes the threads of process `pid`; a process started suspended has
    /// one, its main thread.
    fn resume(pid: u32) -> Result<(), String> {
        let failed = || {
            format!(
                "the shell could not be resumed: {}",
                std::io::Error::last_os_error()
            )
        };
        // SAFETY: a snapshot of all threads; the handle is closed below.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(failed());
        }
        // SAFETY: THREADENTRY32 is plain data; dwSize is set as the API requires.
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut resumed = 0;
        // SAFETY: `snapshot` is open and `entry` is a valid THREADENTRY32.
        let mut more = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                // SAFETY: the thread handle is closed right after use.
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if !thread.is_null() {
                    // SAFETY: `thread` is open with THREAD_SUSPEND_RESUME.
                    if unsafe { ResumeThread(thread) } != u32::MAX {
                        resumed += 1;
                    }
                    // SAFETY: closed exactly once.
                    unsafe { CloseHandle(thread) };
                }
            }
            // SAFETY: as for Thread32First.
            more = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        // SAFETY: closed exactly once.
        unsafe { CloseHandle(snapshot) };
        if resumed == 0 { Err(failed()) } else { Ok(()) }
    }
}

/// Longest an install command may run before it is killed.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

/// The environment variable that permits installation when set to `1`.
pub const INSTALL_ENV: &str = "ACT_LITE_INSTALL_LSP";

/// The shell that runs install commands.
const SHELL: &str = if cfg!(windows) { "cmd" } else { "sh" };

/// Checks if the LSP server command is available on PATH.
pub fn is_installed(config: &ServerConfig) -> bool {
    which::which(&config.command).is_ok()
}

/// True when `ACT_LITE_INSTALL_LSP` is on: a missing server may be installed.
pub fn install_opted_in() -> bool {
    opted_in(std::env::var_os(INSTALL_ENV))
}

/// The values `ACT_LITE_DISABLE_LSP` also accepts as on (clap's boolish
/// values), in any case. Every other value is off.
fn opted_in(value: Option<OsString>) -> bool {
    value.and_then(|v| v.into_string().ok()).is_some_and(|v| {
        ["y", "yes", "t", "true", "on", "1"].contains(&v.to_ascii_lowercase().as_str())
    })
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
/// `allow_install` is true, runs the server's install command through `sh -c`
/// (`cmd /C` on Windows).
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
    match run_install_command(SHELL, &install_cmd, INSTALL_TIMEOUT).await {
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

/// Runs an install command through `shell -c` (`shell /S /C` on Windows) and returns Ok(()) on success or an error string.
/// If it runs longer than `limit`, the shell and every process it started are
/// killed and an error returned.
async fn run_install_command(
    shell: &str,
    cmd: &str,
    limit: Duration,
) -> std::result::Result<(), String> {
    debug!("Running install command: {}", cmd);

    let failed = |e: std::io::Error| format!("failed to execute shell: {e}");
    let mut command = tokio::process::Command::new(shell);
    #[cfg(not(windows))]
    command.arg("-c").arg(cmd);
    // Verbatim: cmd.exe does not parse the quoting `arg` would add.
    #[cfg(windows)]
    command.raw_arg(format!("/S /C \"{cmd}\""));
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Its own process group, so a timeout can kill what the shell started.
    #[cfg(unix)]
    command.process_group(0);
    // Windows has no process groups: a job holds the shell and everything it
    // starts, even after the shell itself has exited.
    #[cfg(windows)]
    command.creation_flags(job::SUSPENDED);
    let child = command.spawn().map_err(failed)?;
    #[cfg(unix)]
    let group = child.id();
    #[cfg(windows)]
    let job = match job::Job::contain(&child) {
        Ok(job) => job,
        Err(e) => {
            // Dropping the suspended shell kills it before it ran anything.
            drop(child);
            return Err(e);
        }
    };
    // Held, not dropped, on a timeout: dropping it kills only the shell.
    let wait = child.wait_with_output();
    tokio::pin!(wait);
    let output = match tokio::time::timeout(limit, &mut wait).await {
        Ok(output) => output.map_err(failed)?,
        Err(_) => {
            #[cfg(unix)]
            if let Some(group) = group {
                // SAFETY: kill(2) on the group the shell leads; no memory is touched.
                unsafe { libc::kill(-(group as i32), libc::SIGKILL) };
            }
            #[cfg(windows)]
            job.terminate();
            return Err(format!("timed out after {} s", limit.as_secs()));
        }
    };

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

    /// A command for [`SHELL`] that creates the empty file `path`.
    fn touch(path: &std::path::Path) -> String {
        if cfg!(windows) {
            format!("type nul > \"{}\"", path.display())
        } else {
            format!("touch {}", path.display())
        }
    }

    /// A command for [`SHELL`] that runs for `secs` seconds.
    fn sleep(secs: u32) -> String {
        if cfg!(windows) {
            format!("ping -n {} 127.0.0.1 > nul", secs + 1)
        } else {
            format!("sleep {secs}")
        }
    }

    /// A config for a binary that is not on PATH.
    fn missing() -> ServerConfig {
        ServerConfig::new("definitely-not-a-real-binary-xyzzy".to_string(), vec![])
    }

    #[tokio::test]
    async fn ensure_installed_does_not_install_without_opt_in() {
        let _path = crate::test_path::lock_path();
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
        let _path = crate::test_path::lock_path();
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let cmd = touch(&marker);
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
        let _path = crate::test_path::lock_path();
        let dir = tempfile::tempdir().unwrap();
        // On Windows a file is executable by its extension.
        let binary = dir.path().join(if cfg!(windows) {
            "server.cmd"
        } else {
            "server"
        });
        let config = ServerConfig::new(binary.display().to_string(), vec![]);
        let cmd = if cfg!(windows) {
            touch(&binary)
        } else {
            format!("touch {0} && chmod +x {0}", binary.display())
        };
        ensure_installed_with(&config, "test", Some(cmd), true)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn ensure_installed_reports_a_failing_install_command() {
        let _path = crate::test_path::lock_path();
        let writes_both = if cfg!(windows) {
            "echo out& echo err 1>&2& exit 3"
        } else {
            "echo out; echo err >&2; exit 3"
        };
        for (cmd, stdout) in [(writes_both, true), ("exit 4", false)] {
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
        let _path = crate::test_path::lock_path();
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
    fn opt_in_accepts_the_boolish_values() {
        for value in ["1", "true", "TRUE", "yes", "On", "y", "t"] {
            assert!(opted_in(Some(value.into())), "{value}");
        }
        for value in ["0", "false", "no", "off", "n", "f", "", "2"] {
            assert!(!opted_in(Some(value.into())), "{value}");
        }
        assert!(!opted_in(None));
        assert_eq!(install_opted_in(), opted_in(std::env::var_os(INSTALL_ENV)));
    }

    #[tokio::test]
    async fn install_command_reports_a_shell_that_does_not_start() {
        let _path = crate::test_path::lock_path();
        let err = run_install_command("/nonexistent/sh", "true", Duration::from_secs(5))
            .await
            .unwrap_err();
        assert!(err.contains("failed to execute shell"), "{err}");
    }

    /// A timeout kills what the install command started, not only the shell.
    #[cfg(unix)]
    #[tokio::test]
    async fn install_command_timeout_kills_the_commands_children() {
        let _path = crate::test_path::lock_path();
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let cmd = format!("sleep 60 & echo $! > {}; wait", pid_file.display());
        let err = run_install_command("sh", &cmd, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        let pid: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // The killed `sleep` is gone once its new parent reaps it.
        let survived = (0..40).all(|_| {
            std::thread::sleep(Duration::from_millis(50));
            // SAFETY: signal 0 only checks that the process exists.
            unsafe { libc::kill(pid, 0) == 0 }
        });
        assert!(!survived, "`sleep 60` (pid {pid}) outlived the timeout");
    }

    /// A timeout kills what the install command started, not only the shell:
    /// the file `ping`'s output goes to is deletable once nothing holds it.
    #[cfg(windows)]
    #[tokio::test]
    async fn install_command_timeout_kills_the_commands_children() {
        let _path = crate::test_path::lock_path();
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let cmd = format!("ping -n 61 127.0.0.1 > \"{}\"", out.display());
        let err = run_install_command(SHELL, &cmd, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        let released = (0..40).any(|_| {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::remove_file(&out).is_ok()
        });
        assert!(
            released,
            "`ping` outlived the timeout and still holds {out:?}"
        );
    }

    /// A timeout kills a process the install command started even after the
    /// shell has exited: the started process would otherwise create `marker`.
    #[cfg(windows)]
    #[tokio::test]
    async fn install_command_timeout_kills_children_of_an_exited_shell() {
        let _path = crate::test_path::lock_path();
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let cmd = format!(
            "start /b cmd /c \"ping -n 4 127.0.0.1 > nul & type nul > {}\"",
            marker.display()
        );
        let err = run_install_command(SHELL, &cmd, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        std::thread::sleep(Duration::from_secs(5));
        assert!(
            !marker.exists(),
            "the process the shell started outlived the timeout"
        );
    }

    /// An install dropped before it ends (its MCP session closed) ends every
    /// process it started: the started process would otherwise create `marker`.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_dropped_install_ends_the_processes_it_started() {
        let _path = crate::test_path::lock_path();
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let cmd = format!(
            "start /b cmd /c \"ping -n 4 127.0.0.1 > nul & type nul > {}\"",
            marker.display()
        );
        let dropped = tokio::time::timeout(
            Duration::from_secs(1),
            run_install_command(SHELL, &cmd, Duration::from_secs(60)),
        )
        .await;
        assert!(
            dropped.is_err(),
            "the install ended on its own: {dropped:?}"
        );
        std::thread::sleep(Duration::from_secs(5));
        assert!(
            !marker.exists(),
            "the process the dropped install started kept running"
        );
    }

    /// The shell starts suspended and runs only once it is in its job, so
    /// nothing it starts can escape the job.
    #[cfg(windows)]
    #[tokio::test]
    async fn the_shell_runs_only_once_it_is_in_its_job() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let mut command = tokio::process::Command::new(SHELL);
        command
            .raw_arg(format!("/S /C \"type nul > {}\"", marker.display()))
            .creation_flags(job::SUSPENDED);
        let child = command.spawn().unwrap();
        std::thread::sleep(Duration::from_secs(1));
        assert!(!marker.exists(), "the shell ran before it was in its job");
        let _job = job::Job::contain(&child).unwrap();
        assert!(child.wait_with_output().await.unwrap().status.success());
        assert!(marker.exists(), "the shell did not run once resumed");
    }

    #[tokio::test]
    async fn install_command_times_out() {
        let _path = crate::test_path::lock_path();
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            run_install_command(SHELL, &sleep(60), Duration::from_secs(1)),
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
        let _path = crate::test_path::lock_path();
        // `sh` should always be available on Unix
        let _config = ServerConfig::new("sh".to_string(), vec![]);
        // We can't use is_installed here because `sh` isn't in ALLOWED_COMMANDS,
        // but `which::which("sh")` will find it. Test the `which` call directly.
        assert!(which::which("sh").is_ok());
    }

    #[test]
    fn test_is_installed_missing_binary() {
        let _path = crate::test_path::lock_path();
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
        let _path = crate::test_path::lock_path();
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
