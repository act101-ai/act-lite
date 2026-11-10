use std::collections::BTreeMap;
use std::path::Path;

use crate::env_line::{Parsed, parse_line};
use crate::error::{self, SecretsError, io_err};

/// A project's secrets, sorted by key for stable serialization.
pub type Store = BTreeMap<String, String>;

/// Validate a secret name: valid iff `NAME=x` parses (with the one `.env`
/// line grammar) as a plain assignment whose key is exactly `name`. This
/// rejects every name the store would read back as something else — a
/// comment (`#TOK`), an `export` line (`export X`), or a different key.
pub(crate) fn validate_name(name: &str) -> Result<(), SecretsError> {
    match parse_line(&format!("{name}=x")) {
        Ok(Parsed::Assignment(a)) if a.key == name && !a.export => Ok(()),
        _ => Err(SecretsError::InvalidName(name.to_string())),
    }
}

/// Parse a whole `.env` strictly. Values are kept as raw tokens (quotes
/// included). Any line that does not parse is an error naming `path` and the
/// line; two assignments to one key with different tokens are an error too.
pub fn parse_env(content: &str, path: &Path) -> Result<Store, SecretsError> {
    let mut store = Store::new();
    let mut first_line: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let parsed = parse_line(raw).map_err(|reason| SecretsError::MalformedEnv {
            path: path.to_path_buf(),
            line: line_no,
            reason,
        })?;
        let Parsed::Assignment(a) = parsed else {
            continue;
        };
        if let Some(prev) = store.get(a.key) {
            if prev != a.token {
                return Err(SecretsError::DuplicateKey {
                    path: path.to_path_buf(),
                    key: a.key.to_string(),
                    first: first_line[a.key],
                    second: line_no,
                });
            }
            continue;
        }
        first_line.insert(a.key, line_no);
        store.insert(a.key.to_string(), a.token.to_string());
    }
    Ok(store)
}

/// Wrap `value` in quotes only when the bare form would parse differently:
/// empty, leading/trailing whitespace, inner whitespace, ` #`, or a quote
/// character. Prefers `"…"`; uses `'…'` when the value contains `"` but no `'`.
/// Callers must not pass a value holding both quote kinds — `Secrets::add`
/// rejects those with `SecretsError::UnquotableValue` before this function is
/// reached.
pub fn quote_if_needed(value: &str) -> String {
    let needs = value.is_empty()
        || value.trim() != value
        || value.contains(char::is_whitespace)
        || value.contains('"')
        || value.contains('\'')
        || value.starts_with('#');
    if !needs {
        return value.to_string();
    }
    if value.contains('"') && !value.contains('\'') {
        format!("'{value}'")
    } else {
        format!("\"{value}\"")
    }
}

/// Strip one matching pair of surrounding quotes. No escape processing.
pub fn unquote(token: &str) -> String {
    for q in ['"', '\''] {
        if token.len() >= 2 && token.starts_with(q) && token.ends_with(q) {
            return token[1..token.len() - 1].to_string();
        }
    }
    token.to_string()
}

/// Serialize the store back to `KEY=value\n` lines (sorted by key).
pub fn serialize_env(store: &Store) -> String {
    let mut out = String::new();
    for (key, value) in store {
        out.push_str(key);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    out
}

/// Load a store from disk. A missing file yields an empty store.
pub fn load_store(path: &Path) -> Result<Store, SecretsError> {
    if !path.exists() {
        return Ok(Store::new());
    }
    let content = std::fs::read_to_string(path).map_err(io_err("read", path))?;
    parse_env(&content, path)
}

/// Save a store to disk atomically: write a `0600` temp file in the same
/// directory, then rename it over `path`. A crash mid-write leaves the old
/// store intact; the rename also replaces any wider permissions the old file had.
pub fn save_store(path: &Path, store: &Store) -> Result<(), SecretsError> {
    let dir = path.parent().ok_or_else(|| SecretsError::Io {
        action: "locate the parent directory of",
        path: path.to_path_buf(),
        source: std::io::Error::other("store path has no parent"),
    })?;
    std::fs::create_dir_all(dir).map_err(io_err("create directory", dir))?;
    restrict_dir(dir)?;

    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("store"),
        std::process::id()
    ));
    write_private(&tmp, &serialize_env(store))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        // Remove the temp file so a failed save leaves no stray plaintext; if
        // that also fails, the error names the temp path itself so the
        // caller knows exactly what to delete.
        return Err(error::err_after_temp_created(
            &tmp,
            path,
            "replace",
            "replace (the temp file also could not be removed; delete it manually) at",
            e,
        ));
    }
    Ok(())
}

