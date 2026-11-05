//! Resolution of Rust relationships.

use super::{Family, Outcome, ResolveCx, Resolver};
use act_lite_parser::deps::Relationship;

/// The resolver of the Rust family.
pub struct RustResolver;

impl Resolver for RustResolver {
    fn family(&self) -> Family {
        Family::Rust
    }

    fn resolve(&self, _file: &str, _rel: &Relationship, _cx: &mut ResolveCx<'_>) -> Outcome {
        Outcome::Unresolved("resolution not implemented".to_string())
    }
}
