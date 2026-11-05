//! Compile-time language and operation registry.
//!
//! Uses the `inventory` crate for distributed registration.
//! Each language module self-registers its capabilities.

use std::sync::LazyLock;

/// Unique language identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageId(pub &'static str);

impl LanguageId {
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for LanguageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Language category for grouping.
#[derive(Debug, Clone, Copy)]
pub enum LanguageCategory {
    Scripting,
    Systems,
    Managed,
    Functional,
    Dynamic,
    Data,
    Shader,
    Other,
}

impl std::fmt::Display for LanguageCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanguageCategory::Scripting => write!(f, "Scripting"),
            LanguageCategory::Systems => write!(f, "Systems"),
            LanguageCategory::Managed => write!(f, "Managed"),
            LanguageCategory::Functional => write!(f, "Functional"),
            LanguageCategory::Dynamic => write!(f, "Dynamic"),
            LanguageCategory::Data => write!(f, "Data"),
            LanguageCategory::Shader => write!(f, "Shader"),
            LanguageCategory::Other => write!(f, "Other"),
        }
    }
}

/// Language metadata — self-registered by each language module.
pub struct LanguageMeta {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub category: LanguageCategory,
}

inventory::collect!(LanguageMeta);

/// Global language registry.
pub static LANGUAGE_REGISTRY: LazyLock<LanguageRegistry> =
    LazyLock::new(LanguageRegistry::from_inventory);

pub struct LanguageRegistry {
    extension_map: std::collections::HashMap<&'static str, LanguageId>,
}

impl LanguageRegistry {
    fn from_inventory() -> Self {
        let mut extension_map = std::collections::HashMap::new();

        for meta in inventory::iter::<LanguageMeta> {
            for ext in meta.extensions {
                extension_map.insert(*ext, meta.id);
            }
        }

        Self { extension_map }
    }

    pub fn from_extension(&self, ext: &str) -> Option<LanguageId> {
        self.extension_map.get(ext).copied()
    }

    pub fn all(&self) -> impl Iterator<Item = &'static LanguageMeta> {
        inventory::iter::<LanguageMeta>.into_iter()
    }

    pub fn get(&self, id: LanguageId) -> Option<&'static LanguageMeta> {
        inventory::iter::<LanguageMeta>
            .into_iter()
            .find(|m| m.id == id)
    }
}