/// Run `f` while holding an exclusive lock on the directory of the store at
/// `path`, so one process's load-modify-save of the store cannot interleave
/// with another's. The lock is released when `f` returns.
pub fn with_lock<T>(
    path: &Path,
    f: impl FnOnce() -> Result<T, SecretsError>,
) -> Result<T, SecretsError> {
    let dir = path.parent().ok_or_else(|| SecretsError::Io {
        action: "locate the parent directory of",
        path: path.to_path_buf(),
        source: std::io::Error::other("store path has no parent"),
    })?;
    std::fs::create_dir_all(dir).map_err(io_err("create directory", dir))?;
    restrict_dir(dir)?;
    let _lock = lock_dir(dir)?;
    f()
}

/// Lock the directory itself, so no lock file is left beside the store.
#[cfg(unix)]
fn lock_dir(dir: &Path) -> Result<std::fs::File, SecretsError> {
    let handle = std::fs::File::open(dir).map_err(io_err("open", dir))?;
    handle.lock().map_err(io_err("lock", dir))?;
    Ok(handle)
}

/// Windows cannot lock a directory handle, so lock a `.lock` file in the
/// directory instead. The file stays: deleting it would let a waiting process
/// lock the old file while a new one locks a fresh file.
#[cfg(not(unix))]
fn lock_dir(dir: &Path) -> Result<std::fs::File, SecretsError> {
    let path = dir.join(".lock");
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(io_err("open", &path))?;
    handle.lock().map_err(io_err("lock", &path))?;
    Ok(handle)
}

#[cfg(unix)]
fn restrict_dir(dir: &Path) -> Result<(), SecretsError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(io_err("set permissions on", dir))
}

#[cfg(not(unix))]
fn restrict_dir(_dir: &Path) -> Result<(), SecretsError> {
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &str) -> Result<(), SecretsError> {
    use std::io::Write;
    let mut file = error::create_temp_file(path)?;
    file.write_all(contents.as_bytes())
        .map_err(io_err("write", path))?;
    file.sync_all().map_err(io_err("sync", path))
}

