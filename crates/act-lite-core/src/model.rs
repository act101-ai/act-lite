//! Universal honesty fields shared by every analysis op result:
//! confidence of a finding, and which categories a grammar actually models.

use serde::{Deserialize, Serialize};

/// How a finding was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Confirmed via LSP / type resolution.
    Resolved,
    /// Matched a known syntactic node kind.
    Syntactic,
    /// Name/pattern heuristic (e.g. a call named `sleep`).
    Heuristic,
}

impl Confidence {
    /// Numeric strength, weakest first. Used to compute a result's
    /// confidence floor (the least-trustworthy classification it relied on).
    pub fn rank(&self) -> u8 {
        match self {
            Confidence::Heuristic => 0,
            Confidence::Syntactic => 1,
            Confidence::Resolved => 2,
        }
    }
}

/// The universal model contract.
///
/// Every archetype A–E analysis result implements this, making
/// provenance/confidence/modeled_kinds a compile-time guarantee rather than a
/// convention. The MCP boundary serializes op results through a generic helper
/// bounded on `OpModel`, so a new op result that forgets the contract fails to
/// compile when wired.
pub trait OpModel {
    /// The least-confident classification this result relied on. Lets a
    /// consumer decide whether to trust it or warm an LSP first.
    fn confidence_floor(&self) -> Confidence;
    /// Which categories the analyzed grammar actually models for this op.
    /// Categories outside the mask are *not-modeled*, never *absent*.
    fn modeled_kinds(&self) -> KindMask;
}

/// A set of effect kinds a grammar models, by stable string name.
/// Lets a consumer distinguish "none found" from "not modeled here".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindMask {
    kinds: Vec<String>,
}

impl KindMask {
    pub fn new() -> Self {
        Self { kinds: Vec::new() }
    }
    pub fn with(mut self, kind: &str) -> Self {
        if !self.kinds.iter().any(|k| k == kind) {
            self.kinds.push(kind.to_string());
        }
        self
    }
    pub fn contains(&self, kind: &str) -> bool {
        self.kinds.iter().any(|k| k == kind)
    }
    /// Whether this mask models no kinds at all (the "not modeled here" state).
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
    /// Intersection — used later by effect_closure for cross-grammar coverage.
    pub fn intersect(&self, other: &KindMask) -> KindMask {
        KindMask {
            kinds: self
                .kinds
                .iter()
                .filter(|k| other.contains(k))
                .cloned()
                .collect(),
        }
    }
    /// Union — keeps all of self's kinds, adds any of `other`'s not already
    /// present (deduped). Used to compose disjoint kind families (e.g. effect
    /// dimensions ∪ control dimensions) into one honest modeled mask.
    pub fn union(&self, other: &KindMask) -> KindMask {
        let mut kinds = self.kinds.clone();
        for k in &other.kinds {
            if !kinds.iter().any(|existing| existing == k) {
                kinds.push(k.clone());
            }
        }
        KindMask { kinds }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kindmask_with_is_dedup_and_contains() {
        let m = KindMask::new().with("read").with("read").with("await");
        assert!(m.contains("read"));
        assert!(m.contains("await"));
        assert!(!m.contains("block"));
    }

    #[test]
    fn kindmask_intersect_keeps_common() {
        let a = KindMask::new().with("read").with("await");
        let b = KindMask::new().with("await").with("raise");
        let i = a.intersect(&b);
        assert!(i.contains("await"));
        assert!(!i.contains("read"));
        assert!(!i.contains("raise"));
    }

    #[test]
    fn kindmask_union_keeps_common_and_distinct() {
        let u = KindMask::new()
            .with("write")
            .union(&KindMask::new().with("write").with("branch"));
        assert!(u.contains("write"));
        assert!(u.contains("branch"));
        assert_eq!(u.kinds.len(), 2);
    }

    #[test]
    fn kindmask_is_empty_distinguishes_none_from_modeled() {
        assert!(KindMask::new().is_empty());
        assert!(!KindMask::new().with("read").is_empty());
    }

    #[test]
    fn confidence_rank_orders_weakest_first() {
        assert!(Confidence::Heuristic.rank() < Confidence::Syntactic.rank());
        assert!(Confidence::Syntactic.rank() < Confidence::Resolved.rank());
    }
}
