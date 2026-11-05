#[cfg(unix)]
use tokio::process::{Child, Command};

/// Watchdog script: loop while BOTH the watched parent and the target
/// process group are alive; once either is gone, TERM then KILL the group
/// (both no-ops on ESRCH when the group died first — normal shutdown).
/// `$1` = parent pid, `$2` = target process-group id.
#[cfg(unix)]
const WATCHDOG_SH: &str = r#"
while kill -0 "$1" 2>/dev/null && kill -0 -"$2" 2>/dev/null; do
  sleep 1
done
kill -TERM -"$2" 2>/dev/null
sleep 1
kill -KILL -"$2" 2>/dev/null
"#;

#[cfg(unix)]
pub fn spawn_with_parent_death_guard(
    mut command: Command,
    parent_pid: u32,
) -> std::io::Result<Child> {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command as StdCommand, Stdio};

    // Own group so a group-kill on the server cannot signal the watchdog,
    // and a group-kill on act cannot take the guard down with it.
    command.process_group(0);
    command.kill_on_drop(true);
    let child = command.spawn()?;
    let pgid = child
        .id()
        .map(|id| id as i32)
        .ok_or_else(|| std::io::Error::other("spawned child has no pid"))?;

    let mut watchdog = StdCommand::new("/bin/sh");
    watchdog
        .arg("-c")
        .arg(WATCHDOG_SH)
        .arg("act-lsp-watchdog")
        .arg(parent_pid.to_string())
        .arg(pgid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let spawned = watchdog.spawn();
    // The watchdog is best-effort hardening on top of Drop/kill_on_drop:
    // a spawn failure (no /bin/sh, fd exhaustion) must not fail the LSP
    // server itself. The group-kill paths in `LspClient` still exist.
    if let Err(e) = spawned {
        tracing::warn!(
            "LSP parent-death watchdog did not start (server still guarded by Drop): {e}"
        );
    }
    Ok(child)
}

#[cfg(all(test, unix))]
mod tests {
    use super::spawn_with_parent_death_guard;
    use std::time::{Duration, Instant};
    use tokio::process::Command as TokioCommand;

