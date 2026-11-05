//! Serializes the tests of this crate that read or change the process `PATH`.

use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

static PATH_MUTEX: Mutex<()> = Mutex::new(());

/// Held while a test resolves or spawns a program through `PATH`, so no other
/// test changes `PATH` meanwhile.
pub struct PathLock {
    /// Held, not read: it is the exclusion itself, released on drop.
    _lock: MutexGuard<'static, ()>,
}

pub fn lock_path() -> PathLock {
    PathLock {
        _lock: PATH_MUTEX.lock().unwrap_or_else(|e| e.into_inner()),
    }
}

/// Sets `PATH` while held, and restores the previous value on drop.
pub struct PathGuard {
    previous: Option<OsString>,
    _lock: PathLock,
}

impl PathGuard {
    pub fn set(dir: &Path) -> Self {
        let lock = lock_path();
        let previous = std::env::var_os("PATH");
        // SAFETY: every PATH writer in this binary holds the lock.
        unsafe { std::env::set_var("PATH", dir) };
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        // SAFETY: as above — still holding the lock.
        unsafe {
            match self.previous.take() {
                Some(previous) => std::env::set_var("PATH", previous),
                None => std::env::remove_var("PATH"),
            }
        }
    }
}
