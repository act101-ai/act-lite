//! Resolution of Go relationships.

use super::{Family, Outcome, ResolveCx, Resolver};
use act_lite_parser::deps::Relationship;

/// The resolver of the Go family.
pub struct GoResolver;

impl Resolver for GoResolver {
    fn family(&self) -> Family {
        Family::Go
    }

    fn resolve(&self, _file: &str, _rel: &Relationship, _cx: &mut ResolveCx<'_>) -> Outcome {
        Outcome::Unresolved("resolution not implemented".to_string())
    }
}
