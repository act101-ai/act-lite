//! Writes a fake executable (a `#!/bin/sh` stand-in for `act`, a language
//! server, a git hook, ...) that a test can exec immediately, from any thread,
//! without `ETXTBSY` ("Text file busy", os error 26).
//!
//! Shared by every test tree that execs a script it just wrote; each crate
//! includes this one file with `#[path = ".../tests/support/executable_script.rs"]`.
//!
//! The race this closes: `std::fs::write` holds a writable fd on the script
//! for a moment. Cargo runs tests on many threads, and any other thread that
//! spawns a process in that moment forks a child that inherits a copy of the
//! fd. `O_CLOEXEC` closes the copy only when that child execs, so until then
//! the inode has a writer and the kernel refuses to exec it with `ETXTBSY`.
//! Retrying or probing only narrows the window. Here this process never opens
//! the script for writing at all: a `/bin/sh` child writes it, and its fd
//! table is private to it, so no sibling fork can inherit a writer. Once that
//! child has exited, the inode has no writer anywhere. The mode is set with
//! `chmod`, which opens no fd.

use std::path::Path;

/// Writes `contents` to `path` (creating or truncating it) and makes it
/// executable (`0o755`), safe to exec as soon as this returns.
///
/// `/bin/sh` is named absolutely: some tests point `PATH` at a scratch
/// directory while others run.
#[cfg(unix)]
pub fn write_executable(path: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let status = Command::new("/bin/sh")
        .args(["-c", "printf '%s' \"$1\" > \"$2\"", "sh", contents])
        .arg(path)
        .status()
        .unwrap_or_else(|e| panic!("could not spawn /bin/sh to write {}: {e}", path.display()));
    assert!(
        status.success(),
        "/bin/sh failed ({status}) writing the test executable {}",
        path.display()
    );
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|e| panic!("could not chmod 755 {}: {e}", path.display()));
}

/// Off unix there is no exec bit and no `ETXTBSY`; the script is only written.
#[cfg(not(unix))]
pub fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap_or_else(|e| {
        panic!(
            "could not write the test executable {}: {e}",
            path.display()
        )
    });
}
