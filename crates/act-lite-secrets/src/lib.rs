#[cfg(windows)]
#[allow(dead_code)] // TEMP: red commit; the next commit wires it in.
mod acl;
pub mod env_edit;
pub mod env_line;
pub mod error;
pub mod paths;
pub mod redact;
pub mod run;
pub mod silhouette;
pub mod store;

pub use error::SecretsError;
use error::{create_temp_file, err_after_temp_created, io_err};

/// What `import` did — counts and key names only; holds no secret value.
#[derive(Debug, PartialEq, Eq)]
pub struct ImportOutcome {
    /// Keys moved into the store and marked; always `sequestered_keys.len()`.
    pub sequestered: usize,
    /// The keys moved into the store and marked, sorted.
    pub sequestered_keys: Vec<String>,
    /// Keys whose line already carried the marker and whose value is in the
    /// store; left alone.
    pub already_sequestered: usize,
    /// Keys whose line carries the marker but whose value the store does not
    /// hold, sorted. Left alone (not refused); the caller warns about each.
    pub missing: Vec<String>,
}

/// What `mv` did — carried to the command layer for reporting; holds no secret value.
#[derive(Debug, PartialEq, Eq)]
pub enum MvOutcome {
    /// Moved `NAME` from the `.env` into the sequestered store.
    Sequestered,
    /// The `.env` line was already the marker — no-op success.
    AlreadySequestered,
    /// Wrote `NAME` back into the `.env` and dropped it from the store.
    Restored,
}

use std::path::{Path, PathBuf};

use store::Store;

/// A project's sequestered secrets plus the paths that anchor them.
pub struct Secrets {
    /// The root the store is keyed on (the main checkout for a linked worktree).
    pub root: PathBuf,
    pub store_path: PathBuf,
    /// The `.env` that `mv` edits: the nearest `.git` ancestor's own `.env`.
    pub env_path: PathBuf,
    store: Store,
}

impl Secrets {
    /// Resolve both roots from `start`, then load the store. The store is keyed
    /// on [`paths::resolve_root`] (shared by every worktree of a repo); the
    /// `.env` is [`paths::env_root`]'s (each worktree edits its own).
    pub fn open(start: &Path) -> Result<Self, SecretsError> {
        Self::open_from(start, &paths::secrets_dir())
    }

    /// [`Secrets::open`] with the store kept under `secrets_dir` (tests).
    fn open_from(start: &Path, secrets_dir: &Path) -> Result<Self, SecretsError> {
        Self::open_with(
            &paths::resolve_root(start),
            &paths::env_root(start),
            secrets_dir,
        )
    }

    /// Use `root` exactly as given (no git walk) for both the store key and
    /// the `.env`; the store lives in the per-user secrets dir. This is the
    /// `--workspace` path.
    pub fn open_root(root: &Path) -> Result<Self, SecretsError> {
        Self::open_in(root, &paths::secrets_dir())
    }

    /// Use `root` as given for both the store key and the `.env`, and keep the
    /// store under `secrets_dir`. Tests use this so nothing is ever written
    /// under `~/.config/act`.
    pub fn open_in(root: &Path, secrets_dir: &Path) -> Result<Self, SecretsError> {
        Self::open_with(root, root, secrets_dir)
    }

    /// Key the store on `store_root`; edit `env_root/.env`.
    fn open_with(
        store_root: &Path,
        env_root: &Path,
        secrets_dir: &Path,
    ) -> Result<Self, SecretsError> {
        let store_path = paths::store_path_in(secrets_dir, store_root);
        let store = store::load_store(&store_path)?;
        Ok(Self {
            root: store_root.to_path_buf(),
            store_path,
            env_path: env_root.join(".env"),
            store,
        })
    }

    pub fn keys(&self) -> Vec<String> {
        self.store.keys().cloned().collect()
    }

    /// The plaintext value (quotes stripped), if stored.
    pub fn get(&self, name: &str) -> Option<String> {
        self.store.get(name).map(|t| store::unquote(t))
    }

    /// Every plaintext value (for redaction).
    pub fn values(&self) -> Vec<String> {
        self.store.values().map(|t| store::unquote(t)).collect()
    }

