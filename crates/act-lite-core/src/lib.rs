//! act-lite-core: Core types and traits with no I/O dependencies.
//!
//! This crate defines the foundational types used throughout act:
//!
//! - **Position types**: [`Location`], [`Range`] — represent positions in source files
//! - **Modification primitives**: [`Edit`], [`ChangeSet`] — represent pending changes
//! - **Symbol representation**: [`Symbol`], [`SymbolKind`] — represent code entities
//! - **Error types**: [`ActError`], [`ErrorCategory`] — actionable error handling
//!
//! # Design Principles
//!
//! - **No I/O**: This crate has no file system or network dependencies
//! - **Serializable**: All types support JSON serialization via serde
//! - **Composable**: Edits can be composed into changesets, changesets into operations
//!
//! # Examples
//!
//! ```
//! use act_lite_core::{Location, Range};
//! use std::path::PathBuf;
//!
//! // Create a range representing a symbol location
//! let start = Location::new(PathBuf::from("src/main.rs"), 10, 5);
//! let end = Location::new(PathBuf::from("src/main.rs"), 10, 15);
//! let range = Range::new(start, end);
//! ```

mod compact;
mod edit;
mod error;
mod keywords;
mod location;
mod meta;
pub mod model;
pub mod registry;
mod symbol;
pub mod touch;

pub use compact::CompactRange;
pub use edit::{ChangeSet, ChangeSetError, Edit};
pub use error::{ActError, Candidate, ErrorCategory, ErrorCode, ErrorDetails};
pub use keywords::{
    is_c_keyword, is_cpp_keyword, is_go_keyword, is_javascript_keyword, is_r_keyword,
    is_ruby_keyword, is_rust_keyword, is_typescript_keyword,
};
pub use location::{Location, Range};
pub use meta::{OperationClass, OperationMeta, SessionMeta};
pub use model::{Confidence, KindMask, OpModel};
pub use registry::{
    LANGUAGE_REGISTRY, LanguageCategory, LanguageId, LanguageMeta, LanguageRegistry,
};
pub use symbol::{Symbol, SymbolKind, SymbolReference, Visibility};
