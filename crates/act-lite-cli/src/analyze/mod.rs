//! Shared conventions of the `analyze` commands: the result envelope,
//! repository discovery and the classification of excluded files.

pub mod classify;
pub mod clones;
pub mod command;
pub mod coupling;
pub mod cycles;
pub mod deps;
pub mod discovery;
pub mod embedded;
pub mod envelope;
pub mod focus;
pub mod glob;
pub mod graph;
pub mod hotspots;
pub mod ignore;
pub mod impact;
pub mod patterns;
pub mod read;
pub mod run;

#[cfg(debug_assertions)]
pub mod fixture;
