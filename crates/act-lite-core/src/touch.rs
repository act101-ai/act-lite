use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// One touched file: full content size and grammar at first touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchRecord {
    pub path: PathBuf,
    pub bytes: u64,
    pub grammar: String,
}

/// Per-operation tally of touched files, deduplicated by path.
#[derive(Debug, Default)]
pub struct TouchSet {
    records: HashMap<PathBuf, TouchRecord>,
    /// Post-edit after-sizes for ChangeSet-affected files (write augmentation),
    /// keyed by path so a file rewritten once contributes its after-size once.
    /// `naive_bytes` adds these on top of the read (before) sizes.
    write_after: HashMap<PathBuf, TouchRecord>,
    /// Σ edit-text bytes (`new_text` lengths) — the write side of `actual_bytes`.
    edit_text_bytes: u64,
}

impl TouchSet {
    /// Records a touch. First touch of a path wins — the pre-operation
    /// size is the naive read cost; later sizes (post-edit) are ignored.
    pub fn record(&mut self, path: &Path, bytes: u64, grammar: &str) {
        self.records
            .entry(path.to_path_buf())
            .or_insert_with(|| TouchRecord {
                path: path.to_path_buf(),
                bytes,
                grammar: grammar.to_string(),
            });
    }

    /// Records write augmentation for a ChangeSet-affected file: `after_bytes`
    /// (the rewritten file size) joins `naive_bytes`, and `edit_text_bytes`
    /// (the bytes the agent would emit to make the edits) joins `actual_bytes`.
    /// First write of a path wins for the after-size (one rewrite per op).
    pub fn record_write(
        &mut self,
        path: &Path,
        after_bytes: u64,
        edit_text_bytes: u64,
        grammar: &str,
    ) {
        self.write_after
            .entry(path.to_path_buf())
            .or_insert_with(|| TouchRecord {
                path: path.to_path_buf(),
                bytes: after_bytes,
                grammar: grammar.to_string(),
            });
        self.edit_text_bytes += edit_text_bytes;
    }

    /// Total edit-text bytes recorded for this operation (write side of actual).
    pub fn edit_text_bytes(&self) -> u64 {
        self.edit_text_bytes
    }

    /// Naive full-file cost: read (before) sizes plus write (after) sizes.
    pub fn naive_bytes(&self) -> u64 {
        self.records.values().map(|r| r.bytes).sum::<u64>()
            + self.write_after.values().map(|r| r.bytes).sum::<u64>()
    }

    /// Touched bytes binned by grammar name (reads + write after-sizes), sorted.
    pub fn grammar_bytes(&self) -> BTreeMap<String, u64> {
        let mut out = BTreeMap::new();
        for r in self.records.values().chain(self.write_after.values()) {
            *out.entry(r.grammar.clone()).or_insert(0) += r.bytes;
        }
        out
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.records.contains_key(path)
    }

    /// Iterates records in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &TouchRecord> {
        self.records.values()
    }
}

use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

/// Shared handle to an in-flight operation's touch set.
pub type SharedTouchSet = Arc<Mutex<TouchSet>>;

thread_local! {
    static SCOPE_STACK: RefCell<Vec<SharedTouchSet>> = const { RefCell::new(Vec::new()) };
}

/// Records a touch into the active scope. No active scope → no-op
/// (tests and internal machinery run unscoped at zero cost).
pub fn record(path: &Path, bytes: u64, grammar: &str) {
    if let Some(set) = current() {
        set.lock()
            .expect("touch set lock poisoned")
            .record(path, bytes, grammar);
    }
}

/// Records write augmentation into the active scope (no-op without one).
/// The refactor executor calls this for each ChangeSet-affected file.
pub fn record_write(path: &Path, after_bytes: u64, edit_text_bytes: u64, grammar: &str) {
    if let Some(set) = current() {
        set.lock().expect("touch set lock poisoned").record_write(
            path,
            after_bytes,
            edit_text_bytes,
            grammar,
        );
    }
}