    /// `(KEY, plaintext)` pairs for injecting into a child environment.
    pub fn env_pairs(&self) -> Vec<(String, String)> {
        self.store
            .iter()
            .map(|(k, t)| (k.clone(), store::unquote(t)))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Add or update one secret in the store. Touches nothing in the repo.
    pub fn add(&mut self, name: &str, value: &str) -> Result<(), SecretsError> {
        store::validate_name(name)?;
        if value.contains('\n') || value.contains('\r') {
            return Err(SecretsError::MultilineValue(name.to_string()));
        }
        if value.contains('"') && value.contains('\'') {
            return Err(SecretsError::UnquotableValue(name.to_string()));
        }
        // Reload under the lock so a concurrent add's key is kept, not overwritten.
        store::with_lock(&self.store_path, || {
            self.store = store::load_store(&self.store_path)?;
            self.store
                .insert(name.to_string(), store::quote_if_needed(value));
            store::save_store(&self.store_path, &self.store)
        })
    }

    /// Remove one secret from the store. Touches nothing in the repo.
    /// Errors if the key is not present.
    pub fn remove(&mut self, name: &str) -> Result<(), SecretsError> {
        // Reload under the lock so a concurrent add's key is kept, not overwritten.
        store::with_lock(&self.store_path, || {
            self.store = store::load_store(&self.store_path)?;
            if self.store.remove(name).is_none() {
                return Err(SecretsError::NotFound(name.to_string()));
            }
            store::save_store(&self.store_path, &self.store)
        })
    }

    /// Sequester every plaintext assignment in `env_path`: move each value
    /// into the store and replace only that line's token with the marker.
    /// Lines already marked are left alone: counted in `already_sequestered`
    /// when the store holds the value, listed in `missing` when it does not.
    /// Every other byte of
    /// the file is preserved; the protocol header is prepended once. Writes
    /// only `env_path` (and the store) — never any other file.
    ///
    /// Refuses (before any write) on a malformed line, a duplicate key with
    /// differing values, or — unless `force` — a key whose stored value
    /// differs from the file's (values are compared unquoted, so `"abc"` and
    /// `abc` agree). A marked line whose key the store lacks is not refused;
    /// it is reported in [`ImportOutcome::missing`]. The header is written with
    /// `\r\n` endings when the file's first line ends in `\r\n`.
    pub fn import(&mut self, env_path: &Path, force: bool) -> Result<ImportOutcome, SecretsError> {
        // Reload the store under the lock so a concurrent add's key is kept,
        // not overwritten.
        let store_path = self.store_path.clone();
        store::with_lock(&store_path, || {
            self.store = store::load_store(&store_path)?;
            self.import_locked(env_path, force)
        })
    }

    /// The body of [`Secrets::import`]; the caller holds the store lock.
    fn import_locked(
        &mut self,
        env_path: &Path,
        force: bool,
    ) -> Result<ImportOutcome, SecretsError> {
        let content = std::fs::read_to_string(env_path).map_err(io_err("read", env_path))?;
        let parsed = store::parse_env(&content, env_path)?;

        let mut incoming: Vec<(&String, &String)> = Vec::new();
        let mut already_sequestered = 0usize;
        let mut missing = Vec::new();
        for (key, token) in &parsed {
            if token == silhouette::SEQUESTERED_VALUE {
                if self.store.contains_key(key) {
                    already_sequestered += 1;
                } else {
                    missing.push(key.clone());
                }
                continue;
            }
            if let Some(existing) = self.store.get(key)
                && store::unquote(existing) != store::unquote(token)
                && !force
            {
                return Err(SecretsError::StoreCollision {
                    key: key.clone(),
                    path: env_path.to_path_buf(),
                });
            }
            incoming.push((key, token));
        }

        // Store first (the value is never nowhere), then mark the lines.
        for (key, token) in &incoming {
            self.store.insert((*key).clone(), (*token).clone());
        }
        store::save_store(&self.store_path, &self.store)?;

        let mut buf = content;
        for (key, _) in &incoming {
            buf = env_edit::set(&buf, key, silhouette::SEQUESTERED_VALUE);
        }
        if !silhouette::has_header(&buf) {
            // Match the file's own line terminator so a CRLF file stays CRLF.
            let crlf = buf
                .split_inclusive('\n')
                .next()
                .is_some_and(|l| l.ends_with("\r\n"));
            let header = if crlf {
                silhouette::SILHOUETTE_HEADER.replace('\n', "\r\n")
            } else {
                silhouette::SILHOUETTE_HEADER.to_string()
            };
            buf = format!("{header}{buf}");
        }
        write_user_file_atomic(env_path, &buf)?;

        let sequestered_keys: Vec<String> = incoming.iter().map(|(k, _)| (*k).clone()).collect();
        Ok(ImportOutcome {
            sequestered: sequestered_keys.len(),
            sequestered_keys,
            already_sequestered,
            missing,
        })
    }

    /// Sequester one key (`restore == false`) or un-sequester it (`restore == true`).
    /// `force` overrides a destination-value collision; values are compared
    /// unquoted, so `"abc"` and `abc` do not collide.
    pub fn mv(
        &mut self,
        name: &str,
        restore: bool,
        force: bool,
    ) -> Result<MvOutcome, SecretsError> {
        store::validate_name(name)?;
        if !self.env_path.exists() {
            return Err(SecretsError::NoEnv(self.env_path.clone()));
        }
        // Read `.env` and reload the store under the lock so a concurrent
        // mv's marker and store entry are kept, not overwritten.
        let store_path = self.store_path.clone();
        store::with_lock(&store_path, || {
            self.store = store::load_store(&store_path)?;
            self.mv_locked(name, restore, force)
        })
    }

    /// The body of [`Secrets::mv`]; the caller holds the store lock.
    fn mv_locked(
        &mut self,
        name: &str,
        restore: bool,
        force: bool,
    ) -> Result<MvOutcome, SecretsError> {
        let buf =
            std::fs::read_to_string(&self.env_path).map_err(io_err("read", &self.env_path))?;
        let state = env_edit::find(&buf, name, &self.env_path)?;

        if restore {
            let stored = self
                .store
                .get(name)
                .cloned()
                .ok_or_else(|| SecretsError::NotFound(name.to_string()))?;
            if let env_edit::LineState::Value(current) = &state
                && store::unquote(current) != store::unquote(&stored)
                && !force
            {
                return Err(SecretsError::EnvCollision {
                    key: name.to_string(),
                    path: self.env_path.clone(),
                });
            }
            // .env first (the value is never nowhere), then drop from the store.
            let new_buf = env_edit::set(&buf, name, &stored);
            write_user_file_atomic(&self.env_path, &new_buf)?;
            self.store.remove(name);
            store::save_store(&self.store_path, &self.store)?;
            return Ok(MvOutcome::Restored);
        }

        let token = match state {
            env_edit::LineState::Absent => {
                return Err(SecretsError::NotInEnv {
                    key: name.to_string(),
                    path: self.env_path.clone(),
                });
            }
            env_edit::LineState::Sequestered => {
                if self.store.contains_key(name) {
                    return Ok(MvOutcome::AlreadySequestered);
                }
                return Err(SecretsError::MarkedButMissing {
                    key: name.to_string(),
                    path: self.env_path.clone(),
                });
            }
            env_edit::LineState::Value(t) => t,
        };
        if let Some(existing) = self.store.get(name)
            && store::unquote(existing) != store::unquote(&token)
            && !force
        {
            return Err(SecretsError::StoreCollision {
                key: name.to_string(),
                path: self.env_path.clone(),
            });
        }
        // Store first (the value is never nowhere), then mark the line.
        self.store.insert(name.to_string(), token);
        store::save_store(&self.store_path, &self.store)?;
        let new_buf = env_edit::set(&buf, name, silhouette::SEQUESTERED_VALUE);
        write_user_file_atomic(&self.env_path, &new_buf)?;
        Ok(MvOutcome::Sequestered)
    }
}

/// Replace a user file (a `.env`) atomically: write `contents` to a temp file
/// in the target's directory, give it the original file's permissions, sync
/// it, then rename it over the target. A crash mid-write leaves the old file
/// intact. When `path` is a symlink, its target is written and the link is
/// kept. On failure the temp file is removed.
fn write_user_file_atomic(path: &Path, contents: &str) -> Result<(), SecretsError> {
    let is_link = path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink());
    let target = if is_link {
        path.canonicalize()
            .map_err(io_err("resolve the symlink", path))?
    } else {
        path.to_path_buf()
    };
    let dir = match target.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("env");
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let permissions = match std::fs::metadata(&target) {
        Ok(meta) => Some(meta.permissions()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io_err("read the permissions of", &target)(e)),
    };

