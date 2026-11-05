use std::path::{Path, PathBuf};

/// Errors from the sequestered secret store. Every variant says what failed,
/// where, and what to do about it.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("failed to {action} {}: {source}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to start `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "no command given to `act-lite env run`; pass one after `--`, e.g. `act-lite env run -- printenv`"
    )]
    NoCommand,

    #[error("secret '{0}' is not in this project's store; `act-lite env list` shows what is")]
    NotFound(String),

    #[error(
        "invalid secret name '{0}': a name must be a single .env key — non-empty, no whitespace \
         or '=', no leading '#', and not the word `export` followed by a space. Choose a name \
         such as MY_TOKEN and retry"
    )]
    InvalidName(String),

    #[error("value for secret '{0}' contains a newline; multi-line values are not supported")]
    MultilineValue(String),

    #[error("malformed env file {} at line {line}: {reason}", path.display())]
    MalformedEnv {
        path: PathBuf,
        line: usize,
        reason: String,
    },

    #[error(
        "`{key}` is assigned twice in {} (lines {first} and {second}) with different values; \
         remove one of them and retry",
        path.display()
    )]
    DuplicateKey {
        path: PathBuf,
        key: String,
        first: usize,
        second: usize,
    },

    #[error("no `.env` at {}; pass --workspace to choose a different project root", .0.display())]
    NoEnv(PathBuf),

    #[error("`{key}` not found in {}; use `act-lite env add {key}` to enter a new value", path.display())]
    NotInEnv { key: String, path: PathBuf },

    #[error(
        "`{key}` is marked #act:sequestered in {} but the store has no value for it; \
         use `act-lite env add {key}` to enter one, or edit the line back to a real value",
        path.display()
    )]
    MarkedButMissing { key: String, path: PathBuf },

    #[error(
        "`{key}` in {} differs from the value already in the sequestered store; \
         sequestering it would overwrite the stored value. Re-run with --force to replace it.",
        path.display()
    )]
    StoreCollision { key: String, path: PathBuf },

    #[error(
        "`{key}` in {} differs from the value in the sequestered store; restoring \
         would overwrite the .env value. Re-run with --force to replace it.",
        path.display()
    )]
    EnvCollision { key: String, path: PathBuf },

    #[error(
        "value for secret '{0}' contains both a double quote and a single quote; the .env grammar \
         has no escapes, so it cannot be stored. Remove one kind of quote and retry"
    )]
    UnquotableValue(String),
}

/// Adapter for `.map_err(io_err("read", &path))` on filesystem calls.
pub(crate) fn io_err(
    action: &'static str,
    path: &Path,
) -> impl FnOnce(std::io::Error) -> SecretsError {
    let path = path.to_path_buf();
    move |source| SecretsError::Io {
        action,
        path,
        source,
    }
}

/// Create a new, empty, private (unix `0600`) file at `tmp`. Never touches
/// the filesystem if a file is already there: the error names `tmp` exactly
/// and tells the caller to delete it. Shared by `store::save_store` and
/// `write_user_file_atomic` so a pre-existing temp file is never silently
/// deleted or overwritten by either.
pub(crate) fn create_temp_file(tmp: &Path) -> Result<std::fs::File, SecretsError> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(tmp).map_err(|source| {
        let action = if source.kind() == std::io::ErrorKind::AlreadyExists {
            "create a temp file at (one already exists there, left over from an earlier run; \
             delete it and retry)"
        } else {
            "create a temp file at"
        };
        SecretsError::Io {
            action,
            path: tmp.to_path_buf(),
            source,
        }
    })
}

/// Report a failure at `action` that happened after `tmp` was already
/// created by this call. Removes the now-stray temp file at `tmp`, since it
/// holds plaintext that must not survive a failed save; on success the
/// error keeps naming `target` and the original `source`, exactly as before
/// cleanup was attempted. If removing `tmp` also fails, the error instead
/// uses `cleanup_failed_action` and names `tmp` itself, so the caller knows
/// precisely what to delete by hand.
pub(crate) fn err_after_temp_created(
    tmp: &Path,
    target: &Path,
    action: &'static str,
    cleanup_failed_action: &'static str,
    source: std::io::Error,
) -> SecretsError {
    match std::fs::remove_file(tmp) {
        Ok(()) => SecretsError::Io {
            action,
            path: target.to_path_buf(),
            source,
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SecretsError::Io {
            action,
            path: target.to_path_buf(),
            source,
        },
        Err(_) => SecretsError::Io {
            action: cleanup_failed_action,
            path: tmp.to_path_buf(),
            source,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_names_action_and_path() {
        let e = io_err("read", Path::new("/p/.env"))(std::io::Error::other("boom"));
        assert_eq!(e.to_string(), "failed to read /p/.env: boom");
    }

    #[test]
    fn spawn_error_names_the_program() {
        let e = SecretsError::Spawn {
            program: "wrnagler".into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        };
        assert!(e.to_string().starts_with("failed to start `wrnagler`: "));
    }
}