fn current() -> Option<SharedTouchSet> {
    let local = SCOPE_STACK.with(|s| s.borrow().last().cloned());
    if local.is_some() {
        return local;
    }
    ASYNC_PROVIDER.get().and_then(|p| p())
}

static ASYNC_PROVIDER: OnceLock<fn() -> Option<SharedTouchSet>> = OnceLock::new();

/// Installs the process-wide fallback provider (e.g. a tokio task-local
/// lookup), consulted only when no thread-local scope is active.
/// Installing twice is a programming error.
pub fn install_async_provider(provider: fn() -> Option<SharedTouchSet>) {
    ASYNC_PROVIDER
        .set(provider)
        .expect("touch async provider already installed");
}

/// RAII guard: while alive on this thread, `record()` lands in its set.
/// Innermost scope wins; drop pops it.
pub struct TouchScope {
    set: SharedTouchSet,
}

impl TouchScope {
    /// Opens a fresh scope.
    pub fn enter() -> Self {
        Self::enter_with(Arc::new(Mutex::new(TouchSet::default())))
    }

    pub fn enter_with(set: SharedTouchSet) -> Self {
        SCOPE_STACK.with(|s| s.borrow_mut().push(Arc::clone(&set)));
        Self { set }
    }

    /// Handle to this scope's set (for cross-thread/task propagation in I2).
    pub fn shared(&self) -> SharedTouchSet {
        Arc::clone(&self.set)
    }

    /// Closes the scope and returns the collected set.
    pub fn finish(self) -> TouchSet {
        let set = Arc::clone(&self.set);
        drop(self); // pops the stack entry
        let mut guard = set.lock().expect("touch set lock poisoned");
        std::mem::take(&mut *guard)
    }
}