    // `create_temp_file` never deletes or overwrites a file it did not
    // create, so a pre-existing (stale) temp file is reported without being
    // touched, rather than removed by the cleanup below.
    let mut file = create_temp_file(&tmp)?;
    if let Err((action, cleanup_failed_action, source)) =
        replace_via_temp(&mut file, &tmp, &target, contents, permissions)
    {
        return Err(err_after_temp_created(
            &tmp,
            &target,
            action,
            cleanup_failed_action,
            source,
        ));
    }
    Ok(())
}

/// The fallible steps of [`write_user_file_atomic`] that run after the temp
/// file has already been created. Each returns the action to report on an
/// ordinary failure, the action to report if cleaning up the temp file also
/// fails, and the underlying error.
fn replace_via_temp(
    file: &mut std::fs::File,
    tmp: &Path,
    target: &Path,
    contents: &str,
    permissions: Option<std::fs::Permissions>,
) -> Result<(), (&'static str, &'static str, std::io::Error)> {
    use std::io::Write;
    file.write_all(contents.as_bytes()).map_err(|e| {
        (
            "write a temp file beside",
            "write a temp file (its leftover copy also could not be removed; delete it \
             manually) at",
            e,
        )
    })?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions).map_err(|e| {
            (
                "copy permissions onto a temp file beside",
                "copy permissions onto a temp file (its leftover copy also could not be \
                 removed; delete it manually) at",
                e,
            )
        })?;
    }
    file.sync_all().map_err(|e| {
        (
            "sync a temp file beside",
            "sync a temp file (its leftover copy also could not be removed; delete it \
             manually) at",
            e,
        )
    })?;
    std::fs::rename(tmp, target).map_err(|e| {
        (
            "replace",
            "replace (the temp file also could not be removed; delete it manually) at",
            e,
        )
    })
}

#[cfg(test)]
mod secrets_tests {
    use super::*;

    #[test]
    fn open_in_places_store_under_the_given_secrets_dir_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let root = root.canonicalize().unwrap();
        let secrets_dir = tmp.path().join("secrets");

        let mut s = Secrets::open_in(&root, &secrets_dir).unwrap();
        s.add("K", "longvalue1").unwrap();

