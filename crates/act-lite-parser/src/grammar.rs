//! Grammar definitions for supported languages.

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::language::LanguageSupport;
use crate::languages::{
    BashSupport, CMakeSupport, CSharpSupport, CSupport, CppSupport, CssSupport, DartSupport,
    DockerfileSupport, ElixirSupport, GoSupport, GraphQlSupport, HclTerraformSupport, HtmlSupport,
    JavaScriptSupport, JavaSupport, JsonSupport, JsoncSupport, JsxSupport, KotlinSupport,
    LuaSupport, MakeSupport, MarkdownSupport, NixSupport, PhpSupport, ProtobufSupport,
    PythonSupport, RubySupport, RustSupport, ScalaSupport, ScssSupport, SqlSupport, SwiftSupport,
    TomlSupport, TsxSupport, TypeScriptSupport, XmlSupport, YamlSupport, ZigSupport,
};

/// Supported language grammars.
///
/// Each variant represents a language with tree-sitter grammar support.
/// Variants are sorted alphabetically to minimize merge conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum Grammar {
    /// Bash shell scripting
    Bash,
    /// C
    C,
    /// CMake build descriptions
    CMake,
    /// C++
    Cpp,
    /// C#
    CSharp,
    /// CSS
    Css,
    /// Dart
    Dart,
    /// Dockerfile
    Dockerfile,
    /// Elixir
    Elixir,
    /// Go
    Go,
    /// GraphQL schemas and operations
    GraphQl,
    /// HCL/Terraform
    HclTerraform,
    /// HTML (HyperText Markup Language)
    Html,
    /// Java
    Java,
    /// JavaScript
    JavaScript,
    /// JSX — JavaScript with JSX syntax
    Jsx,
    /// JSON (data format)
    Json,
    /// JSONC — JSON with comments
    Jsonc,
    /// Kotlin
    Kotlin,
    /// Lua
    Lua,
    /// Make (GNU Make makefiles)
    Make,
    /// Markdown
    Markdown,
    /// Nix expression language
    Nix,
    /// PHP
    Php,
    /// Protocol Buffers
    Protobuf,
    /// Python
    Python,
    /// Ruby
    Ruby,
    /// Rust
    Rust,
    /// Scala
    Scala,
    /// SQL
    Sql,
    /// SCSS
    Scss,
    /// Swift
    Swift,
    /// TOML (configuration format)
    Toml,
    /// TSX — TypeScript with JSX syntax
    Tsx,
    /// TypeScript
    TypeScript,
    /// XML (data format)
    Xml,
    /// YAML
    Yaml,
    /// Zig
    Zig,
}