impl Drop for TouchScope {
    fn drop(&mut self) {
        SCOPE_STACK.with(|s| {
            let mut stack = s.borrow_mut();
            match stack.last() {
                Some(top) if Arc::ptr_eq(top, &self.set) => {
                    stack.pop();
                }
                _ => debug_assert!(false, "TouchScope dropped out of LIFO order"),
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditViolation {
    /// A file in the operation's result/ChangeSet was never recorded as touched.
    MissingTouch { path: PathBuf },
    /// A recorded touch has no resolved grammar (empty name, not even "other").
    UnresolvedGrammar { path: PathBuf },
    /// A file-touching operation touched zero files — it produced output
    /// about/within a file but never pulled any file through an instrumented
    /// path (the F1 class: a read/write op reading via an unhooked entry point).
    /// Distinct from touching an empty file, which still records one touch.
    MissingAttribution { op: String },
}

impl std::fmt::Display for AuditViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditViolation::MissingTouch { path } => {
                write!(
                    f,
                    "missing-touch: {} was affected but never recorded as touched",
                    path.display()
                )
            }
            AuditViolation::UnresolvedGrammar { path } => {
                write!(
                    f,
                    "unresolved-grammar: {} has no resolved grammar",
                    path.display()
                )
            }
            AuditViolation::MissingAttribution { op } => {
                write!(
                    f,
                    "missing-attribution: file op '{op}' touched no files (read via an unhooked path?)"
                )
            }
        }
    }
}

/// Process-global sink for audit violations surfaced under `ACT_TOUCH_AUDIT=1`.
/// Chokepoints push violations here; the CLI drains it after the command and
/// exits non-zero if any were recorded, so attribution gaps fail loudly
/// (and surface as scenario failures under the corpus validator) instead of
/// being silently logged.
static AUDIT_SINK: Mutex<Vec<AuditViolation>> = Mutex::new(Vec::new());

/// Records an audit violation into the process-global sink.
pub fn record_audit_violation(v: AuditViolation) {
    AUDIT_SINK.lock().expect("audit sink lock poisoned").push(v);
}

/// Drains and returns all recorded audit violations.
pub fn take_audit_violations() -> Vec<AuditViolation> {
    std::mem::take(&mut *AUDIT_SINK.lock().expect("audit sink lock poisoned"))
}

pub fn audit_active_scope(affected: &[PathBuf]) -> Vec<AuditViolation> {
    match current() {
        Some(set) => set.lock().expect("touch set lock poisoned").audit(affected),
        None => Vec::new(),
    }
}

/// Parses the `ACT_TOUCH_AUDIT` value: only `"1"` enables audit.
pub fn audit_enabled_from(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Reads `ACT_TOUCH_AUDIT` from the process environment.
pub fn audit_enabled() -> bool {
    audit_enabled_from(std::env::var("ACT_TOUCH_AUDIT").ok().as_deref())
}

impl TouchSet {
    /// Checks the audit invariants against the operation's affected files
    /// (result paths and/or ChangeSet-affected paths). Returns every violation
    /// found (empty == clean). (a) every affected file is touched,
    /// (b) every touch has a resolved grammar.
    pub fn audit(&self, affected: &[PathBuf]) -> Vec<AuditViolation> {
        let mut out = Vec::new();
        for path in affected {
            if !self.records.contains_key(path) && !self.write_after.contains_key(path) {
                out.push(AuditViolation::MissingTouch { path: path.clone() });
            }
        }
        for r in self.records.values().chain(self.write_after.values()) {
            if r.grammar.is_empty() {
                out.push(AuditViolation::UnresolvedGrammar {
                    path: r.path.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn touch_set_dedups_by_path_first_seen_wins() {
        let mut set = TouchSet::default();
        set.record(Path::new("/w/a.rs"), 100, "Rust");
        set.record(Path::new("/w/a.rs"), 999, "Rust"); // same file again: ignored
        set.record(Path::new("/w/b.ts"), 40, "TypeScript");
        assert_eq!(set.len(), 2);
        assert_eq!(set.naive_bytes(), 140);
    }

    #[test]
    fn touch_set_aggregates_bytes_by_grammar() {
        let mut set = TouchSet::default();
        set.record(Path::new("/w/a.rs"), 100, "Rust");
        set.record(Path::new("/w/c.rs"), 50, "Rust");
        set.record(Path::new("/w/b.ts"), 40, "TypeScript");
        let by_grammar = set.grammar_bytes();
        assert_eq!(by_grammar.get("Rust"), Some(&150));
        assert_eq!(by_grammar.get("TypeScript"), Some(&40));
        assert_eq!(by_grammar.len(), 2);
    }

    #[test]
    fn record_without_scope_is_a_noop_and_scope_starts_empty() {
        record(Path::new("/w/before.rs"), 10, "Rust"); // no scope: dropped
        let scope = TouchScope::enter();
        let set = scope.finish();
        assert!(set.is_empty());
    }

    #[test]
    fn scope_collects_records_and_finish_returns_them() {
        let scope = TouchScope::enter();
        record(Path::new("/w/a.rs"), 100, "Rust");
        record(Path::new("/w/a.rs"), 999, "Rust");
        record(Path::new("/w/b.ts"), 40, "TypeScript");
        let set = scope.finish();
        assert_eq!(set.len(), 2);
        assert_eq!(set.naive_bytes(), 140);
    }

    #[test]
    fn nested_scopes_attribute_to_the_innermost() {
        let outer = TouchScope::enter();
        record(Path::new("/w/outer.rs"), 10, "Rust");
        {
            let inner = TouchScope::enter();
            record(Path::new("/w/inner.rs"), 20, "Rust");
            let inner_set = inner.finish();
            assert_eq!(inner_set.len(), 1);
            assert!(inner_set.contains(Path::new("/w/inner.rs")));
        }
        record(Path::new("/w/outer2.rs"), 30, "Rust");
        let outer_set = outer.finish();
        assert_eq!(outer_set.len(), 2);
        assert!(outer_set.contains(Path::new("/w/outer.rs")));
        assert!(outer_set.contains(Path::new("/w/outer2.rs")));
        assert!(!outer_set.contains(Path::new("/w/inner.rs")));
    }

    #[test]
    fn scopes_are_thread_isolated() {
        let scope = TouchScope::enter();
        std::thread::spawn(|| {
            // Other thread has no scope: no-op, and must not panic.
            record(Path::new("/w/other-thread.rs"), 50, "Rust");
        })
        .join()
        .expect("spawned thread panicked");
        let set = scope.finish();
        assert!(set.is_empty());
    }

    #[test]
    fn enter_with_joins_an_existing_shared_set() {
        let shared: SharedTouchSet =
            std::sync::Arc::new(std::sync::Mutex::new(TouchSet::default()));
        {
            let _scope = TouchScope::enter_with(std::sync::Arc::clone(&shared));
            record(Path::new("/w/joined.rs"), 70, "Rust");
        }
        let set = shared.lock().expect("touch set lock poisoned");
        assert!(set.contains(Path::new("/w/joined.rs")));
        assert_eq!(set.naive_bytes(), 70);
    }

    // Process-global provider: exactly one test may install it.
    static PROVIDER_SET: std::sync::LazyLock<SharedTouchSet> = std::sync::LazyLock::new(|| {
        std::sync::Arc::new(std::sync::Mutex::new(TouchSet::default()))
    });

    fn test_provider() -> Option<SharedTouchSet> {
        Some(std::sync::Arc::clone(&PROVIDER_SET))
    }

    #[test]
    fn async_provider_supplies_set_only_when_no_thread_scope_is_active() {
        install_async_provider(test_provider);

        // No thread-local scope: record lands in the provider's set.
        record(Path::new("/w/via-provider.rs"), 60, "Rust");
        assert!(
            PROVIDER_SET
                .lock()
                .expect("touch set lock poisoned")
                .contains(Path::new("/w/via-provider.rs"))
        );

        // Active thread-local scope takes precedence over the provider.
        let scope = TouchScope::enter();
        record(Path::new("/w/via-scope.rs"), 30, "Rust");
        let set = scope.finish();
        assert!(set.contains(Path::new("/w/via-scope.rs")));
        assert!(
            !PROVIDER_SET
                .lock()
                .expect("touch set lock poisoned")
                .contains(Path::new("/w/via-scope.rs"))
        );
    }

    #[test]
    fn record_write_adds_after_bytes_to_naive_and_bins_by_grammar() {
        let mut set = TouchSet::default();
        // A write op first reads the file (before = 100), then rewrites it
        // (after = 130). naive = before + after = 230, all under "Rust".
        set.record(Path::new("/w/a.rs"), 100, "Rust");
        set.record_write(Path::new("/w/a.rs"), 130, 30, "Rust");
        assert_eq!(set.naive_bytes(), 230);
        assert_eq!(set.edit_text_bytes(), 30);
        assert_eq!(set.grammar_bytes().get("Rust"), Some(&230));
    }

    #[test]
    fn record_write_without_prior_read_still_counts_after_bytes() {
        // Defensive: if a write somehow lands without a prior read touch,
        // the after-size alone is the naive cost (never negative, never lost).
        let mut set = TouchSet::default();
        set.record_write(Path::new("/w/new.rs"), 40, 40, "Rust");
        assert_eq!(set.naive_bytes(), 40);
        assert_eq!(set.edit_text_bytes(), 40);
        assert_eq!(set.grammar_bytes().get("Rust"), Some(&40));
    }

    #[test]
    fn edit_text_bytes_accumulate_across_files() {
        let mut set = TouchSet::default();
        set.record_write(Path::new("/w/a.rs"), 10, 5, "Rust");
        set.record_write(Path::new("/w/b.ts"), 20, 7, "TypeScript");
        assert_eq!(set.edit_text_bytes(), 12);
        assert_eq!(set.naive_bytes(), 30);
    }

    #[test]
    fn audit_is_disabled_by_default() {
        // Without ACT_TOUCH_AUDIT=1 the gate is off regardless of env noise.
        // (We don't mutate process env here — just assert the parser's default.)
        assert!(!audit_enabled_from(None));
        assert!(!audit_enabled_from(Some("0")));
        assert!(audit_enabled_from(Some("1")));
    }

    #[test]
    fn audit_flags_missing_files_and_unresolved_grammar() {
        let mut set = TouchSet::default();
        set.record(Path::new("/w/seen.rs"), 100, "Rust");
        set.record(Path::new("/w/blank.rs"), 50, ""); // unresolved grammar

        let affected = [
            std::path::PathBuf::from("/w/seen.rs"),
            std::path::PathBuf::from("/w/absent.rs"), // never touched
        ];
        let violations = set.audit(&affected);

        // absent.rs is in the result but not the touch set → MissingTouch.
        // blank.rs has an empty grammar → UnresolvedGrammar.
        assert!(violations.iter().any(|v| matches!(
            v,
            AuditViolation::MissingTouch { path } if path.ends_with("absent.rs")
        )));
        assert!(violations.iter().any(|v| matches!(
            v,
            AuditViolation::UnresolvedGrammar { path } if path.ends_with("blank.rs")
        )));
        // seen.rs is clean — no violation references it.
        assert!(!violations.iter().any(|v| match v {
            AuditViolation::MissingTouch { path } | AuditViolation::UnresolvedGrammar { path } =>
                path.ends_with("seen.rs"),
            AuditViolation::MissingAttribution { .. } => false,
        }));
    }

    #[test]
    fn audit_sink_collects_and_drains() {
        // Drain any residue from other tests in this process first.
        let _ = take_audit_violations();
        record_audit_violation(AuditViolation::MissingAttribution {
            op: "skeleton".into(),
        });
        record_audit_violation(AuditViolation::MissingTouch {
            path: PathBuf::from("/w/x.rs"),
        });
        let drained = take_audit_violations();
        assert_eq!(drained.len(), 2);
        // A second drain is empty — take() clears the sink.
        assert!(take_audit_violations().is_empty());
    }

    #[test]
    fn audit_active_scope_flags_missing_touch_in_live_scope() {
        let scope = TouchScope::enter();
        record(Path::new("/w/seen.rs"), 100, "Rust");
        // affected includes a file never touched → MissingTouch from the live scope.
        let v = audit_active_scope(&[PathBuf::from("/w/seen.rs"), PathBuf::from("/w/gone.rs")]);
        drop(scope);
        assert!(v.iter().any(
            |x| matches!(x, AuditViolation::MissingTouch { path } if path.ends_with("gone.rs"))
        ));
        assert!(!v.iter().any(
            |x| matches!(x, AuditViolation::MissingTouch { path } if path.ends_with("seen.rs"))
        ));
    }

    #[test]
    fn audit_active_scope_clean_when_affected_is_touched() {
        let scope = TouchScope::enter();
        record(Path::new("/w/here.rs"), 80, "Rust");
        let v = audit_active_scope(&[PathBuf::from("/w/here.rs")]);
        drop(scope);
        assert!(
            v.is_empty(),
            "touched affected file must not violate, got {v:?}"
        );
    }

    #[test]
    fn audit_violation_display_is_actionable() {
        assert!(
            AuditViolation::MissingAttribution {
                op: "symbols".into()
            }
            .to_string()
            .contains("touched no files")
        );
        assert!(
            AuditViolation::MissingTouch {
                path: PathBuf::from("/w/a.rs")
            }
            .to_string()
            .contains("never recorded as touched")
        );
    }

    #[test]
    fn audit_clean_set_yields_no_violations() {
        let mut set = TouchSet::default();
        set.record(Path::new("/w/a.rs"), 100, "Rust");
        set.record_write(Path::new("/w/a.rs"), 120, 20, "Rust");
        let affected = [std::path::PathBuf::from("/w/a.rs")];
        assert!(set.audit(&affected).is_empty());
    }

    #[test]
    fn record_write_free_fn_routes_to_active_scope() {
        let scope = TouchScope::enter();
        record_write(Path::new("/w/w.rs"), 120, 20, "Rust");
        let set = scope.finish();
        assert_eq!(set.naive_bytes(), 120);
        assert_eq!(set.edit_text_bytes(), 20);
    }
}