        assert!(s.store_path.starts_with(&secrets_dir));
        assert!(s.store_path.exists());
        // Nothing appeared under the real per-user secrets dir for this root.
        assert!(!paths::store_path_in(&paths::secrets_dir(), &root).exists());
    }

    /// One tempdir holding a git-rooted project (`proj/`) and a private
    /// secrets dir (`secrets/`). Returns (tempdir, root, secrets).
    fn sandbox() -> (tempfile::TempDir, PathBuf, Secrets) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let root = root.canonicalize().unwrap();
        let secrets = Secrets::open_in(&root, &tmp.path().join("secrets")).unwrap();
        (tmp, root, secrets)
    }

    /// Re-open the same sandbox from disk.
    fn reopen(tmp: &tempfile::TempDir, root: &Path) -> Secrets {
        Secrets::open_in(root, &tmp.path().join("secrets")).unwrap()
    }

    #[test]
    fn open_in_a_worktree_edits_the_worktree_env_and_shares_the_main_store() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let main = base.join("main");
        std::fs::create_dir_all(main.join(".git").join("worktrees").join("wt1")).unwrap();
        let wt = base.join("wt1");
        std::fs::create_dir_all(wt.join("src")).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!(
                "gitdir: {}\n",
                main.join(".git").join("worktrees").join("wt1").display()
            ),
        )
        .unwrap();
        std::fs::write(main.join(".env"), "WT_KEY=mainvalue1\n").unwrap();
        std::fs::write(wt.join(".env"), "WT_KEY=worktreevalue1\n").unwrap();
        let secrets_dir = base.join("secrets");

        let mut s = Secrets::open_from(&wt.join("src"), &secrets_dir).unwrap();
        assert_eq!(s.env_path, wt.join(".env"));
        assert_eq!(s.store_path, paths::store_path_in(&secrets_dir, &main));

        s.mv("WT_KEY", false, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "WT_KEY=#act:sequestered\n"
        );
        assert_eq!(
            std::fs::read_to_string(main.join(".env")).unwrap(),
            "WT_KEY=mainvalue1\n"
        );
        // The main checkout sees the value through the shared store.
        let from_main = Secrets::open_from(&main, &secrets_dir).unwrap();
        assert_eq!(from_main.get("WT_KEY").as_deref(), Some("worktreevalue1"));
        assert_eq!(from_main.env_path, main.join(".env"));
    }

    #[test]
    fn open_empty_project_has_no_keys() {
        let (tmp, root, secrets) = sandbox();
        assert!(secrets.is_empty());
        assert_eq!(secrets.env_path, root.join(".env"));
        assert!(secrets.store_path.starts_with(tmp.path().join("secrets")));
    }

    #[test]
    fn add_writes_the_store_and_creates_no_env_file() {
        let (tmp, root, mut secrets) = sandbox();
        secrets.add("GH_TOKEN", "supersecretvalue").unwrap();

        assert_eq!(
            reopen(&tmp, &root).get("GH_TOKEN").as_deref(),
            Some("supersecretvalue")
        );
        assert!(
            !root.join(".env").exists(),
            "add must not create a repo .env"
        );
    }

    #[test]
    fn add_and_rm_leave_a_mixed_env_byte_identical() {
        let mixed =
            "# app\nDATABASE_URL=postgres://x\nexport PORT=8080 # dev\nGH_TOKEN=#act:sequestered\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(mixed);

        secrets.add("NEW_KEY", "anothersecret").unwrap();
        secrets.add("GH_TOKEN", "ghp_realvalue").unwrap();
        secrets.remove("NEW_KEY").unwrap();

        assert_eq!(std::fs::read_to_string(root.join(".env")).unwrap(), mixed);
        let again = reopen(&tmp, &root);
        assert_eq!(again.get("GH_TOKEN").as_deref(), Some("ghp_realvalue"));
        assert!(again.get("NEW_KEY").is_none());
    }

    #[test]
    fn remove_deletes_key_and_errors_when_missing() {
        let (tmp, root, mut secrets) = sandbox();
        secrets.add("A", "longvalue1").unwrap();
        secrets.remove("A").unwrap();
        assert!(reopen(&tmp, &root).get("A").is_none());

        let err = secrets.remove("A").unwrap_err();
        assert!(matches!(err, SecretsError::NotFound(_)));
    }

    #[test]
    fn add_rejects_invalid_names() {
        let (tmp, root, mut secrets) = sandbox();
        // Each name either fails to parse as a `.env` key or reads back as a
        // different key / a comment, so the stored secret would be lost.
        for bad in ["bad name", "", "A=B", "#TOK", "export X", " LEAD", "TAB\t"] {
            assert!(
                matches!(
                    secrets.add(bad, "longvalue1").unwrap_err(),
                    SecretsError::InvalidName(_)
                ),
                "name {bad:?} must be rejected"
            );
        }
        assert!(
            !secrets.store_path.exists(),
            "a rejected add writes nothing"
        );

        // Valid names round-trip, including one that is the bare word `export`.
        for good in ["TOK", "export", "a.b-c", "T#1"] {
            secrets.add(good, "longvalue1").unwrap();
        }
        let again = reopen(&tmp, &root);
        assert_eq!(again.keys(), vec!["T#1", "TOK", "a.b-c", "export"]);
    }

    #[test]
    fn add_rejects_multiline_value() {
        let (_tmp, _root, mut secrets) = sandbox();
        assert!(matches!(
            secrets.add("KEY", "line1\nline2").unwrap_err(),
            SecretsError::MultilineValue(_)
        ));
    }

    #[test]
    fn add_stores_a_token_that_reloads_to_the_exact_value() {
        let (tmp, root, mut secrets) = sandbox();
        for (k, v) in [
            ("A", " x "),
            ("B", "a #b"),
            ("C", "\"quoted\""),
            ("D", "plain"),
        ] {
            secrets.add(k, v).unwrap();
        }
        let again = reopen(&tmp, &root);
        assert_eq!(again.get("A").as_deref(), Some(" x "));
        assert_eq!(again.get("B").as_deref(), Some("a #b"));
        assert_eq!(again.get("C").as_deref(), Some("\"quoted\""));
        assert_eq!(again.get("D").as_deref(), Some("plain"));
        let pairs = again.env_pairs();
        assert!(pairs.contains(&("A".to_string(), " x ".to_string())));
    }

    #[test]
    fn add_rejects_a_value_with_both_quote_kinds_and_writes_nothing() {
        let (tmp, root, mut secrets) = sandbox();

        let err = secrets.add("K", "a\"b'c").unwrap_err();
        assert!(matches!(err, SecretsError::UnquotableValue(_)));
        assert!(!secrets.store_path.exists());

        // A subsequent valid add is unaffected and survives a reopen.
        secrets.add("OK", "plain").unwrap();
        let again = reopen(&tmp, &root);
        assert_eq!(again.get("OK").as_deref(), Some("plain"));
    }

    #[test]
    fn import_rejects_multiline_value_without_data_loss() {
        let (tmp, root, mut secrets) = sandbox();
        let env = root.join(".env");
        let original = "PRIVATE_KEY=\"-----BEGIN KEY-----\nMIIBVAIBADANBgk\n-----END KEY-----\"\n";
        std::fs::write(&env, original).unwrap();

        let err = secrets.import(&env, false).unwrap_err();
        assert!(matches!(err, SecretsError::MalformedEnv { .. }));

        // Source preserved verbatim — not truncated, not silhouetted.
        assert_eq!(std::fs::read_to_string(&env).unwrap(), original);
        assert!(reopen(&tmp, &root).is_empty());
    }

    // ── mv tests ──────────────────────────────────────────────────────────────

    fn make_secrets_with_env(content: &str) -> (tempfile::TempDir, PathBuf, Secrets) {
        let (tmp, root, secrets) = sandbox();
        std::fs::write(root.join(".env"), content).unwrap();
        (tmp, root, secrets)
    }

    #[test]
    fn mv_sequester_happy_path() {
        let env_content =
            "# app keys\nAPP_KEY=cfkey_abc123\n# blank below\n\nADMIN_KEY=adminpass99\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);

        let outcome = secrets.mv("ADMIN_KEY", false, false).unwrap();
        assert_eq!(outcome, MvOutcome::Sequestered);

        // The target line is replaced with the marker.
        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert!(after.contains("ADMIN_KEY=#act:sequestered"));
        // Every other line is byte-identical.
        let expected =
            "# app keys\nAPP_KEY=cfkey_abc123\n# blank below\n\nADMIN_KEY=#act:sequestered\n";
        assert_eq!(after, expected);

        // Re-open: store now holds the real value.
        let reopened = reopen(&tmp, &root);
        assert_eq!(reopened.get("ADMIN_KEY").as_deref(), Some("adminpass99"));

        // Store file must be mode 0600 on unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&secrets.store_path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn mv_restore_happy_path_marker_line_present() {
        // Start with a silhouette-style .env (marker already there) and a pre-populated store.
        let env_content = "APP_KEY=cfkey_abc123\nADMIN_KEY=#act:sequestered\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);
        // Manually prime the store so the key is in it.
        secrets
            .store
            .insert("ADMIN_KEY".to_string(), "adminpass99".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();

        let outcome = secrets.mv("ADMIN_KEY", true, false).unwrap();
        assert_eq!(outcome, MvOutcome::Restored);

        // .env line becomes real value; other lines byte-identical.
        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert_eq!(after, "APP_KEY=cfkey_abc123\nADMIN_KEY=adminpass99\n");

        // Key is no longer in the store.
        let reopened = reopen(&tmp, &root);
        assert!(reopened.get("ADMIN_KEY").is_none());
    }

    #[test]
    fn mv_restore_appends_when_line_absent() {
        // .env has no line for SECRET_KEY; store has the value.
        let env_content = "APP_KEY=cfkey_abc123\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);
        secrets
            .store
            .insert("SECRET_KEY".to_string(), "secretval99".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();

        let outcome = secrets.mv("SECRET_KEY", true, false).unwrap();
        assert_eq!(outcome, MvOutcome::Restored);

        // Pre-existing APP_KEY line byte-identical; SECRET_KEY appended.
        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert_eq!(after, "APP_KEY=cfkey_abc123\nSECRET_KEY=secretval99\n");

        let reopened = reopen(&tmp, &root);
        assert!(reopened.get("SECRET_KEY").is_none());
    }

    #[test]
    fn mv_round_trip_byte_identity() {
        let original = "DATABASE_URL=postgres://x\nGH_TOKEN=ghp_abc123\n";
        let (_tmp, root, mut secrets) = make_secrets_with_env(original);

        secrets.mv("GH_TOKEN", false, false).unwrap();
        secrets.mv("GH_TOKEN", true, false).unwrap();

        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert_eq!(after, original);
    }

    #[test]
    fn mv_on_marker_line_with_store_value_is_a_noop() {
        let (tmp, root, mut secrets) = make_secrets_with_env("APP=x\nADMIN_KEY=#act:sequestered\n");
        secrets.add("ADMIN_KEY", "adminpass99").unwrap();
        let before = std::fs::read_to_string(root.join(".env")).unwrap();

        assert_eq!(
            secrets.mv("ADMIN_KEY", false, false).unwrap(),
            MvOutcome::AlreadySequestered
        );
        assert_eq!(std::fs::read_to_string(root.join(".env")).unwrap(), before);
        assert_eq!(
            reopen(&tmp, &root).get("ADMIN_KEY").as_deref(),
            Some("adminpass99")
        );
    }

    #[test]
    fn mv_on_marker_line_without_store_value_errors() {
        let (_tmp, _root, mut secrets) = make_secrets_with_env("ADMIN_KEY=#act:sequestered\n");
        let err = secrets.mv("ADMIN_KEY", false, false).unwrap_err();
        assert!(matches!(err, SecretsError::MarkedButMissing { .. }));
    }

    #[test]
    fn mv_round_trip_is_byte_identical_for_quoted_export_and_commented_lines() {
        let original = "# head\nexport DATABASE_URL=\"postgres://u:p@h/db?x=1\" # prod\n  TOKEN='a b #c'\r\nPLAIN=v\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(original);

        secrets.mv("DATABASE_URL", false, false).unwrap();
        secrets.mv("TOKEN", false, false).unwrap();
        let mid = std::fs::read_to_string(root.join(".env")).unwrap();
        assert_eq!(
            mid,
            "# head\nexport DATABASE_URL=#act:sequestered # prod\n  TOKEN=#act:sequestered\r\nPLAIN=v\n"
        );
        assert_eq!(
            reopen(&tmp, &root).get("DATABASE_URL").as_deref(),
            Some("postgres://u:p@h/db?x=1")
        );
        assert_eq!(reopen(&tmp, &root).get("TOKEN").as_deref(), Some("a b #c"));

        secrets.mv("TOKEN", true, false).unwrap();
        secrets.mv("DATABASE_URL", true, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(".env")).unwrap(),
            original
        );
    }

    #[test]
    fn mv_refuses_differing_duplicate_lines_and_sequesters_identical_ones() {
        let (_tmp, _root, mut secrets) = make_secrets_with_env("K=a\nK=b\n");
        assert!(matches!(
            secrets.mv("K", false, false).unwrap_err(),
            SecretsError::DuplicateKey { .. }
        ));

        let (tmp, root, mut secrets) = make_secrets_with_env("K=same1\nK=same1\n");
        secrets.mv("K", false, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(".env")).unwrap(),
            "K=#act:sequestered\nK=#act:sequestered\n"
        );
        assert_eq!(reopen(&tmp, &root).get("K").as_deref(), Some("same1"));
    }

    #[test]
    fn mv_sequester_collision_different_value() {
        let env_content = "ADMIN_KEY=newvalue99\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);
        // Pre-populate store with a DIFFERENT value.
        secrets
            .store
            .insert("ADMIN_KEY".to_string(), "oldvalue99".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();
        let store_before = std::fs::read_to_string(&secrets.store_path).unwrap();
        let env_before = std::fs::read_to_string(root.join(".env")).unwrap();

        // Without force → collision error.
        let err = secrets.mv("ADMIN_KEY", false, false).unwrap_err();
        match &err {
            SecretsError::StoreCollision { key, path } => {
                assert_eq!(key, "ADMIN_KEY");
                assert_eq!(path, &root.join(".env"));
            }
            other => panic!("expected StoreCollision, got {other:?}"),
        }
        assert_eq!(
            err.to_string(),
            format!(
                "`ADMIN_KEY` in {} differs from the value already in the sequestered store; \
                 sequestering it would overwrite the stored value. Re-run with --force to replace it.",
                root.join(".env").display()
            )
        );

        // Nothing was written.
        assert_eq!(
            std::fs::read_to_string(root.join(".env")).unwrap(),
            env_before
        );
        assert_eq!(
            std::fs::read_to_string(&secrets.store_path).unwrap(),
            store_before
        );

        // With force → succeeds; store holds the .env's value.
        let outcome = secrets.mv("ADMIN_KEY", false, true).unwrap();
        assert_eq!(outcome, MvOutcome::Sequestered);
        let reopened = reopen(&tmp, &root);
        assert_eq!(reopened.get("ADMIN_KEY").as_deref(), Some("newvalue99"));
    }

    #[test]
    fn mv_sequester_idempotent_same_value() {
        let env_content = "ADMIN_KEY=sharedval99\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);
        // Store already has the exact same value.
        secrets
            .store
            .insert("ADMIN_KEY".to_string(), "sharedval99".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();

        // Should succeed without --force.
        let outcome = secrets.mv("ADMIN_KEY", false, false).unwrap();
        assert_eq!(outcome, MvOutcome::Sequestered);

        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert!(after.contains("ADMIN_KEY=#act:sequestered"));
        let reopened = reopen(&tmp, &root);
        assert_eq!(reopened.get("ADMIN_KEY").as_deref(), Some("sharedval99"));
    }

    #[test]
    fn mv_restore_collision_different_value() {
        let env_content = "ADMIN_KEY=envvalue99\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);
        secrets
            .store
            .insert("ADMIN_KEY".to_string(), "storevalue9".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();
        let env_before = std::fs::read_to_string(root.join(".env")).unwrap();
        let store_before = std::fs::read_to_string(&secrets.store_path).unwrap();

        // Without force → collision error.
        let err = secrets.mv("ADMIN_KEY", true, false).unwrap_err();
        match &err {
            SecretsError::EnvCollision { key, path } => {
                assert_eq!(key, "ADMIN_KEY");
                assert_eq!(path, &root.join(".env"));
            }
            other => panic!("expected EnvCollision, got {other:?}"),
        }
        assert_eq!(
            err.to_string(),
            format!(
                "`ADMIN_KEY` in {} differs from the value in the sequestered store; restoring \
                 would overwrite the .env value. Re-run with --force to replace it.",
                root.join(".env").display()
            )
        );

        // Nothing written.
        assert_eq!(
            std::fs::read_to_string(root.join(".env")).unwrap(),
            env_before
        );
        assert_eq!(
            std::fs::read_to_string(&secrets.store_path).unwrap(),
            store_before
        );

        // With force → succeeds; .env holds the store's value.
        let outcome = secrets.mv("ADMIN_KEY", true, true).unwrap();
        assert_eq!(outcome, MvOutcome::Restored);
        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert!(after.contains("ADMIN_KEY=storevalue9"));
        // Dropped from store.
        let reopened = reopen(&tmp, &root);
        assert!(reopened.get("ADMIN_KEY").is_none());
    }

    #[test]
    fn mv_restore_idempotent_same_value() {
        let env_content = "APP_KEY=cfkey_abc123\nADMIN_KEY=samevalue9\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(env_content);
        secrets
            .store
            .insert("ADMIN_KEY".to_string(), "samevalue9".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();

        let outcome = secrets.mv("ADMIN_KEY", true, false).unwrap();
        assert_eq!(outcome, MvOutcome::Restored);

        // Dropped from store.
        let reopened = reopen(&tmp, &root);
        assert!(reopened.get("ADMIN_KEY").is_none());
        // .env is byte-identical: ADMIN_KEY already held the value, APP_KEY untouched.
        let after = std::fs::read_to_string(root.join(".env")).unwrap();
        assert_eq!(after, "APP_KEY=cfkey_abc123\nADMIN_KEY=samevalue9\n");
    }

    #[test]
    fn mv_edge_errors() {
        // NotInEnv: key absent from .env
        {
            let env_content = "OTHER_KEY=cfkey_abc123\n";
            let (_tmp, _root, mut secrets) = make_secrets_with_env(env_content);
            let err = secrets.mv("ABSENT_KEY", false, false).unwrap_err();
            assert!(matches!(err, SecretsError::NotInEnv { .. }));
        }
        // NoEnv: no .env file at all (sequester path)
        {
            let (_tmp, _root, mut secrets) = sandbox();
            let err = secrets.mv("SOME_KEY", false, false).unwrap_err();
            assert!(matches!(err, SecretsError::NoEnv(_)));
        }
        // NotFound: key not in store (restore path)
        {
            let env_content = "OTHER_KEY=cfkey_abc123\n";
            let (_tmp, _root, mut secrets) = make_secrets_with_env(env_content);
            let err = secrets.mv("MISSING_KEY", true, false).unwrap_err();
            assert!(matches!(err, SecretsError::NotFound(_)));
        }
        // NoEnv: key in store but no .env file at all (restore path)
        {
            let (_tmp, _root, mut secrets) = sandbox();
            secrets
                .store
                .insert("SOME_KEY".to_string(), "somevalue99".to_string());
            store::save_store(&secrets.store_path, &secrets.store).unwrap();
            let err = secrets.mv("SOME_KEY", true, false).unwrap_err();
            assert!(matches!(err, SecretsError::NoEnv(_)));
        }
    }

    #[test]
    fn mv_restore_no_env_file() {
        // Key present in the store, but no `.env` on disk → restore must error NoEnv,
        // and must NOT drop the key from the store.
        let (tmp, root, mut secrets) = sandbox();
        secrets
            .store
            .insert("ADMIN_KEY".to_string(), "adminpass99".to_string());
        store::save_store(&secrets.store_path, &secrets.store).unwrap();

        let err = secrets.mv("ADMIN_KEY", true, false).unwrap_err();
        assert!(matches!(err, SecretsError::NoEnv(_)));

        // Key is still in the store — nothing was dropped.
        assert_eq!(
            reopen(&tmp, &root).get("ADMIN_KEY").as_deref(),
            Some("adminpass99")
        );
    }

    /// Every entry name in `dir`, sorted.
    #[cfg(unix)]
    fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// Every `.env` write replaces the file by rename (a new inode) rather than
    /// truncating it in place, so a crash mid-write cannot lose plaintext that
    /// was never in the store.
    #[cfg(unix)]
    #[test]
    fn env_writes_replace_the_file_instead_of_truncating_it() {
        use std::os::unix::fs::MetadataExt;
        let (_tmp, root, mut secrets) = make_secrets_with_env("A=avalue123\nB=bvalue123\n");
        let env = root.join(".env");
        let ino = |p: &Path| std::fs::metadata(p).unwrap().ino();

        let i0 = ino(&env);
        secrets.mv("A", false, false).unwrap();
        let i1 = ino(&env);
        assert_ne!(i0, i1, "mv sequester truncated .env in place");
        secrets.mv("A", true, false).unwrap();
        let i2 = ino(&env);
        assert_ne!(i1, i2, "mv --restore truncated .env in place");
        secrets.import(&env, false).unwrap();
        assert_ne!(i2, ino(&env), "import truncated .env in place");
    }

    #[cfg(unix)]
    #[test]
    fn env_writes_keep_the_file_mode_and_leave_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, root, mut secrets) = make_secrets_with_env("A=avalue123\nB=bvalue123\n");
        let env = root.join(".env");
        std::fs::set_permissions(&env, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = dir_entries(&root);

        secrets.mv("A", false, false).unwrap();
        assert_eq!(
            std::fs::metadata(&env).unwrap().permissions().mode() & 0o777,
            0o640
        );
        secrets.mv("A", true, false).unwrap();
        assert_eq!(
            std::fs::metadata(&env).unwrap().permissions().mode() & 0o777,
            0o640
        );
        secrets.import(&env, false).unwrap();
        assert_eq!(
            std::fs::metadata(&env).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(dir_entries(&root), before, "no temp file may remain");
    }

    #[test]
    fn write_user_file_atomic_refuses_a_preexisting_temp_file_and_leaves_it_in_place() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let target = tmp_dir.path().join("secrets.env");
        std::fs::write(&target, "A=1\n").unwrap();
        let stray_tmp = tmp_dir
            .path()
            .join(format!(".secrets.env.tmp-{}", std::process::id()));
        std::fs::write(&stray_tmp, "leftover").unwrap();

        let err = write_user_file_atomic(&target, "A=2\n").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(&stray_tmp.display().to_string()),
            "error must name the exact stray temp path, got: {msg}"
        );
        assert_eq!(std::fs::read_to_string(&stray_tmp).unwrap(), "leftover");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "A=1\n");
    }

    #[cfg(unix)]
    #[test]
    fn env_writes_through_a_symlink_edit_the_target_and_keep_the_link() {
        let (tmp, root, mut secrets) = sandbox();
        let shared = tmp.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let target = shared.join("real.env");
        std::fs::write(&target, "A=avalue123\n").unwrap();
        let env = root.join(".env");
        std::os::unix::fs::symlink(&target, &env).unwrap();

        secrets.mv("A", false, false).unwrap();
        assert!(env.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "A=#act:sequestered\n"
        );

        secrets.mv("A", true, false).unwrap();
        assert!(env.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "A=avalue123\n");

        secrets.import(&env, false).unwrap();
        assert!(env.symlink_metadata().unwrap().file_type().is_symlink());
        assert!(
            std::fs::read_to_string(&target)
                .unwrap()
                .contains("A=#act:sequestered")
        );
        assert_eq!(dir_entries(&shared), vec!["real.env".to_string()]);
        assert_eq!(
            dir_entries(&root),
            vec![".env".to_string(), ".git".to_string()]
        );
    }

    #[test]
    fn import_marks_plaintext_lines_in_place_and_prepends_the_header_once() {
        let original = "# app\nDATABASE_URL=postgres://x # local\nexport GH_TOKEN=\"ghp_abc\"\n";
        let (tmp, root, mut secrets) = make_secrets_with_env(original);
        let env = root.join(".env");

        let out = secrets.import(&env, false).unwrap();
        assert_eq!((out.sequestered, out.already_sequestered), (2, 0));

        let after = std::fs::read_to_string(&env).unwrap();
        let expected_body =
            "# app\nDATABASE_URL=#act:sequestered # local\nexport GH_TOKEN=#act:sequestered\n";
        assert_eq!(
            after,
            format!("{}{}", silhouette::SILHOUETTE_HEADER, expected_body)
        );
        assert!(!after.contains("ghp_abc"));

        let again = reopen(&tmp, &root);
        assert_eq!(again.get("GH_TOKEN").as_deref(), Some("ghp_abc"));
        assert_eq!(again.get("DATABASE_URL").as_deref(), Some("postgres://x"));

        // Second import: nothing to do, header not duplicated.
        let out2 = secrets.import(&env, false).unwrap();
        assert_eq!((out2.sequestered, out2.already_sequestered), (0, 2));
        assert_eq!(std::fs::read_to_string(&env).unwrap(), after);
    }

    #[test]
    fn import_of_a_mixed_file_never_stores_the_marker() {
        let (tmp, root, mut secrets) = make_secrets_with_env("A=#act:sequestered\nB=realvalue1\n");
        secrets.add("A", "storedA1").unwrap();

        secrets.import(&root.join(".env"), false).unwrap();

        let again = reopen(&tmp, &root);
        assert_eq!(again.get("A").as_deref(), Some("storedA1"));
        assert_eq!(again.get("B").as_deref(), Some("realvalue1"));
        assert!(
            again
                .values()
                .iter()
                .all(|v| v != silhouette::SEQUESTERED_VALUE)
        );
    }

    #[test]
    fn import_writes_only_the_file_it_was_given() {
        let (tmp, root, mut secrets) = make_secrets_with_env("REPO=keepme1\n");
        let other = root.join("deploy.env");
        std::fs::write(&other, "DEPLOY_KEY=dk_1234567\n").unwrap();

        secrets.import(&other, false).unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join(".env")).unwrap(),
            "REPO=keepme1\n"
        );
        assert!(
            std::fs::read_to_string(&other)
                .unwrap()
                .contains("DEPLOY_KEY=#act:sequestered")
        );
        assert_eq!(
            reopen(&tmp, &root).get("DEPLOY_KEY").as_deref(),
            Some("dk_1234567")
        );

        // No repo .env is created when none existed.
        let (_tmp2, root2, mut s2) = sandbox();
        let f = root2.join("x.env");
        std::fs::write(&f, "K=v1234567\n").unwrap();
        s2.import(&f, false).unwrap();
        assert!(!root2.join(".env").exists());
    }

    #[test]
    fn import_refuses_to_overwrite_a_different_stored_value_without_force() {
        let (tmp, root, mut secrets) = make_secrets_with_env("K=envvalue1\n");
        secrets.add("K", "storevalue1").unwrap();
        let env = root.join(".env");

        let err = secrets.import(&env, false).unwrap_err();
        assert!(matches!(
            &err,
            SecretsError::StoreCollision { key, path } if key == "K" && path == &env
        ));
        assert_eq!(std::fs::read_to_string(&env).unwrap(), "K=envvalue1\n");
        assert_eq!(reopen(&tmp, &root).get("K").as_deref(), Some("storevalue1"));

        secrets.import(&env, true).unwrap();
        assert_eq!(reopen(&tmp, &root).get("K").as_deref(), Some("envvalue1"));
    }

    #[test]
    fn collisions_compare_values_not_quoting() {
        // import: the store holds `samevalue1`, the file quotes the same value.
        let (tmp, root, mut secrets) = make_secrets_with_env("Q=\"samevalue1\"\n");
        secrets.add("Q", "samevalue1").unwrap();
        secrets.import(&root.join(".env"), false).unwrap();
        assert_eq!(reopen(&tmp, &root).get("Q").as_deref(), Some("samevalue1"));

        // mv sequester: same value, different quoting.
        let (_tmp, _root, mut secrets) = make_secrets_with_env("Q='samevalue1'\n");
        secrets.add("Q", "samevalue1").unwrap();
        assert_eq!(
            secrets.mv("Q", false, false).unwrap(),
            MvOutcome::Sequestered
        );

        // mv --restore: the .env holds the quoted form of the stored value.
        let (_tmp, root, mut secrets) = make_secrets_with_env("Q=\"samevalue1\"\n");
        secrets.add("Q", "samevalue1").unwrap();
        assert_eq!(secrets.mv("Q", true, false).unwrap(), MvOutcome::Restored);
        assert!(
            std::fs::read_to_string(root.join(".env"))
                .unwrap()
                .contains("samevalue1")
        );
    }

    #[test]
    fn import_reports_marker_lines_the_store_has_no_value_for() {
        let (_tmp, root, mut secrets) =
            make_secrets_with_env("HAVE=#act:sequestered\nGONE=#act:sequestered\nNEW=newvalue1\n");
        secrets.add("HAVE", "havevalue1").unwrap();

        let out = secrets.import(&root.join(".env"), false).unwrap();
        assert_eq!(out.sequestered, 1);
        assert_eq!(out.sequestered_keys, vec!["NEW".to_string()]);
        assert_eq!(out.already_sequestered, 1);
        assert_eq!(out.missing, vec!["GONE".to_string()]);
        // Not refused: the plaintext line was still sequestered.
        assert!(
            std::fs::read_to_string(root.join(".env"))
                .unwrap()
                .contains("NEW=#act:sequestered")
        );
    }

    #[test]
    fn import_lists_the_keys_it_sequestered() {
        let (_tmp, root, mut secrets) = make_secrets_with_env("B=bvalue123\nA=avalue123\n");
        let out = secrets.import(&root.join(".env"), false).unwrap();
        assert_eq!(out.sequestered_keys, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(out.sequestered, out.sequestered_keys.len());
        assert!(out.missing.is_empty());
    }

    #[test]
    fn import_writes_the_header_with_crlf_endings_on_a_crlf_file() {
        let (_tmp, root, mut secrets) = make_secrets_with_env("A=avalue123\r\nB=bvalue123\r\n");
        let env = root.join(".env");
        secrets.import(&env, false).unwrap();

        let after = std::fs::read_to_string(&env).unwrap();
        let header_crlf = silhouette::SILHOUETTE_HEADER.replace('\n', "\r\n");
        assert_eq!(
            after,
            format!("{header_crlf}A=#act:sequestered\r\nB=#act:sequestered\r\n")
        );
        assert!(!after.replace("\r\n", "").contains('\n'), "no bare LF");
        // Header is detected on the next import and not duplicated.
        secrets.import(&env, false).unwrap();
        assert_eq!(std::fs::read_to_string(&env).unwrap(), after);
    }

    #[test]
    fn import_rejects_bad_key_names_before_writing_anything() {
        let (tmp, root, mut secrets) = make_secrets_with_env("BAD KEY=v1234567\nGOOD=v1234567\n");
        let err = secrets.import(&root.join(".env"), false).unwrap_err();
        assert!(matches!(err, SecretsError::MalformedEnv { line: 1, .. }));
        assert!(reopen(&tmp, &root).is_empty());
    }
}
