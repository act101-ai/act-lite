//! Resolution of Python relationships.

use super::{Family, Outcome, ResolveCx, Resolver};
use act_lite_parser::deps::Relationship;

/// The resolver of the Python family.
pub struct PythonResolver;

impl Resolver for PythonResolver {
    fn family(&self) -> Family {
        Family::Python
    }

    fn resolve(&self, _file: &str, _rel: &Relationship, _cx: &mut ResolveCx<'_>) -> Outcome {
        Outcome::Unresolved("resolution not implemented".to_string())
    }
}
