use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// act's per-user home directory (`~/.config/act`), shared with act.
///
/// Precedence:
/// 1. `ACT_CONFIG_DIR` — used verbatim (explicit override / test isolation).
/// 2. non-Windows: `$XDG_CONFIG_HOME/act`, else `$HOME/.config/act`.
/// 3. Windows: `%APPDATA%\act` (via `dirs::config_dir()`).
pub fn act_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ACT_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    platform_default()
}

#[cfg(windows)]
fn platform_default() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("act")
}

#[cfg(not(windows))]
fn platform_default() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("act");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("act")
}

/// Directory holding per-project sequestered stores (`~/.config/act/secrets`).
pub fn secrets_dir() -> PathBuf {
    act_dir().join("secrets")
}

/// Resolve the root the store is keyed on: the nearest ancestor of `start`
/// containing `.git`. When `.git` is a **file** (a linked worktree or a
/// submodule), a `gitdir:` pointing into `<main>/.git/worktrees/<name>`
/// resolves to `<main>` so every worktree of a repo shares one store; any
/// other `.git` file (a submodule) makes that directory its own root. With no
/// `.git` anywhere, `start` itself (canonicalized) is the root. This root
/// picks the store only — the `.env` edited is each worktree's own (see
/// [`env_root`]).
pub fn resolve_root(start: &Path) -> PathBuf {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    for ancestor in start.ancestors() {
        let dot_git = ancestor.join(".git");
        if dot_git.is_dir() {
            return ancestor.to_path_buf();
        }
        if dot_git.is_file() {
            return worktree_main_root(&dot_git).unwrap_or_else(|| ancestor.to_path_buf());
        }
    }
    start
}

/// The directory whose `.env` act edits: the nearest ancestor of `start`
/// containing `.git` (a directory, or a file in a linked worktree or
/// submodule), or `start` itself (canonicalized) with no `.git` anywhere.
/// Unlike [`resolve_root`], a linked worktree is NOT mapped to its main
/// checkout: the store is shared across worktrees, but each worktree's own
/// `.env` is the one edited.
pub fn env_root(start: &Path) -> PathBuf {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    start
        .ancestors()
        .find(|a| a.join(".git").exists())
        .map_or_else(|| start.clone(), Path::to_path_buf)
}

/// For a `.git` file whose `gitdir:` lies under `<main>/.git/worktrees/`,
/// return `<main>`. None for any other content (e.g. a submodule's gitdir).
fn worktree_main_root(dot_git_file: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(dot_git_file).ok()?;
    let gitdir = content.lines().next()?.strip_prefix("gitdir:")?.trim();
    let gitdir = Path::new(gitdir);
    let gitdir = if gitdir.is_absolute() {
        gitdir.to_path_buf()
    } else {
        dot_git_file.parent()?.join(gitdir)
    };
    let gitdir = gitdir.canonicalize().unwrap_or(gitdir);
    // …/<main>/.git/worktrees/<name>  →  <main>
    let name_dir = gitdir;
    let worktrees = name_dir.parent()?;
    if worktrees.file_name()? != "worktrees" {
        return None;
    }
    let git_dir = worktrees.parent()?;
    if git_dir.file_name()? != ".git" {
        return None;
    }
    git_dir.parent().map(Path::to_path_buf)
}

/// Deterministic per-project id: first 8 bytes of sha256(root path), hex.
///
/// Keyed on the absolute root path, so moving or renaming the project directory
/// changes the id and orphans the existing store (a fresh, empty store appears
/// at the new location). Re-import or re-add after relocating.
pub fn project_id(root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Absolute path to a project's store file inside `secrets_dir`.
pub fn store_path_in(secrets_dir: &Path, root: &Path) -> PathBuf {
    secrets_dir.join(format!("{}.env", project_id(root)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_is_deterministic_16_hex_chars() {
        let root = Path::new("/home/dev/project");
        let a = project_id(root);
        let b = project_id(root);
        assert_eq!(a, b, "same path must hash identically");
        assert_eq!(a.len(), 16, "id is 8 bytes hex-encoded");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, project_id(Path::new("/home/dev/other")));
    }

    #[test]
    fn store_path_is_under_secrets_dir_with_env_extension() {
        let dir = Path::new("/home/dev/.config/act/secrets");
        let p = store_path_in(dir, Path::new("/home/dev/project"));
        assert!(p.starts_with(dir), "store lives under secrets dir");
        assert_eq!(p.extension().and_then(|e| e.to_str()), Some("env"));
    }

    #[test]
    fn resolve_root_walks_up_to_git_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        let nested = root.join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(resolve_root(&nested), root);
    }

    #[test]
    fn resolve_root_without_git_returns_start() {
        let tmp = tempfile::tempdir().unwrap();
        let start = tmp.path().canonicalize().unwrap();
        assert_eq!(resolve_root(&start), start);
    }

    #[test]
    fn resolve_root_maps_a_worktree_to_its_main_checkout() {
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

        assert_eq!(resolve_root(&wt.join("src")), main);
    }

    #[test]
    fn env_root_is_the_nearest_git_ancestor_even_in_a_worktree() {
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

        // The store is keyed on the main checkout; the .env is the worktree's own.
        assert_eq!(env_root(&wt.join("src")), wt);
        assert_eq!(env_root(&main), main);
        // No .git anywhere: start itself.
        let bare = base.join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        assert_eq!(env_root(&bare), bare);
    }

    #[test]
    fn resolve_root_keeps_a_submodule_as_its_own_root() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        std::fs::create_dir_all(base.join(".git").join("modules").join("sub")).unwrap();
        let sub = base.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(".git"), "gitdir: ../.git/modules/sub\n").unwrap();

        assert_eq!(resolve_root(&sub), sub);
    }

    #[test]
    fn secrets_dir_resolves_under_config_act() {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::var_os("ACT_CONFIG_DIR");
        unsafe {
            std::env::set_var("ACT_CONFIG_DIR", tmp.path());
        }
        assert_eq!(secrets_dir(), tmp.path().join("secrets"));
        unsafe {
            match prev {
                Some(v) => std::env::set_var("ACT_CONFIG_DIR", v),
                None => std::env::remove_var("ACT_CONFIG_DIR"),
            }
        }
    }
}
