//! Language-specific keyword detection.

/// Checks if a name is a C keyword.
///
/// Includes all C89/C90 and C99 reserved keywords.
pub fn is_c_keyword(name: &str) -> bool {
    matches!(
        name,
        "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "int"
            | "long"
            | "register"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
            // C99 keywords
            | "inline"
            | "restrict"
            | "_Bool"
            | "_Complex"
            | "_Imaginary"
    )
}

/// Checks if a name is a C++ keyword.
///
/// Includes all C keywords (which are also C++ keywords) plus C++-specific keywords.
/// Covers C++98 through C++23.
pub fn is_cpp_keyword(name: &str) -> bool {
    // C keywords are also C++ keywords
    if is_c_keyword(name) {
        return true;
    }

    matches!(
        name,
        // C++98 keywords
        "asm"
            | "bool"
            | "catch"
            | "class"
            | "const_cast"
            | "delete"
            | "dynamic_cast"
            | "explicit"
            | "false"
            | "friend"
            | "mutable"
            | "namespace"
            | "new"
            | "operator"
            | "private"
            | "protected"
            | "public"
            | "reinterpret_cast"
            | "static_cast"
            | "template"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeid"
            | "typename"
            | "using"
            | "virtual"
            | "wchar_t"
            // C++11 keywords
            | "nullptr"
            | "constexpr"
            | "decltype"
            | "noexcept"
            | "override"
            | "final"
            // C++20 keywords
            | "concept"
            | "requires"
            | "co_await"
            | "co_return"
            | "co_yield"
            // C++23 keywords
            | "import"
            | "module"
            | "export"
    )
}

/// Checks if a name is a TypeScript keyword.
///
/// Includes all ECMAScript reserved words, TypeScript strict-mode reserved words,
/// and TypeScript contextual keywords that should not be used as identifiers.
pub fn is_typescript_keyword(name: &str) -> bool {
    matches!(
        name,
        // ECMAScript reserved words
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            // Strict mode reserved words
            | "as"
            | "implements"
            | "interface"
            | "let"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
            | "yield"
            // TypeScript-specific keywords
            | "any"
            | "boolean"
            | "constructor"
            | "declare"
            | "get"
            | "module"
            | "require"
            | "number"
            | "set"
            | "string"
            | "symbol"
            | "type"
            | "from"
            | "of"
            | "namespace"
            | "async"
            | "await"
            // TypeScript contextual keywords
            | "abstract"
            | "override"
            | "readonly"
            | "satisfies"
            | "infer"
            | "keyof"
            | "unique"
            | "asserts"
            | "is"
            | "out"
    )
}

/// Checks if a name is a JavaScript keyword.
pub fn is_javascript_keyword(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            // Strict mode reserved
            | "as"
            | "implements"
            | "interface"
            | "let"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
            | "yield"
            | "async"
            | "await"
    )
}

/// Checks if a name is a Ruby keyword.
pub fn is_ruby_keyword(name: &str) -> bool {
    matches!(
        name,
        "BEGIN"
            | "END"
            | "alias"
            | "and"
            | "begin"
            | "break"
            | "case"
            | "class"
            | "def"
            | "defined?"
            | "do"
            | "else"
            | "elsif"
            | "end"
            | "ensure"
            | "false"
            | "for"
            | "if"
            | "in"
            | "module"
            | "next"
            | "nil"
            | "not"
            | "or"
            | "redo"
            | "rescue"
            | "retry"
            | "return"
            | "self"
            | "super"
            | "then"
            | "true"
            | "undef"
            | "unless"
            | "until"
            | "when"
            | "while"
            | "yield"
            | "loop"
            | "__ENCODING__"
            | "__FILE__"
            | "__LINE__"
            | "__callee__"
            | "__method__"
    )
}

/// Checks if a name is an R keyword.
pub fn is_r_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "else"
            | "repeat"
            | "while"
            | "function"
            | "for"
            | "in"
            | "next"
            | "break"
            | "TRUE"
            | "FALSE"
            | "NULL"
            | "Inf"
            | "NaN"
            | "NA"
            | "NA_integer_"
            | "NA_real_"
            | "NA_complex_"
            | "NA_character_"
            | "return"
            | "library"
            | "require"
    )
}