// Non-unix platforms have no portable `0600` equivalent here, so the store
// inherits default filesystem ACLs and is NOT permission-protected at rest.
#[cfg(not(unix))]
fn write_private(path: &Path, contents: &str) -> Result<(), SecretsError> {
    std::fs::write(path, contents).map_err(io_err("write", path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_keeps_raw_tokens_and_handles_export_and_comments() {
        let input = "# c\n\nexport A=1 # dev\nB=\"two words\"\nC='x=y'\nD=#act:sequestered\n";
        let store = parse_env(input, Path::new("t.env")).unwrap();
        assert_eq!(store["A"], "1");
        assert_eq!(store["B"], "\"two words\"");
        assert_eq!(store["C"], "'x=y'");
        assert_eq!(store["D"], "#act:sequestered");
    }

    #[test]
    fn parse_env_rejects_malformed_with_path_and_line() {
        let err = parse_env("A=1\njunkline\n", Path::new("t.env")).unwrap_err();
        match err {
            SecretsError::MalformedEnv { path, line, .. } => {
                assert_eq!(path, Path::new("t.env"));
                assert_eq!(line, 2);
            }
            other => panic!("expected MalformedEnv, got {other:?}"),
        }
    }

    #[test]
    fn parse_env_rejects_differing_duplicates_and_allows_identical_ones() {
        let err = parse_env("A=1\nA=2\n", Path::new("t.env")).unwrap_err();
        assert!(matches!(
            err,
            SecretsError::DuplicateKey {
                first: 1,
                second: 2,
                ..
            }
        ));
        let ok = parse_env("A=1\nA=1\n", Path::new("t.env")).unwrap();
        assert_eq!(ok["A"], "1");
    }

    #[test]
    fn parse_env_rejects_invalid_key_names() {
        let err = parse_env("BAD KEY=1\n", Path::new("t.env")).unwrap_err();
        assert!(matches!(err, SecretsError::MalformedEnv { line: 1, .. }));
    }

    #[test]
    fn quote_if_needed_only_when_the_bare_form_would_change_meaning() {
        assert_eq!(quote_if_needed("abc"), "abc");
        assert_eq!(quote_if_needed("abc#def"), "abc#def");
        assert_eq!(quote_if_needed(""), "\"\"");
        assert_eq!(quote_if_needed("a b"), "\"a b\"");
        assert_eq!(quote_if_needed("a #b"), "\"a #b\"");
        assert_eq!(quote_if_needed(" lead"), "\" lead\"");
        assert_eq!(quote_if_needed("has\"dq"), "'has\"dq'");
        assert_eq!(quote_if_needed("has'sq"), "\"has'sq\"");
    }

    #[test]
    fn quote_then_unquote_round_trips_every_single_line_value() {
        for v in [
            "abc",
            "",
            "a b",
            "a #b",
            " x ",
            "has\"dq",
            "has'sq",
            "x=y",
            "#act:sequestered",
        ] {
            assert_eq!(unquote(&quote_if_needed(v)), v, "value {v:?}");
        }
    }

    #[test]
    fn unquote_strips_one_matching_pair_only() {
        assert_eq!(unquote("\"abc\""), "abc");
        assert_eq!(unquote("'abc'"), "abc");
        assert_eq!(unquote("abc"), "abc");
        assert_eq!(unquote("\"abc'"), "\"abc'");
        assert_eq!(unquote("\"\"abc\"\""), "\"abc\"");
    }

    #[test]
    fn save_then_load_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("store.env");
        let mut store = Store::new();
        store.insert("A".into(), "longvalue1".into());
        store.insert("B".into(), "longvalue2".into());
        save_store(&path, &store).unwrap();
        assert_eq!(load_store(&path).unwrap(), store);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = load_store(&tmp.path().join("nope.env")).unwrap();
        assert!(store.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn saved_store_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("store.env");
        let mut store = Store::new();
        store.insert("A".into(), "longvalue1".into());
        save_store(&path, &store).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn save_tightens_permissions_on_an_existing_wide_store() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("store.env");
        std::fs::write(&path, "A=old\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let mut store = Store::new();
        store.insert("A".into(), "newvalue1".into());
        save_store(&path, &store).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "A=newvalue1\n");
    }

    #[cfg(unix)]
    #[test]
    fn save_refuses_a_preexisting_temp_file_and_leaves_it_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("store.env");
        let stray_tmp = tmp
            .path()
            .join(format!(".store.env.tmp-{}", std::process::id()));
        std::fs::write(&stray_tmp, "leftover").unwrap();

        let store = Store::new();
        let err = save_store(&path, &store).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(&stray_tmp.display().to_string()),
            "error must name the exact stray temp path, got: {msg}"
        );
        assert_eq!(std::fs::read_to_string(&stray_tmp).unwrap(), "leftover");
        assert!(!path.exists(), "save must not have completed");
    }

    #[test]
    fn save_leaves_no_temp_file_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("store.env");
        let mut store = Store::new();
        store.insert("A".into(), "longvalue1".into());
        save_store(&path, &store).unwrap();
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(entries, vec!["store.env".to_string()]);
    }
}