    /// The child counts as alive only if `kill(pid, 0)` succeeds AND it is
    /// still running: `kill(pid, 0)` also succeeds for a zombie, and the
    /// guarded child is a direct child of this test process (tokio reaps
    /// lazily), so once the watchdog kills the group the pid lingers as a
    /// not-yet-reaped zombie that a bare `kill` probe would call alive
    /// forever. `waitpid(WNOHANG)` distinguishes the two: 0 = still
    /// running, pid = exited (reaped here), -1 = not our child.
    fn alive(pid: u32) -> bool {
        // SAFETY: `kill` with signal 0 performs the permission/existence
        // check only; no signal is delivered and no memory is touched.
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            return false;
        }
        let mut status = 0;
        // SAFETY: plain syscall on our own child's pid; WNOHANG never blocks.
        unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) == 0 }
    }

    /// Kill `pid` with SIGKILL, ignoring the result (racy by design).
    fn hard_kill(pid: u32) {
        // SAFETY: plain syscall; ESRCH on an already-dead pid is ignored.
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }

    /// Both tests clean up every process they spawn on pass AND fail paths —
    /// a test about not leaking processes must not leak processes.
    struct KillOnDrop(Option<u32>);
    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            if let Some(pid) = self.0 {
                hard_kill(pid);
            }
        }
    }

    #[test]
    fn guarded_child_dies_when_parent_dies() {
        // A sacrificial parent stands in for act: the guard watches its pid.
        let mut parent = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("sacrificial parent");
        let parent_pid = parent.id();
        let _parent_guard = KillOnDrop(Some(parent_pid));

        let mut command = TokioCommand::new("sleep");
        command.arg("300");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let child = rt
            .block_on(async { spawn_with_parent_death_guard(command, parent_pid) })
            .expect("guarded child");
        let child_pid = child.id().expect("child pid");
        let _child_guard = KillOnDrop(Some(child_pid));

        // SIGKILL the parent: no destructors run anywhere — the worst case.
        hard_kill(parent_pid);
        let _ = parent.wait();

        let deadline = Instant::now() + Duration::from_secs(15);
        while alive(child_pid) {
            assert!(
                Instant::now() < deadline,
                "guarded child (pid {child_pid}) survived its parent's death — watchdog did not fire"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    #[test]
    fn unguarded_control_child_survives_parent_death() {
        // The control that makes the first test bite: WITHOUT the guard the
        // same setup leaves the child alive, so the first test's pass is the
        // guard's effect, not the OS reaping something.
        let mut parent = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("sacrificial parent");
        let parent_pid = parent.id();
        let _parent_guard = KillOnDrop(Some(parent_pid));

        let mut control = std::process::Command::new("sleep")
            .arg("300")
            .spawn()
            .expect("unguarded control child");
        let control_pid = control.id();
        let _control_guard = KillOnDrop(Some(control_pid));

        hard_kill(parent_pid);
        let _ = parent.wait();

        std::thread::sleep(Duration::from_secs(3));
        assert!(
            alive(control_pid),
            "control child died without a guard — test premise broken"
        );
        // Reap the control child: the assert only proves it alive, its
        // cleanup still has to kill + wait() (clippy::zombie_processes).
        let _ = control.kill();
        let _ = control.wait();
    }

    #[test]
    fn watchdog_kills_the_whole_group_not_just_the_direct_child() {
        let mut parent = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("sacrificial parent");
        let parent_pid = parent.id();
        let _parent_guard = KillOnDrop(Some(parent_pid));

        // The descendant publishes its pid as its first act, so it can be
        // read and armed for cleanup BEFORE the parent dies.
        let pidfile = std::env::temp_dir().join(format!(
            "f2236-watchdog-groupkill-{}-{}.pid",
            std::process::id(),
            line!()
        ));
        let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());
        let mut command = TokioCommand::new("/bin/sh");
        command.arg("-c").arg(&script);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let child = rt
            .block_on(async { spawn_with_parent_death_guard(command, parent_pid) })
            .expect("guarded child");
        let child_pid = child.id().expect("child pid");
        let _child_guard = KillOnDrop(Some(child_pid));

        let descendant: u32 = {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match std::fs::read_to_string(&pidfile) {
                    Ok(text) => {
                        let _ = std::fs::remove_file(&pidfile);
                        break text.trim().parse::<u32>().expect("pid in pidfile");
                    }
                    Err(_) => {
                        assert!(
                            Instant::now() < deadline,
                            "guarded sh (pid {child_pid}) never wrote the descendant pidfile"
                        );
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        };
        // Armed before the parent dies, so the sleep is cleaned up on every
        // path below — pass and fail.
        let _descendant_guard = KillOnDrop(Some(descendant));

        // SIGKILL the parent: no destructors run anywhere — the worst case.
        hard_kill(parent_pid);
        let _ = parent.wait();

        let deadline = Instant::now() + Duration::from_secs(15);
        while alive(child_pid) {
            assert!(
                Instant::now() < deadline,
                "guarded sh (pid {child_pid}) survived its parent's death — watchdog did not fire"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
        // The group kill must have reached the backgrounded descendant too.
        // The descendant is NOT this process's child (its parent sh died), so
        // the zombie-aware `alive` helper cannot be reused on it: its waitpid
        // returns -1 (ECHILD) and the helper would answer `false` while the
        // sleep is still running. Probe with kill-0 directly — a live `sleep`
        // answers signal 0; a killed-and-reaped one answers ESRCH.
        // SAFETY: `kill` with signal 0 performs the existence/permission
        // check only; no signal is delivered.
        let deadline = Instant::now() + Duration::from_secs(10);
        while unsafe { libc::kill(descendant as i32, 0) } == 0 {
            assert!(
                Instant::now() < deadline,
                "descendant sleep (pid {descendant}) survived the watchdog's group kill"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}