/// Checks if a name is a Go keyword.
///
/// Includes all Go keywords and predeclared identifiers from the Go specification.
/// See https://go.dev/ref/spec#Keywords
pub fn is_go_keyword(name: &str) -> bool {
    matches!(
        name,
        // Go keywords
        "break"
            | "case"
            | "chan"
            | "const"
            | "continue"
            | "default"
            | "defer"
            | "else"
            | "fallthrough"
            | "for"
            | "func"
            | "go"
            | "goto"
            | "if"
            | "import"
            | "interface"
            | "map"
            | "package"
            | "range"
            | "return"
            | "select"
            | "struct"
            | "switch"
            | "type"
            | "var"
            // Types and values (predeclared identifiers that act like keywords)
            | "bool"
            | "byte"
            | "complex64"
            | "complex128"
            | "error"
            | "float32"
            | "float64"
            | "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "rune"
            | "string"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uintptr"
            | "true"
            | "false"
            | "iota"
            | "nil"
            // Functions
            | "append"
            | "cap"
            | "close"
            | "complex"
            | "copy"
            | "delete"
            | "imag"
            | "len"
            | "make"
            | "new"
            | "panic"
            | "print"
            | "println"
            | "real"
            | "recover"
    )
}

/// Checks if a name is a Rust keyword.
///
/// Includes all strict keywords (2021 edition), reserved keywords,
/// and weak keywords that should not be used as identifiers in most contexts.
pub fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        // Strict keywords (2021 edition)
        "as"
            | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "union"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            // Reserved keywords
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // C keyword tests
    #[test]
    fn test_c_keywords_core() {
        let keywords = [
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
            "else", "enum", "extern", "float", "for", "goto", "if", "int", "long", "register",
            "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
            "union", "unsigned", "void", "volatile", "while",
        ];
        for keyword in keywords {
            assert!(
                is_c_keyword(keyword),
                "Expected '{}' to be a C keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_c99_keywords() {
        let c99_keywords = ["inline", "restrict", "_Bool", "_Complex", "_Imaginary"];
        for keyword in c99_keywords {
            assert!(
                is_c_keyword(keyword),
                "Expected '{}' (C99) to be a C keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_c_non_keywords() {
        let non_keywords = [
            "myFunction",
            "variable",
            "data",
            "result",
            "value",
            "foo",
            "bar",
            "baz",
            "helper",
            "util",
            "Struct",
            "_private",
            "camelCase",
            "snake_case",
            "CONSTANT",
            "PascalCase",
            "class",
            "template",
            "namespace",
        ];
        for name in non_keywords {
            assert!(
                !is_c_keyword(name),
                "Expected '{}' NOT to be a C keyword",
                name
            );
        }
    }

    // C++ keyword tests
    #[test]
    fn test_cpp_keywords_core() {
        let cpp_keywords = [
            "asm",
            "bool",
            "catch",
            "class",
            "const_cast",
            "delete",
            "dynamic_cast",
            "explicit",
            "false",
            "friend",
            "mutable",
            "namespace",
            "new",
            "operator",
            "private",
            "protected",
            "public",
            "reinterpret_cast",
            "static_cast",
            "template",
            "this",
            "throw",
            "true",
            "try",
            "typeid",
            "typename",
            "using",
            "virtual",
            "wchar_t",
        ];
        for keyword in cpp_keywords {
            assert!(
                is_cpp_keyword(keyword),
                "Expected '{}' to be a C++ keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_cpp11_keywords() {
        let cpp11_keywords = [
            "nullptr",
            "constexpr",
            "decltype",
            "noexcept",
            "override",
            "final",
        ];
        for keyword in cpp11_keywords {
            assert!(
                is_cpp_keyword(keyword),
                "Expected '{}' (C++11) to be a C++ keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_cpp20_keywords() {
        let cpp20_keywords = ["concept", "requires", "co_await", "co_return", "co_yield"];
        for keyword in cpp20_keywords {
            assert!(
                is_cpp_keyword(keyword),
                "Expected '{}' (C++20) to be a C++ keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_cpp23_keywords() {
        let cpp23_keywords = ["import", "module"];
        for keyword in cpp23_keywords {
            assert!(
                is_cpp_keyword(keyword),
                "Expected '{}' (C++23) to be a C++ keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_cpp_includes_c_keywords() {
        // All C keywords should also be C++ keywords
        let c_keywords = [
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
            "else", "enum", "extern", "float", "for", "goto", "if", "int", "long", "register",
            "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
            "union", "unsigned", "void", "volatile", "while",
        ];
        for keyword in c_keywords {
            assert!(
                is_cpp_keyword(keyword),
                "Expected C keyword '{}' to also be a C++ keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_cpp_non_keywords() {
        let non_keywords = [
            "myFunction",
            "variable",
            "data",
            "result",
            "value",
            "foo",
            "bar",
            "baz",
            "helper",
            "util",
            "Struct",
            "_private",
            "camelCase",
            "snake_case",
            "CONSTANT",
            "PascalCase",
            "def",
        ];
        for name in non_keywords {
            assert!(
                !is_cpp_keyword(name),
                "Expected '{}' NOT to be a C++ keyword",
                name
            );
        }
    }

    // Ruby keyword tests

    #[test]
    fn test_ruby_keywords_core() {
        let keywords = [
            "begin", "end", "class", "module", "def", "if", "unless", "while", "until", "for",
            "case", "when", "return", "yield", "self", "super", "true", "false", "nil", "rescue",
            "ensure",
        ];
        for keyword in keywords {
            assert!(
                is_ruby_keyword(keyword),
                "Expected '{}' to be a Ruby keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_ruby_non_keywords() {
        let non_keywords = [
            "myFunction",
            "variable",
            "data",
            "result",
            "value",
            "foo",
            "bar",
            "baz",
            "helper",
            "util",
            "Class",
            "_private",
            "$special",
            "camelCase",
            "snake_case",
            "CONSTANT",
            "PascalCase",
        ];
        for name in non_keywords {
            assert!(
                !is_ruby_keyword(name),
                "Expected '{}' NOT to be a Ruby keyword",
                name
            );
        }
    }

    #[test]
    fn test_ruby_soft_keywords() {
        // These are context-sensitive keywords but still reserved
        let soft_keywords = [
            "__ENCODING__",
            "__FILE__",
            "__LINE__",
            "__callee__",
            "__method__",
        ];
        for keyword in soft_keywords {
            assert!(
                is_ruby_keyword(keyword),
                "Expected '{}' (soft keyword) to be detected",
                keyword
            );
        }
    }

    // TypeScript keyword tests

    #[test]
    fn test_typescript_keywords_reserved() {
        let keywords = [
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "enum",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "new",
            "null",
            "return",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "var",
            "void",
            "while",
            "with",
        ];
        for keyword in keywords {
            assert!(
                is_typescript_keyword(keyword),
                "Expected '{}' to be a TypeScript keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_typescript_strict_mode_keywords() {
        let keywords = [
            "as",
            "implements",
            "interface",
            "let",
            "package",
            "private",
            "protected",
            "public",
            "static",
            "yield",
        ];
        for keyword in keywords {
            assert!(
                is_typescript_keyword(keyword),
                "Expected '{}' to be a TypeScript keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_typescript_specific_keywords() {
        let keywords = [
            "any",
            "boolean",
            "constructor",
            "declare",
            "get",
            "module",
            "require",
            "number",
            "set",
            "string",
            "symbol",
            "type",
            "from",
            "of",
            "namespace",
            "async",
            "await",
        ];
        for keyword in keywords {
            assert!(
                is_typescript_keyword(keyword),
                "Expected '{}' to be a TypeScript keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_typescript_contextual_keywords() {
        let contextual = [
            "abstract",
            "override",
            "readonly",
            "satisfies",
            "infer",
            "keyof",
            "unique",
            "asserts",
            "is",
            "out",
        ];
        for keyword in contextual {
            assert!(
                is_typescript_keyword(keyword),
                "Expected contextual keyword '{}' to be detected",
                keyword
            );
        }
    }

    #[test]
    fn test_typescript_non_keywords() {
        let non_keywords = [
            "myVariable",
            "foo",
            "bar",
            "handler",
            "config",
            "Component",
            "useState",
            "useEffect",
            "React",
            "Promise",
        ];
        for name in non_keywords {
            assert!(
                !is_typescript_keyword(name),
                "Expected '{}' NOT to be a TypeScript keyword",
                name
            );
        }
    }

    // JavaScript keyword tests

    #[test]
    fn test_javascript_keywords() {
        let keywords = [
            "break", "class", "const", "function", "if", "return", "var", "async", "await", "let",
            "yield",
        ];
        for keyword in keywords {
            assert!(
                is_javascript_keyword(keyword),
                "Expected '{}' to be a JavaScript keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_javascript_non_keywords() {
        let non_keywords = ["myFunction", "handler", "React", "useState"];
        for name in non_keywords {
            assert!(
                !is_javascript_keyword(name),
                "Expected '{}' NOT to be a JavaScript keyword",
                name
            );
        }
    }

    #[test]
    fn test_typescript_has_more_keywords_than_javascript() {
        // TypeScript-specific keywords that are NOT JavaScript keywords
        let ts_only = [
            "abstract",
            "readonly",
            "satisfies",
            "keyof",
            "infer",
            "any",
            "number",
            "string",
        ];
        for keyword in ts_only {
            assert!(
                is_typescript_keyword(keyword),
                "'{}' should be TypeScript keyword",
                keyword
            );
            assert!(
                !is_javascript_keyword(keyword),
                "'{}' should NOT be JavaScript keyword",
                keyword
            );
        }
    }

    // Rust keyword tests

    #[test]
    fn test_rust_strict_keywords() {
        let keywords = [
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "union", "unsafe", "use", "where", "while",
        ];
        for keyword in keywords {
            assert!(
                is_rust_keyword(keyword),
                "Expected '{}' to be a Rust keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_rust_reserved_keywords() {
        let reserved = [
            "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try",
            "typeof", "unsized", "virtual", "yield",
        ];
        for keyword in reserved {
            assert!(
                is_rust_keyword(keyword),
                "Expected reserved keyword '{}' to be detected",
                keyword
            );
        }
    }

    #[test]
    fn test_rust_non_keywords() {
        let non_keywords = [
            "my_function",
            "variable",
            "data",
            "result",
            "value",
            "foo",
            "bar",
            "baz",
            "helper",
            "util",
            "String",
            "Vec",
            "Option",
            "Result",
            "HashMap",
            "_private",
            "snake_case",
            "CONSTANT",
            "PascalCase",
            "i32",
            "u64",
            "f64",
            "bool",
            "usize",
        ];
        for name in non_keywords {
            assert!(
                !is_rust_keyword(name),
                "Expected '{}' NOT to be a Rust keyword",
                name
            );
        }
    }

    #[test]
    fn test_rust_keywords_distinct_from_types() {
        // Built-in type names like i32, u64, bool, str are NOT keywords in Rust
        // They are just identifiers in the standard prelude
        let type_names = ["i32", "u64", "f64", "bool", "usize", "isize", "str", "char"];
        for name in type_names {
            assert!(
                !is_rust_keyword(name),
                "Built-in type '{}' should NOT be a keyword",
                name
            );
        }
    }

    // Go keyword tests

    #[test]
    fn test_go_keywords_core() {
        let keywords = [
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
        ];
        for keyword in keywords {
            assert!(
                is_go_keyword(keyword),
                "Expected '{}' to be a Go keyword",
                keyword
            );
        }
    }

    #[test]
    fn test_go_predeclared_identifiers() {
        // Predeclared identifiers that act like keywords
        let predeclared = [
            "bool",
            "byte",
            "complex64",
            "complex128",
            "error",
            "float32",
            "float64",
            "int",
            "int8",
            "int16",
            "int32",
            "int64",
            "rune",
            "string",
            "uint",
            "uint8",
            "uint16",
            "uint32",
            "uint64",
            "uintptr",
            "true",
            "false",
            "iota",
            "nil",
        ];
        for name in predeclared {
            assert!(
                is_go_keyword(name),
                "Expected predeclared identifier '{}' to be detected",
                name
            );
        }
    }

    #[test]
    fn test_go_builtin_functions() {
        // Built-in functions that should not be used as identifiers
        let builtins = [
            "append", "cap", "close", "complex", "copy", "delete", "imag", "len", "make", "new",
            "panic", "print", "println", "real", "recover",
        ];
        for name in builtins {
            assert!(
                is_go_keyword(name),
                "Expected built-in function '{}' to be detected",
                name
            );
        }
    }

    #[test]
    fn test_go_non_keywords() {
        let non_keywords = [
            "myFunction",
            "variable",
            "data",
            "result",
            "value",
            "foo",
            "bar",
            "baz",
            "helper",
            "util",
            "Struct",
            "_private",
            "camelCase",
            "snake_case",
            "CONSTANT",
            "PascalCase",
            "MyType",
            "httpHandler",
            "JSONEncoder",
        ];
        for name in non_keywords {
            assert!(
                !is_go_keyword(name),
                "Expected '{}' NOT to be a Go keyword",
                name
            );
        }
    }
}
