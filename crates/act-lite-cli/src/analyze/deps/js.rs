//! Resolution of JavaScript, JSX, TypeScript and TSX relationships.

use super::{Family, Outcome, ResolveCx, Resolver};
use act_lite_parser::deps::Relationship;

/// The resolver of the JavaScript, JSX, TypeScript and TSX family.
pub struct JsResolver;

impl Resolver for JsResolver {
    fn family(&self) -> Family {
        Family::JsTs
    }

    fn resolve(&self, _file: &str, _rel: &Relationship, _cx: &mut ResolveCx<'_>) -> Outcome {
        Outcome::Unresolved("resolution not implemented".to_string())
    }
}