impl Grammar {
    /// Returns the human-readable name of this grammar.
    pub fn name(&self) -> &'static str {
        match self {
            Grammar::Bash => "Bash",
            Grammar::C => "C",
            Grammar::CMake => "CMake",
            Grammar::Cpp => "C++",
            Grammar::CSharp => "C#",
            Grammar::Css => "CSS",
            Grammar::Dart => "Dart",
            Grammar::Dockerfile => "Dockerfile",
            Grammar::Elixir => "Elixir",
            Grammar::Go => "Go",
            Grammar::GraphQl => "GraphQL",
            Grammar::HclTerraform => "HCL/Terraform",
            Grammar::Html => "HTML",
            Grammar::Java => "Java",
            Grammar::JavaScript => "JavaScript",
            Grammar::Jsx => "JSX",
            Grammar::Json => "JSON",
            Grammar::Jsonc => "JSONC",
            Grammar::Kotlin => "Kotlin",
            Grammar::Lua => "Lua",
            Grammar::Make => "Make",
            Grammar::Markdown => "Markdown",
            Grammar::Nix => "Nix",
            Grammar::Php => "PHP",
            Grammar::Protobuf => "Protocol Buffers",
            Grammar::Python => "Python",
            Grammar::Ruby => "Ruby",
            Grammar::Rust => "Rust",
            Grammar::Scala => "Scala",
            Grammar::Scss => "SCSS",
            Grammar::Sql => "SQL",
            Grammar::Swift => "Swift",
            Grammar::Toml => "TOML",
            Grammar::Tsx => "TSX",
            Grammar::TypeScript => "TypeScript",
            Grammar::Xml => "XML",
            Grammar::Yaml => "YAML",
            Grammar::Zig => "Zig",
        }
    }

    /// Returns the query directory name for this grammar.
    ///
    /// Maps each grammar to the subdirectory under `crates/act-lite-parser/queries/`
    /// where its tree-sitter query files live. Cannot be derived from `name()`
    /// because names have special characters (e.g. "C#" → "csharp", "C++" → "cpp").
    pub fn query_dir(&self) -> &'static str {
        match self {
            Grammar::Bash => "bash",
            Grammar::C => "c",
            Grammar::CMake => "cmake",
            Grammar::Cpp => "cpp",
            Grammar::CSharp => "csharp",
            Grammar::Css => "css",
            Grammar::Dart => "dart",
            Grammar::Dockerfile => "dockerfile",
            Grammar::Elixir => "elixir",
            Grammar::Go => "go",
            Grammar::GraphQl => "graphql",
            Grammar::HclTerraform => "hcl_terraform",
            Grammar::Html => "html",
            Grammar::Java => "java",
            Grammar::JavaScript => "javascript",
            Grammar::Jsx => "jsx",
            Grammar::Json | Grammar::Jsonc => "json",
            Grammar::Kotlin => "kotlin",
            Grammar::Lua => "lua",
            Grammar::Make => "make",
            Grammar::Markdown => "markdown",
            Grammar::Nix => "nix",
            Grammar::Php => "php",
            Grammar::Protobuf => "protobuf",
            Grammar::Python => "python",
            Grammar::Ruby => "ruby",
            Grammar::Rust => "rust",
            Grammar::Scala => "scala",
            Grammar::Sql => "sql",
            Grammar::Scss => "scss",
            Grammar::Swift => "swift",
            Grammar::Toml => "toml",
            Grammar::Tsx => "tsx",
            Grammar::TypeScript => "typescript",
            Grammar::Xml => "xml",
            Grammar::Yaml => "yaml",
            Grammar::Zig => "zig",
        }
    }

    /// Returns the language support implementation for this grammar.
    ///
    /// This is the single authoritative mapping from Grammar to LanguageSupport.
    /// All dispatch code should use this method rather than maintaining separate
    /// match blocks.
    pub fn language_support(&self) -> &'static dyn LanguageSupport {
        match self {
            Grammar::Bash => &BashSupport,
            Grammar::C => &CSupport,
            Grammar::CMake => &CMakeSupport,
            Grammar::Cpp => &CppSupport,
            Grammar::CSharp => &CSharpSupport,
            Grammar::Css => &CssSupport,
            Grammar::Dart => &DartSupport,
            Grammar::Dockerfile => &DockerfileSupport,
            Grammar::Elixir => &ElixirSupport,
            Grammar::Go => &GoSupport,
            Grammar::GraphQl => &GraphQlSupport,
            Grammar::HclTerraform => &HclTerraformSupport,
            Grammar::Html => &HtmlSupport,
            Grammar::Java => &JavaSupport,
            Grammar::JavaScript => &JavaScriptSupport,
            Grammar::Jsx => &JsxSupport,
            Grammar::Json => &JsonSupport,
            Grammar::Jsonc => &JsoncSupport,
            Grammar::Kotlin => &KotlinSupport,
            Grammar::Lua => &LuaSupport,
            Grammar::Make => &MakeSupport,
            Grammar::Markdown => &MarkdownSupport,
            Grammar::Nix => &NixSupport,
            Grammar::Php => &PhpSupport,
            Grammar::Protobuf => &ProtobufSupport,
            Grammar::Python => &PythonSupport,
            Grammar::Ruby => &RubySupport,
            Grammar::Rust => &RustSupport,
            Grammar::Scala => &ScalaSupport,
            Grammar::Scss => &ScssSupport,
            Grammar::Sql => &SqlSupport,
            Grammar::Swift => &SwiftSupport,
            Grammar::Toml => &TomlSupport,
            Grammar::Tsx => &TsxSupport,
            Grammar::TypeScript => &TypeScriptSupport,
            Grammar::Xml => &XmlSupport,
            Grammar::Yaml => &YamlSupport,
            Grammar::Zig => &ZigSupport,
        }
    }

    /// Returns all Grammar variants.
    ///
    /// Derived from `strum::EnumIter` — automatically includes every variant.
    /// Use `Grammar::iter()` directly when an iterator is sufficient.
    pub fn all() -> Vec<Grammar> {
        Grammar::iter().collect()
    }

    /// Returns this grammar's language category from the registry metadata.
    ///
    /// Thin accessor delegating to the `LanguageMeta.category` self-registered by
    /// each language module. Resolved via the grammar's first file extension (the
    /// registry is keyed by extension). Falls back to `Other` only if no
    /// registry entry is found for any of the grammar's extensions — every
    /// shipped grammar registers a `LanguageMeta`, so the fallback is defensive.
    pub fn category(&self) -> act_lite_core::registry::LanguageCategory {
        use act_lite_core::registry::LANGUAGE_REGISTRY;
        for ext in self.language_support().extensions() {
            if let Some(id) = LANGUAGE_REGISTRY.from_extension(ext)
                && let Some(meta) = LANGUAGE_REGISTRY.get(id)
            {
                return meta.category;
            }
        }
        act_lite_core::registry::LanguageCategory::Other
    }

    /// Returns all file extensions recognized across all supported languages.
    ///
    /// Collected from each language's `LanguageSupport::extensions()`.
    /// This is the single source of truth — no hardcoded extension lists elsewhere.
    pub fn all_extensions() -> Vec<&'static str> {
        Grammar::iter()
            .flat_map(|g| g.language_support().extensions().iter().copied())
            .collect()
    }

    /// Returns project marker files associated with this grammar.
    ///
    /// Markers are filenames (e.g. `"Cargo.toml"`) or glob-like wildcard patterns
    /// that begin with `*` (e.g. `"*.csproj"`), which match file suffixes.
    /// Grammars without a standard project marker return an empty slice.
    pub fn project_markers(&self) -> &'static [&'static str] {
        match self {
            Grammar::TypeScript => &["tsconfig.json", "tsconfig.base.json"],
            Grammar::JavaScript => &["package.json", "jsconfig.json"],
            Grammar::Rust => &["Cargo.toml"],
            Grammar::Go => &["go.mod"],
            Grammar::Python => &["pyproject.toml", "setup.py", "setup.cfg"],
            Grammar::Java => &["pom.xml", "build.gradle", "build.gradle.kts"],
            Grammar::Kotlin => &["build.gradle.kts", "build.gradle"],
            Grammar::CSharp => &["*.csproj", "*.sln"],
            Grammar::Cpp => &["CMakeLists.txt", "Makefile"],
            Grammar::C => &["CMakeLists.txt", "Makefile"],
            Grammar::Swift => &["Package.swift"],
            Grammar::Ruby => &["Gemfile"],
            Grammar::Php => &["composer.json"],
            Grammar::Scala => &["build.sbt"],
            Grammar::Elixir => &["mix.exs"],
            _ => &[],
        }
    }

    /// Detects grammar from a conventional file NAME (no extension required).
    ///
    /// The single source for the filename conventions that extension routing
    /// cannot express: `Dockerfile` (case-insensitive; no extension), the
    /// three makefile names GNU Make reads by default, `CMakeLists.txt`, and
    /// the JSONC configuration names (all case-sensitive; `tsconfig.*.json`
    /// and `jsconfig.*.json` need one or more characters for `*`). Returns
    /// `None` when the name matches no
    /// convention; callers fall back to `from_extension`. Routers beyond the
    /// parser layer (MCP) must route through this so the convention list
    /// cannot drift.
    pub fn from_filename(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("dockerfile") {
            return Some(Grammar::Dockerfile);
        }
        match name {
            "Makefile" | "makefile" | "GNUmakefile" => Some(Grammar::Make),
            "CMakeLists.txt" => Some(Grammar::CMake),
            "tsconfig.json" | "jsconfig.json" | "devcontainer.json" | ".devcontainer.json"
            | ".eslintrc.json" => Some(Grammar::Jsonc),
            _ if ["tsconfig.", "jsconfig."].iter().any(|prefix| {
                name.strip_prefix(prefix)
                    .and_then(|rest| rest.strip_suffix(".json"))
                    .is_some_and(|middle| !middle.is_empty())
            }) =>
            {
                Some(Grammar::Jsonc)
            }
            _ => None,
        }
    }

    /// The grammar `SourceFile::load` routes `path` to: filename conventions
    /// ([`Grammar::from_filename`]) first, then the extension. The single
    /// classifier for the loader and the reference walk, so they cannot drift.
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(Grammar::from_filename)
            .or_else(|| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .and_then(Grammar::from_extension)
            })
    }

    /// Whether a cross-file walk (`references`, `definition`) from `anchor`,
    /// a file of this grammar, reads the file at `path`. Each grammar walks
    /// its own files, except JSON and JSONC: from a JSON file, or a JSONC
    /// file routed by its name, the walk reads the JSON files and the JSONC
    /// files routed by name; from any other JSONC file it reads every JSON
    /// and JSONC file.
    pub fn walk_reads(&self, anchor: &std::path::Path, path: &std::path::Path) -> bool {
        let named_jsonc = |p: &std::path::Path| {
            p.file_name()
                .and_then(|name| name.to_str())
                .and_then(Grammar::from_filename)
                == Some(Grammar::Jsonc)
        };
        match (self, Grammar::from_path(path)) {
            (Grammar::Json | Grammar::Jsonc, Some(Grammar::Json)) => true,
            (Grammar::Json | Grammar::Jsonc, Some(Grammar::Jsonc)) => {
                named_jsonc(path) || (*self == Grammar::Jsonc && !named_jsonc(anchor))
            }
            (_, other) => other == Some(*self),
        }
    }

    /// Detects grammar from file extension.
    ///
    /// Returns `None` if the extension is not recognized.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "sh" | "bash" => Some(Grammar::Bash),
            "c" | "h" => Some(Grammar::C),
            "cmake" => Some(Grammar::CMake),
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => Some(Grammar::Cpp),
            "cs" | "csx" => Some(Grammar::CSharp),
            "css" => Some(Grammar::Css),
            "dart" => Some(Grammar::Dart),
            "dockerfile" => Some(Grammar::Dockerfile),
            "ex" | "exs" | "leex" => Some(Grammar::Elixir),
            "go" => Some(Grammar::Go),
            "graphql" | "graphqls" | "gql" => Some(Grammar::GraphQl),
            "hcl" | "tf" => Some(Grammar::HclTerraform),
            // `.conf` stays with Nginx — it is ambiguous with HOCON, Apache,
            // and many others. Use the explicit `.hocon` extension to opt
            // into the HOCON parser.
            "html" | "htm" => Some(Grammar::Html),
            "java" => Some(Grammar::Java),
            "js" | "mjs" | "cjs" => Some(Grammar::JavaScript),
            "jsx" => Some(Grammar::Jsx),
            "json" => Some(Grammar::Json),
            "jsonc" | "code-workspace" => Some(Grammar::Jsonc),
            "kt" | "kts" => Some(Grammar::Kotlin),
            "lua" => Some(Grammar::Lua),
            "mk" | "mak" => Some(Grammar::Make),
            "md" | "markdown" | "mdown" | "mkd" | "mkdn" => Some(Grammar::Markdown),
            "nix" => Some(Grammar::Nix),
            "php" | "phtml" | "php3" | "php4" | "php5" | "php7" | "php8" => Some(Grammar::Php),
            "proto" => Some(Grammar::Protobuf),
            "py" | "pyi" | "pyw" => Some(Grammar::Python),
            "rb" | "rake" | "gemspec" | "ru" | "erb" => Some(Grammar::Ruby),
            "rs" => Some(Grammar::Rust),
            "scala" | "sc" => Some(Grammar::Scala),
            "scss" => Some(Grammar::Scss),
            "sql" | "psql" | "pgsql" | "mysql" => Some(Grammar::Sql),
            "swift" => Some(Grammar::Swift),
            "toml" => Some(Grammar::Toml),
            "tsx" => Some(Grammar::Tsx),
            "ts" | "mts" | "cts" => Some(Grammar::TypeScript),
            "xml" | "xsd" | "xsl" | "xslt" | "svg" | "plist" => Some(Grammar::Xml),
            "csproj" | "fsproj" | "vbproj" | "props" | "targets" => Some(Grammar::Xml),
            "resx" | "xaml" => Some(Grammar::Xml),
            "yaml" | "yml" => Some(Grammar::Yaml),
            "zig" => Some(Grammar::Zig),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_name() {
        assert_eq!(Grammar::TypeScript.name(), "TypeScript");
    }

    #[test]
    fn category_resolves_from_registry_meta() {
        use act_lite_core::registry::LanguageCategory;
        // YAML is registered as a Data grammar; Rust as Systems. The accessor
        // must delegate to the self-registered LanguageMeta.category (resolved
        // via the grammar's file extensions), not return Other.
        assert!(matches!(Grammar::Yaml.category(), LanguageCategory::Data));
        assert!(matches!(
            Grammar::Rust.category(),
            LanguageCategory::Systems
        ));
    }

    #[test]
    fn project_markers_typescript() {
        let markers = Grammar::TypeScript.project_markers();
        assert!(markers.contains(&"tsconfig.json"));
    }

    #[test]
    fn project_markers_rust() {
        assert_eq!(Grammar::Rust.project_markers(), &["Cargo.toml"]);
    }

    #[test]
    fn project_markers_csharp_wildcards() {
        let markers = Grammar::CSharp.project_markers();
        assert!(markers.contains(&"*.csproj"));
        assert!(markers.contains(&"*.sln"));
    }

    #[test]
    fn project_markers_unknown_language_is_empty() {
        // Grammars without standard project markers return an empty slice.
        assert!(Grammar::Markdown.project_markers().is_empty());
        assert!(Grammar::Yaml.project_markers().is_empty());
    }

    #[test]
    fn from_extension_typescript() {
        assert_eq!(Grammar::from_extension("ts"), Some(Grammar::TypeScript));
        assert_eq!(Grammar::from_extension("mts"), Some(Grammar::TypeScript));
        assert_eq!(Grammar::from_extension("cts"), Some(Grammar::TypeScript));
    }

    #[test]
    fn from_extension_inc_is_unrouted() {
        // `.inc` is claimed by four ecosystems (Pascal, PHP, assembler, Make
        // includes) and no grammar can win it; BitBake does not advertise it.
        // Routing it to BitBake silently turns a Pascal/PHP/asm `.inc` file
        // into an empty BitBake skeleton instead of an actionable error.
        assert_eq!(Grammar::from_extension("inc"), None);
    }

    #[test]
    fn tsx_name() {
        assert_eq!(Grammar::Tsx.name(), "TSX");
    }

    #[test]
    fn tsx_extension() {
        assert_eq!(Grammar::from_extension("tsx"), Some(Grammar::Tsx));
    }

    #[test]
    fn from_extension_unknown() {
        assert_eq!(Grammar::from_extension("unknown"), None);
    }

    #[test]
    fn from_filename_convention_forms() {
        // The six filename conventions, every form load_inner routes by name.
        assert_eq!(
            Grammar::from_filename("Dockerfile"),
            Some(Grammar::Dockerfile)
        );
        // dockerfile matching is case-insensitive (eq_ignore_ascii_case).
        assert_eq!(
            Grammar::from_filename("dockerfile"),
            Some(Grammar::Dockerfile)
        );
        // .sqlite.sql suffix matching is case-insensitive.
    }

    #[test]
    fn from_filename_non_conventions_return_none() {
        // No convention matches: from_filename is name-only routing; callers
        // fall back to from_extension.
        assert_eq!(Grammar::from_filename("foo"), None);
        assert_eq!(Grammar::from_filename("BUILD.bak"), None);
        assert_eq!(Grammar::from_filename("README"), None);
        // The Kconfig family is case-SENSITIVE: lowercase `kconfig` is not a
        // convention filename (only the `.kconfig` EXTENSION routes, via
        // from_extension). Same shape for the other family forms.
        assert_eq!(Grammar::from_filename("kconfig"), None);
        assert_eq!(Grammar::from_filename("DEFCONFIG"), None);
        assert_eq!(Grammar::from_filename("KCONFIG.arch"), None);
    }

    #[test]
    fn from_filename_make_and_cmake_conventions() {
        assert_eq!(Grammar::from_filename("Makefile"), Some(Grammar::Make));
        assert_eq!(Grammar::from_filename("makefile"), Some(Grammar::Make));
        assert_eq!(Grammar::from_filename("GNUmakefile"), Some(Grammar::Make));
        assert_eq!(
            Grammar::from_filename("CMakeLists.txt"),
            Some(Grammar::CMake)
        );
    }

    #[test]
    fn from_path_refuses_near_miss_build_file_names() {
        for name in [
            "Makefile.bak",
            "makefile.txt",
            "MAKEFILE",
            "BSDmakefile",
            "Makefile.am",
            "cmakelists.txt",
            "CMakeLists.txt.orig",
            "notes.txt",
        ] {
            assert_eq!(
                Grammar::from_path(std::path::Path::new(name)),
                None,
                "{name}"
            );
        }
    }

    #[test]
    fn added_grammar_names_and_query_dirs() {
        for (grammar, name, dir) in [
            (Grammar::Xml, "XML", "xml"),
            (Grammar::GraphQl, "GraphQL", "graphql"),
            (Grammar::Protobuf, "Protocol Buffers", "protobuf"),
            (Grammar::Make, "Make", "make"),
            (Grammar::CMake, "CMake", "cmake"),
            (Grammar::Nix, "Nix", "nix"),
        ] {
            assert_eq!(grammar.name(), name);
            assert_eq!(grammar.query_dir(), dir);
            assert!(
                matches!(
                    grammar.category(),
                    act_lite_core::registry::LanguageCategory::Data
                ),
                "{grammar:?} is a Data grammar"
            );
        }
    }

    #[test]
    fn xml_extensions() {
        for ext in [
            "xml", "xsd", "xsl", "xslt", "svg", "plist", "csproj", "fsproj", "vbproj", "props",
            "targets", "resx", "xaml",
        ] {
            assert_eq!(Grammar::from_extension(ext), Some(Grammar::Xml), "{ext}");
        }
        assert_eq!(Grammar::from_extension("dtd"), None);
    }

    #[test]
    fn graphql_extensions() {
        for ext in ["graphql", "graphqls", "gql"] {
            assert_eq!(
                Grammar::from_extension(ext),
                Some(Grammar::GraphQl),
                "{ext}"
            );
        }
    }

    #[test]
    fn protobuf_extension() {
        assert_eq!(Grammar::from_extension("proto"), Some(Grammar::Protobuf));
    }

    #[test]
    fn make_extensions() {
        assert_eq!(Grammar::from_extension("mk"), Some(Grammar::Make));
        assert_eq!(Grammar::from_extension("mak"), Some(Grammar::Make));
    }

    #[test]
    fn cmake_extension() {
        assert_eq!(Grammar::from_extension("cmake"), Some(Grammar::CMake));
    }

    #[test]
    fn nix_extension() {
        assert_eq!(Grammar::from_extension("nix"), Some(Grammar::Nix));
    }

    #[test]
    fn zig_extension_name_query_dir_and_category() {
        assert_eq!(Grammar::from_extension("zig"), Some(Grammar::Zig));
        assert_eq!(Grammar::Zig.name(), "Zig");
        assert_eq!(Grammar::Zig.query_dir(), "zig");
        assert!(matches!(
            Grammar::Zig.category(),
            act_lite_core::registry::LanguageCategory::Systems
        ));
    }

    #[test]
    fn r_extensions_file_name_query_dir_and_category() {
        assert_eq!(Grammar::from_extension("R"), Some(Grammar::R));
        assert_eq!(Grammar::from_extension("r"), Some(Grammar::R));
        assert_eq!(Grammar::from_filename(".Rprofile"), Some(Grammar::R));
        assert_eq!(
            Grammar::from_path(std::path::Path::new("pkg/.Rprofile")),
            Some(Grammar::R)
        );
        assert_eq!(Grammar::from_filename(".rprofile"), None);
        assert_eq!(Grammar::from_filename("Rprofile"), None);
        assert_eq!(Grammar::R.name(), "R");
        assert_eq!(Grammar::R.query_dir(), "r");
        assert!(matches!(
            Grammar::R.category(),
            act_lite_core::registry::LanguageCategory::Dynamic
        ));
    }

    #[test]
    fn jsonc_name_query_dir_and_category() {
        assert_eq!(Grammar::Jsonc.name(), "JSONC");
        assert_eq!(Grammar::Jsonc.query_dir(), "json");
        assert!(matches!(
            Grammar::Jsonc.category(),
            act_lite_core::registry::LanguageCategory::Data
        ));
    }

    #[test]
    fn jsonc_extensions() {
        assert_eq!(Grammar::from_extension("jsonc"), Some(Grammar::Jsonc));
        assert_eq!(
            Grammar::from_extension("code-workspace"),
            Some(Grammar::Jsonc)
        );
    }

    #[test]
    fn from_filename_jsonc_conventions() {
        for name in [
            "tsconfig.json",
            "jsconfig.json",
            "tsconfig.app.json",
            "jsconfig.base.json",
            "tsconfig.a.b.json",
            "devcontainer.json",
            ".devcontainer.json",
            ".eslintrc.json",
        ] {
            assert_eq!(Grammar::from_filename(name), Some(Grammar::Jsonc), "{name}");
        }
    }

    #[test]
    fn from_path_jsonc_near_misses() {
        for (name, grammar) in [
            ("mytsconfig.json", Some(Grammar::Json)),
            ("tsconfig..json", Some(Grammar::Json)),
            ("tsconfig.json.bak", None),
            ("TSCONFIG.json", Some(Grammar::Json)),
            ("package.json", Some(Grammar::Json)),
        ] {
            assert_eq!(
                Grammar::from_path(std::path::Path::new(name)),
                grammar,
                "{name}"
            );
        }
    }

    #[test]
    fn walks_share_json_and_jsonc_asymmetrically() {
        use std::path::Path;
        let reads = |anchor: &str, path: &str| {
            let grammar = Grammar::from_path(Path::new(anchor)).unwrap();
            grammar.walk_reads(Path::new(anchor), Path::new(path))
        };
        for anchor in ["package.json", "tsconfig.json", ".eslintrc.json"] {
            assert!(reads(anchor, "b.json"), "{anchor}");
            assert!(reads(anchor, "jsconfig.json"), "{anchor}");
            assert!(!reads(anchor, "x.jsonc"), "{anchor}");
            assert!(!reads(anchor, "a.code-workspace"), "{anchor}");
            assert!(!reads(anchor, "a.yaml"), "{anchor}");
        }
        for anchor in ["x.jsonc", "a.code-workspace"] {
            for path in ["b.json", "tsconfig.json", "y.jsonc", "b.code-workspace"] {
                assert!(reads(anchor, path), "{anchor} {path}");
            }
            assert!(!reads(anchor, "a.yaml"), "{anchor}");
        }
        assert!(reads("a.yaml", "b.yml"));
        assert!(!reads("a.yaml", "b.json"));
    }

    #[test]
    fn javascript_name() {
        assert_eq!(Grammar::JavaScript.name(), "JavaScript");
    }

    #[test]
    fn javascript_extensions() {
        assert_eq!(Grammar::from_extension("js"), Some(Grammar::JavaScript));
        assert_eq!(Grammar::from_extension("mjs"), Some(Grammar::JavaScript));
        assert_eq!(Grammar::from_extension("cjs"), Some(Grammar::JavaScript));
    }

    #[test]
    fn jsx_name() {
        assert_eq!(Grammar::Jsx.name(), "JSX");
    }

    #[test]
    fn jsx_extension() {
        assert_eq!(Grammar::from_extension("jsx"), Some(Grammar::Jsx));
    }

    #[test]
    fn csharp_name() {
        assert_eq!(Grammar::CSharp.name(), "C#");
    }

    #[test]
    fn csharp_extensions() {
        assert_eq!(Grammar::from_extension("cs"), Some(Grammar::CSharp));
        assert_eq!(Grammar::from_extension("csx"), Some(Grammar::CSharp));
    }

    #[test]
    fn python_name() {
        assert_eq!(Grammar::Python.name(), "Python");
    }

    #[test]
    fn python_extensions() {
        assert_eq!(Grammar::from_extension("py"), Some(Grammar::Python));
        assert_eq!(Grammar::from_extension("pyi"), Some(Grammar::Python));
        assert_eq!(Grammar::from_extension("pyw"), Some(Grammar::Python));
    }

    #[test]
    fn rust_name() {
        assert_eq!(Grammar::Rust.name(), "Rust");
    }

    #[test]
    fn rust_extensions() {
        assert_eq!(Grammar::from_extension("rs"), Some(Grammar::Rust));
    }

    #[test]
    fn go_name() {
        assert_eq!(Grammar::Go.name(), "Go");
    }

    #[test]
    fn go_extension() {
        assert_eq!(Grammar::from_extension("go"), Some(Grammar::Go));
    }

    #[test]
    fn elixir_name() {
        assert_eq!(Grammar::Elixir.name(), "Elixir");
    }

    #[test]
    fn elixir_extensions() {
        assert_eq!(Grammar::from_extension("ex"), Some(Grammar::Elixir));
        assert_eq!(Grammar::from_extension("exs"), Some(Grammar::Elixir));
        assert_eq!(Grammar::from_extension("leex"), Some(Grammar::Elixir));
    }

    #[test]
    fn java_name() {
        assert_eq!(Grammar::Java.name(), "Java");
    }

    #[test]
    fn java_extensions() {
        assert_eq!(Grammar::from_extension("java"), Some(Grammar::Java));
    }

    #[test]
    fn kotlin_name() {
        assert_eq!(Grammar::Kotlin.name(), "Kotlin");
    }

    #[test]
    fn kotlin_extensions() {
        assert_eq!(Grammar::from_extension("kt"), Some(Grammar::Kotlin));
        assert_eq!(Grammar::from_extension("kts"), Some(Grammar::Kotlin));
    }

    #[test]
    fn ruby_name() {
        assert_eq!(Grammar::Ruby.name(), "Ruby");
    }

    #[test]
    fn ruby_extensions() {
        assert_eq!(Grammar::from_extension("rb"), Some(Grammar::Ruby));
        assert_eq!(Grammar::from_extension("rake"), Some(Grammar::Ruby));
        assert_eq!(Grammar::from_extension("gemspec"), Some(Grammar::Ruby));
        assert_eq!(Grammar::from_extension("ru"), Some(Grammar::Ruby));
        assert_eq!(Grammar::from_extension("erb"), Some(Grammar::Ruby));
    }

    #[test]
    fn sql_name() {
        assert_eq!(Grammar::Sql.name(), "SQL");
    }

    #[test]
    fn sql_extensions() {
        assert_eq!(Grammar::from_extension("sql"), Some(Grammar::Sql));
        assert_eq!(Grammar::from_extension("psql"), Some(Grammar::Sql));
        assert_eq!(Grammar::from_extension("pgsql"), Some(Grammar::Sql));
        assert_eq!(Grammar::from_extension("mysql"), Some(Grammar::Sql));
    }

    #[test]
    fn c_name() {
        assert_eq!(Grammar::C.name(), "C");
    }

    #[test]
    fn c_extensions() {
        assert_eq!(Grammar::from_extension("c"), Some(Grammar::C));
        assert_eq!(Grammar::from_extension("h"), Some(Grammar::C));
    }

    #[test]
    fn cpp_name() {
        assert_eq!(Grammar::Cpp.name(), "C++");
    }

    #[test]
    fn cpp_extensions() {
        assert_eq!(Grammar::from_extension("cpp"), Some(Grammar::Cpp));
        assert_eq!(Grammar::from_extension("cxx"), Some(Grammar::Cpp));
        assert_eq!(Grammar::from_extension("cc"), Some(Grammar::Cpp));
        assert_eq!(Grammar::from_extension("hpp"), Some(Grammar::Cpp));
        assert_eq!(Grammar::from_extension("hxx"), Some(Grammar::Cpp));
        assert_eq!(Grammar::from_extension("hh"), Some(Grammar::Cpp));
    }

    #[test]
    fn php_name() {
        assert_eq!(Grammar::Php.name(), "PHP");
    }

    #[test]
    fn php_extensions() {
        assert_eq!(Grammar::from_extension("php"), Some(Grammar::Php));
        assert_eq!(Grammar::from_extension("phtml"), Some(Grammar::Php));
        assert_eq!(Grammar::from_extension("php3"), Some(Grammar::Php));
        assert_eq!(Grammar::from_extension("php4"), Some(Grammar::Php));
        assert_eq!(Grammar::from_extension("php5"), Some(Grammar::Php));
        assert_eq!(Grammar::from_extension("php7"), Some(Grammar::Php));
        assert_eq!(Grammar::from_extension("php8"), Some(Grammar::Php));
    }

    #[test]
    fn scala_name() {
        assert_eq!(Grammar::Scala.name(), "Scala");
    }

    #[test]
    fn scala_extensions() {
        assert_eq!(Grammar::from_extension("scala"), Some(Grammar::Scala));
        assert_eq!(Grammar::from_extension("sc"), Some(Grammar::Scala));
    }

    #[test]
    fn dart_extension_dart() {
        assert_eq!(Grammar::from_extension("dart"), Some(Grammar::Dart));
    }

    #[test]
    fn dart_name() {
        assert_eq!(Grammar::Dart.name(), "Dart");
    }

    #[test]
    fn bash_extension_sh() {
        assert_eq!(Grammar::from_extension("sh"), Some(Grammar::Bash));
    }

    #[test]
    fn bash_extension_bash() {
        assert_eq!(Grammar::from_extension("bash"), Some(Grammar::Bash));
    }

    #[test]
    fn bash_name() {
        assert_eq!(Grammar::Bash.name(), "Bash");
    }

    #[test]
    fn grammar_all_returns_all_variants() {
        let all = Grammar::all();
        // Hardcoded count guards against accidental variant removal.
        // Update this constant when intentionally adding/removing Grammar variants.
        assert_eq!(
            all.len(),
            39,
            "Grammar count changed — update this constant if intentional"
        );
        let mut seen = std::collections::HashSet::new();
        for g in all {
            assert!(seen.insert(g), "duplicate variant: {:?}", g);
        }
        assert!(seen.contains(&Grammar::TypeScript));
        assert!(seen.contains(&Grammar::Tsx));
        // Spot-check a historically-tricky variant is present
    }

    #[test]
    fn grammar_all_extensions_includes_known() {
        let exts = Grammar::all_extensions();
        assert!(exts.contains(&"ts"), "should include TypeScript extensions");
        assert!(exts.contains(&"py"), "should include Python extensions");
        assert!(exts.contains(&"rs"), "should include Rust extensions");
        assert!(exts.contains(&"go"), "should include Go extensions");
        assert!(
            exts.len() >= 50,
            "should have many extensions, got {}",
            exts.len()
        );
    }

    /// Table-driven over every variant: `name()` must be non-empty and unique,
    /// because names are the registry keys `language_for_extension` and the
    /// operation dispatch resolve by. Exercises every `name()` match arm.
    #[test]
    fn every_grammar_name_is_non_empty_and_unique() {
        let mut seen = std::collections::HashMap::new();
        for variant in Grammar::all() {
            let name = variant.name();
            assert!(!name.is_empty(), "{variant:?}.name() must not be empty");
            if let Some(previous) = seen.insert(name.to_string(), variant) {
                panic!("duplicate grammar name {name:?} shared by {previous:?} and {variant:?}");
            }
        }
    }

    /// Table-driven over every advertised extension: `from_extension` must
    /// resolve it to a grammar that itself claims it. Exercises every
    /// reachable `from_extension` match arm and pins the mapping's symmetry.
    #[test]
    fn every_advertised_extension_resolves_to_a_grammar_that_claims_it() {
        for ext in Grammar::all_extensions() {
            let resolved = Grammar::from_extension(ext)
                .unwrap_or_else(|| panic!("extension {ext:?} resolves to no grammar"));
            assert!(
                resolved.language_support().extensions().contains(&ext),
                "extension {ext:?} resolves to {resolved:?}, which does not advertise it"
            );
        }
    }

    #[test]
    fn language_support_returns_correct_grammar() {
        for variant in Grammar::all() {
            let support = variant.language_support();
            assert_eq!(
                support.grammar(),
                variant,
                "{:?}.language_support().grammar() should return {:?}",
                variant,
                variant
            );
        }
    }

    #[test]
    fn language_support_ts_language_is_valid() {
        for variant in Grammar::all() {
            let support = variant.language_support();
            let lang = support.ts_language();
            let mut parser = tree_sitter::Parser::new();
            assert!(
                parser.set_language(&lang).is_ok(),
                "{:?}.language_support().ts_language() should be a valid language",
                variant
            );
        }
    }

    #[test]
    fn all_includes_tsx_and_typescript_as_separate_variants() {
        let all = Grammar::all();
        assert!(
            all.contains(&Grammar::Tsx),
            "Grammar::all() must include Tsx"
        );
        assert!(
            all.contains(&Grammar::TypeScript),
            "Grammar::all() must include TypeScript"
        );
        // Verify they are counted as two separate entries
        let tsx_count = all.iter().filter(|&&g| g == Grammar::Tsx).count();
        let ts_count = all.iter().filter(|&&g| g == Grammar::TypeScript).count();
        assert_eq!(tsx_count, 1, "Tsx must appear exactly once");
        assert_eq!(ts_count, 1, "TypeScript must appear exactly once");
    }

    #[test]
    fn query_dir_matches_actual_directory() {
        let cases = [
            // Non-obvious special cases: name() differs from what naive lowercasing would give
            (Grammar::TypeScript, "typescript"),
            (Grammar::Tsx, "tsx"),
            (Grammar::JavaScript, "javascript"),
            (Grammar::CSharp, "csharp"),
            (Grammar::Cpp, "cpp"),
            (Grammar::HclTerraform, "hcl_terraform"),
            // Obvious cases: verify that common grammars map to the expected directory name
            (Grammar::Go, "go"),
            (Grammar::Rust, "rust"),
            (Grammar::Python, "python"),
            (Grammar::Java, "java"),
            (Grammar::Ruby, "ruby"),
        ];
        for (grammar, expected) in cases {
            assert_eq!(
                grammar.query_dir(),
                expected,
                "grammar {:?} should map to query dir {:?}",
                grammar,
                expected
            );
        }
    }
}
