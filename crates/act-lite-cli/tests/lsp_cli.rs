use act_lite_lsp::{LspClient, LspClientTrait, LspServer};
use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// One workspace per server. `lib` defines `compute` (its first occurrence
/// in `lib` is the name in the declaration) and `Shape`; `main` defines
/// `invoke`, which uses `compute` on the first line holding `use_needle`;
/// `implementor` is the (file, text) on the line that implements `Shape`.
/// `{root}` in a file is replaced by the workspace path.
struct Fixture {
    files: &'static [(&'static str, &'static str)],
    lib: &'static str,
    main: &'static str,
    use_needle: &'static str,
    implementor: Option<(&'static str, &'static str)>,
    calls: bool,
}

const TS: Fixture = Fixture {
    files: &[
        (
            "tsconfig.json",
            "{\"compilerOptions\": {\"strict\": true}}\n",
        ),
        (
            "lib.ts",
            "export function compute(x: number): number {\n  return x * 2;\n}\n\n\
             export interface Shape {\n  area(): number;\n}\n\n\
             export class Square implements Shape {\n  area(): number {\n    return 4;\n  }\n}\n",
        ),
        (
            "main.ts",
            "import { compute } from \"./lib\";\n\n\
             export function invoke(): number {\n  return compute(21);\n}\n",
        ),
    ],
    lib: "lib.ts",
    main: "main.ts",
    use_needle: "compute(",
    implementor: Some(("lib.ts", "class Square")),
    calls: true,
};

const CSHARP: Fixture = Fixture {
    files: &[
        (
            "Fixture.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    \
             <TargetFramework>net8.0</TargetFramework>\n  </PropertyGroup>\n</Project>\n",
        ),
        (
            "Lib.cs",
            "public static class Lib\n{\n    public static int compute(int x)\n    {\n        \
             return x * 2;\n    }\n}\n\npublic interface Shape\n{\n    int Area();\n}\n\n\
             public class Square : Shape\n{\n    public int Area() => 4;\n}\n",
        ),
        (
            "App.cs",
            "public static class App\n{\n    public static int invoke()\n    {\n        \
             return Lib.compute(21);\n    }\n}\n",
        ),
    ],
    lib: "Lib.cs",
    main: "App.cs",
    use_needle: "compute(",
    implementor: Some(("Lib.cs", "class Square")),
    calls: true,
};

const PYTHON: Fixture = Fixture {
    files: &[
        (
            "pyproject.toml",
            "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        ),
        (
            "lib.py",
            "from abc import ABC, abstractmethod\n\n\n\
             def compute(x: int) -> int:\n    return x * 2\n\n\n\
             class Shape(ABC):\n    @abstractmethod\n    def area(self) -> int: ...\n\n\n\
             class Square(Shape):\n    def area(self) -> int:\n        return 4\n",
        ),
        (
            "main.py",
            "from lib import compute\n\n\ndef invoke() -> int:\n    return compute(21)\n",
        ),
    ],
    lib: "lib.py",
    main: "main.py",
    use_needle: "compute(",
    implementor: Some(("lib.py", "class Square")),
    calls: true,
};

const RUST: Fixture = Fixture {
    files: &[
        (
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [lib]\npath = \"src/lib.rs\"\n\n[workspace]\n",
        ),
        (
            "src/lib.rs",
            "pub mod app;\n\npub fn compute(x: i32) -> i32 {\n    x * 2\n}\n\n\
             pub trait Shape {\n    fn area(&self) -> i32;\n}\n\npub struct Square;\n\n\
             impl Shape for Square {\n    fn area(&self) -> i32 {\n        4\n    }\n}\n",
        ),
        (
            "src/app.rs",
            "use crate::compute;\n\npub fn invoke() -> i32 {\n    compute(21)\n}\n",
        ),
    ],
    lib: "src/lib.rs",
    main: "src/app.rs",
    use_needle: "compute(",
    implementor: Some(("src/lib.rs", "impl Shape for Square")),
    calls: true,
};

const GO: Fixture = Fixture {
    files: &[
        ("go.mod", "module fixture\n\ngo 1.21\n"),
        (
            "lib.go",
            "package fixture\n\nfunc compute(x int) int {\n\treturn x * 2\n}\n\n\
             type Shape interface {\n\tArea() int\n}\n\ntype Square struct{}\n\n\
             func (Square) Area() int {\n\treturn 4\n}\n",
        ),
        (
            "main.go",
            "package fixture\n\nfunc invoke() int {\n\treturn compute(21)\n}\n",
        ),
    ],
    lib: "lib.go",
    main: "main.go",
    use_needle: "compute(",
    implementor: Some(("lib.go", "type Square")),
    calls: true,
};

const ELIXIR: Fixture = Fixture {
    files: &[
        (
            "mix.exs",
            "defmodule Fixture.MixProject do\n  use Mix.Project\n\n  def project do\n    \
             [app: :fixture, version: \"0.1.0\"]\n  end\nend\n",
        ),
        (
            "lib/lib.ex",
            "defmodule Fixture.Lib do\n  def compute(x) do\n    x * 2\n  end\nend\n\n\
             defprotocol Shape do\n  def area(shape)\nend\n\n\
             defmodule Square do\n  defstruct side: 2\nend\n\n\
             defimpl Shape, for: Square do\n  def area(_square), do: 4\nend\n",
        ),
        (
            "lib/app.ex",
            "defmodule Fixture.App do\n  def invoke do\n    Fixture.Lib.compute(21)\n  end\nend\n",
        ),
    ],
    lib: "lib/lib.ex",
    main: "lib/app.ex",
    use_needle: "compute(",
    implementor: Some(("lib/lib.ex", "defimpl Shape")),
    calls: true,
};

const JAVA: Fixture = Fixture {
    files: &[
        (
            "Lib.java",
            "public class Lib {\n    public static int compute(int x) {\n        \
             return x * 2;\n    }\n}\n\ninterface Shape {\n    int area();\n}\n\n\
             class Square implements Shape {\n    public int area() {\n        \
             return 4;\n    }\n}\n",
        ),
        (
            "App.java",
            "public class App {\n    public static int invoke() {\n        \
             return Lib.compute(21);\n    }\n}\n",
        ),
    ],
    lib: "Lib.java",
    main: "App.java",
    use_needle: "compute(",
    implementor: Some(("Lib.java", "class Square")),
    calls: true,
};

const KOTLIN: Fixture = Fixture {
    files: &[
        (
            "Lib.kt",
            "fun compute(x: Int): Int {\n    return x * 2\n}\n\n\
             interface Shape {\n    fun area(): Int\n}\n\n\
             class Square : Shape {\n    override fun area(): Int = 4\n}\n",
        ),
        ("App.kt", "fun invoke(): Int {\n    return compute(21)\n}\n"),
    ],
    lib: "Lib.kt",
    main: "App.kt",
    use_needle: "compute(",
    implementor: Some(("Lib.kt", "class Square")),
    calls: true,
};

const CPP: Fixture = Fixture {
    files: &[
        (
            "compile_commands.json",
            "[{\"directory\": \"{root}\", \"file\": \"{root}/main.cpp\", \
             \"arguments\": [\"clang++\", \"-std=c++17\", \"-c\", \"main.cpp\"]}]\n",
        ),
        (
            "lib.h",
            "#pragma once\n\ninline int compute(int x) {\n    return x * 2;\n}\n\n\
             struct Shape {\n    virtual ~Shape() = default;\n    virtual int area() const = 0;\n};\n\n\
             struct Square : Shape {\n    int area() const override { return 4; }\n};\n",
        ),
        (
            "main.cpp",
            "#include \"lib.h\"\n\nint invoke() {\n    return compute(21);\n}\n",
        ),
    ],
    lib: "lib.h",
    main: "main.cpp",
    use_needle: "compute(",
    implementor: Some(("lib.h", "struct Square")),
    calls: true,
};

const RUBY: Fixture = Fixture {
    files: &[
        (
            "lib.rb",
            "def compute(x)\n  x * 2\nend\n\nmodule Shape\n  def area\n    \
             raise NotImplementedError\n  end\nend\n\nclass Square\n  include Shape\n\n  \
             def area\n    4\n  end\nend\n",
        ),
        (
            "app.rb",
            "require_relative \"lib\"\n\ndef invoke\n  compute(21)\nend\n",
        ),
    ],
    lib: "lib.rb",
    main: "app.rb",
    use_needle: "compute(",
    implementor: Some(("lib.rb", "class Square")),
    calls: true,
};

const SQL: Fixture = Fixture {
    files: &[
        ("lib.sql", "CREATE TABLE compute (id INTEGER);\n"),
        ("app.sql", "SELECT id FROM compute;\n"),
    ],
    lib: "lib.sql",
    main: "app.sql",
    use_needle: "FROM compute",
    implementor: None,
    calls: false,
};

const PHP: Fixture = Fixture {
    files: &[
        (
            "lib.php",
            "<?php\n\nfunction compute(int $x): int\n{\n    return $x * 2;\n}\n\n\
             interface Shape\n{\n    public function area(): int;\n}\n\n\
             class Square implements Shape\n{\n    public function area(): int\n    {\n        \
             return 4;\n    }\n}\n",
        ),
        (
            "app.php",
            "<?php\n\nrequire_once __DIR__ . '/lib.php';\n\nfunction invoke(): int\n{\n    \
             return compute(21);\n}\n",
        ),
    ],
    lib: "lib.php",
    main: "app.php",
    use_needle: "compute(",
    implementor: Some(("lib.php", "class Square")),
    calls: true,
};

const SCALA: Fixture = Fixture {
    files: &[
        ("build.sbt", "scalaVersion := \"3.3.3\"\n"),
        (
            "src/main/scala/Lib.scala",
            "object Lib {\n  def compute(x: Int): Int = x * 2\n}\n\n\
             trait Shape {\n  def area: Int\n}\n\nclass Square extends Shape {\n  def area: Int = 4\n}\n",
        ),
        (
            "src/main/scala/App.scala",
            "object App {\n  def invoke(): Int = Lib.compute(21)\n}\n",
        ),
    ],
    lib: "src/main/scala/Lib.scala",
    main: "src/main/scala/App.scala",
    use_needle: "compute(",
    implementor: Some(("src/main/scala/Lib.scala", "class Square")),
    calls: true,
};

const SWIFT: Fixture = Fixture {
    files: &[
        (
            "Package.swift",
            "// swift-tools-version:5.7\nimport PackageDescription\n\n\
             let package = Package(\n    name: \"Fixture\",\n    \
             targets: [.target(name: \"Fixture\")]\n)\n",
        ),
        (
            "Sources/Fixture/Lib.swift",
            "func compute(_ x: Int) -> Int {\n    return x * 2\n}\n\n\
             protocol Shape {\n    func area() -> Int\n}\n\n\
             struct Square: Shape {\n    func area() -> Int {\n        return 4\n    }\n}\n",
        ),
        (
            "Sources/Fixture/App.swift",
            "func invoke() -> Int {\n    return compute(21)\n}\n",
        ),
    ],
    lib: "Sources/Fixture/Lib.swift",
    main: "Sources/Fixture/App.swift",
    use_needle: "compute(",
    implementor: Some(("Sources/Fixture/Lib.swift", "struct Square")),
    calls: true,
};

const DART: Fixture = Fixture {
    files: &[
        (
            "pubspec.yaml",
            "name: fixture\nenvironment:\n  sdk: \">=3.0.0 <4.0.0\"\n",
        ),
        (
            "lib/lib.dart",
            "int compute(int x) {\n  return x * 2;\n}\n\nabstract class Shape {\n  int area();\n}\n\n\
             class Square implements Shape {\n  @override\n  int area() => 4;\n}\n",
        ),
        (
            "lib/app.dart",
            "import 'lib.dart';\n\nint invoke() {\n  return compute(21);\n}\n",
        ),
    ],
    lib: "lib/lib.dart",
    main: "lib/app.dart",
    use_needle: "compute(",
    implementor: Some(("lib/lib.dart", "class Square")),
    calls: true,
};

fn fixture(server: LspServer) -> &'static Fixture {
    match server {
        LspServer::TypeScriptLanguageServer => &TS,
        LspServer::OmniSharp => &CSHARP,
        LspServer::Pyright => &PYTHON,
        LspServer::RustAnalyzer => &RUST,
        LspServer::Gopls => &GO,
        LspServer::ElixirLs => &ELIXIR,
        LspServer::Jdtls => &JAVA,
        LspServer::KotlinLanguageServer => &KOTLIN,
        LspServer::Clangd => &CPP,
        LspServer::RubyLsp => &RUBY,
        LspServer::Sqls => &SQL,
        LspServer::Intelephense => &PHP,
        LspServer::Metals => &SCALA,
        LspServer::SourceKitLsp => &SWIFT,
        LspServer::Dart => &DART,
    }
}

impl Fixture {
    fn text(&self, file: &str) -> &'static str {
        self.files
            .iter()
            .find(|(path, _)| *path == file)
            .unwrap_or_else(|| panic!("fixture has no {file}"))
            .1
    }

    /// Writes the fixture into a fresh directory; returns it with its
    /// canonical path.
    fn write(&self) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for (path, text) in self.files {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            let text = text.replace("{root}", root.to_str().unwrap());
            std::fs::write(file, text).unwrap();
        }
        (dir, root)
    }
}

/// 1-based (line, column) of the first occurrence of `needle` in `text`.
fn position(text: &str, needle: &str) -> (u32, u32) {
    text.lines()
        .enumerate()
        .find_map(|(i, line)| line.find(needle).map(|c| (i as u32 + 1, c as u32 + 1)))
        .unwrap_or_else(|| panic!("fixture text has no {needle:?}"))
}

#[derive(Clone, Copy, Debug)]
enum Op {
    References,
    Callers,
    Callees,
    Implementations,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::References => "references",
            Op::Callers => "callers",
            Op::Callees => "callees",
            Op::Implementations => "implementations",
        }
    }

    /// The `initialize` capability that advertises this operation.
    fn capability(self) -> &'static str {
        match self {
            Op::References => "referencesProvider",
            Op::Callers | Op::Callees => "callHierarchyProvider",
            Op::Implementations => "implementationProvider",
        }
    }
}

/// The server binaries `ACT_LITE_REQUIRE_LSP` names.
fn required_servers() -> Vec<String> {
    std::env::var("ACT_LITE_REQUIRE_LSP")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn on_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(binary).is_file()))
}

/// The server's `initialize` capabilities, from a live handshake with the
/// parameters act-lite itself sends. A server that does not start or does
/// not answer `initialize` fails the test.
fn capabilities(server: LspServer, root: &Path) -> Value {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = LspClient::spawn(
            server.binary_name().to_string(),
            server.config(root.to_path_buf()),
        )
        .await
        .unwrap_or_else(|e| panic!("{} did not start: {e}", server.binary_name()));
        let result = client
            .initialize(root.to_path_buf())
            .await
            .unwrap_or_else(|e| panic!("{} did not answer initialize: {e}", server.binary_name()));
        let _ = client.shutdown().await;
        serde_json::to_value(&result.capabilities).unwrap()
    })
}

fn advertises(capabilities: &Value, op: Op) -> bool {
    !matches!(
        capabilities.get(op.capability()),
        None | Some(Value::Null) | Some(Value::Bool(false))
    )
}

/// Runs `act-lite --workspace <root> query <args>`.
fn query(root: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("act-lite")
        .expect("the act-lite binary must be built")
        .current_dir(root)
        .arg("--workspace")
        .arg(root)
        .arg("query")
        .args(args)
        .timeout(Duration::from_secs(300))
        .output()
        .unwrap()
}

fn json(output: &std::process::Output, what: &str) -> Value {
    assert!(
        output.status.success(),
        "{what} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{what}: stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// (file, line) of a serialized `Location`.
fn file_line(location: &Value) -> (PathBuf, u64) {
    (
        PathBuf::from(location["file"].as_str().unwrap_or_default()),
        location["line"].as_u64().unwrap_or_default(),
    )
}

fn check(server: LspServer, op: Op) {
    let binary = server.binary_name();
    let required = required_servers();
    let unknown: Vec<&String> = required
        .iter()
        .filter(|b| !LspServer::ALL.iter().any(|s| s.binary_name() == b.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "ACT_LITE_REQUIRE_LSP names {unknown:?}, which are not server binaries in servers.rs"
    );
    if !required.iter().any(|b| b == binary) {
        eprintln!("skipped: ACT_LITE_REQUIRE_LSP does not name {binary}");
        return;
    }
    assert!(
        on_path(binary),
        "ACT_LITE_REQUIRE_LSP names {binary}, which is not on PATH"
    );
    let fx = fixture(server);
    let (_dir, root) = fx.write();
    let advertised = advertises(&capabilities(server, &root), op);
    let what = format!("{binary} {}", op.name());
    let lib = root.join(fx.lib);
    let lib_arg = lib.to_str().unwrap();
    let (decl_line, decl_column) = position(fx.text(fx.lib), "compute");
    let (use_line, _) = position(fx.text(fx.main), fx.use_needle);

    let output = match op {
        Op::References => query(
            &root,
            &[
                "references",
                "compute",
                "--file",
                lib_arg,
                "--line",
                &decl_line.to_string(),
                "--column",
                &decl_column.to_string(),
            ],
        ),
        Op::Callers => query(&root, &["callers", "compute", "--file", lib_arg]),
        Op::Callees => {
            let main = root.join(fx.main);
            query(
                &root,
                &["callees", "invoke", "--file", main.to_str().unwrap()],
            )
        }
        Op::Implementations => query(&root, &["implementations", "Shape", "--file", lib_arg]),
    };

    if let Op::References = op {
        // README: without a server answer, `references` falls back to
        // tree-sitter and says so.
        let source = if advertised { "lsp" } else { "tree-sitter" };
        let result = json(&output, &what);
        assert_eq!(result["source"], source, "{what}: {result}");
        let found: Vec<(PathBuf, u64)> = result["references"]
            .as_array()
            .unwrap_or_else(|| panic!("{what}: no references array in {result}"))
            .iter()
            .map(file_line)
            .collect();
        let declaration = (root.join(fx.lib), u64::from(decl_line));
        let call = (root.join(fx.main), u64::from(use_line));
        assert!(
            found.contains(&call),
            "{what}: expected {call:?} in {found:?}"
        );
        if matches!(server, LspServer::KotlinLanguageServer) {
            assert!(
                !found.contains(&declaration),
                "{what}: no longer reproduces; remove the limitation: {found:?}"
            );
        } else {
            assert!(
                found.contains(&declaration),
                "{what}: expected {declaration:?} in {found:?}"
            );
        }
        return;
    }

    if !advertised {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "{what}: not advertised, so it must fail; stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(!stderr.contains("panicked"), "{what}: {stderr}");
        assert!(
            stderr.contains(op.name()) && stderr.to_lowercase().contains("support"),
            "{what}: the error must say the server does not support the operation: {stderr}"
        );
        return;
    }

    assert!(
        fx.calls,
        "{what} is advertised, but the fixture has no call"
    );
    let result = json(&output, &what);
    match op {
        Op::References => unreachable!("references returned above"),
        Op::Callers | Op::Callees => {
            let (key, name, file) = match op {
                Op::Callers => ("callers", "invoke", fx.main),
                _ if matches!(server, LspServer::ElixirLs) => ("callees", "compute", fx.main),
                _ => ("callees", "compute", fx.lib),
            };
            let symbols = result[key]
                .as_array()
                .unwrap_or_else(|| panic!("{what}: no {key} array in {result}"));
            assert!(
                symbols.iter().any(|s| {
                    s["name"].as_str().unwrap_or_default().contains(name)
                        && file_line(&s["range"]["start"]).0 == root.join(file)
                }),
                "{what}: expected `{name}` in {file}, got {result}"
            );
        }
        Op::Implementations => {
            let (file, needle) = fx
                .implementor
                .unwrap_or_else(|| panic!("{what} is advertised, but the fixture has no Shape"));
            let expected = (
                root.join(file),
                u64::from(position(fx.text(file), needle).0),
            );
            let found: Vec<(PathBuf, u64)> = result["locations"]
                .as_array()
                .unwrap_or_else(|| panic!("{what}: no locations array in {result}"))
                .iter()
                .map(|r| file_line(&r["start"]))
                .collect();
            assert!(
                found.contains(&expected),
                "{what}: expected {expected:?} in {found:?}"
            );
        }
    }
}

macro_rules! lsp_tests {
    ($($(#[$meta:meta])* $name:ident: $server:ident, $op:ident;)*) => {
        $(
            $(#[$meta])*
            #[test]
            fn $name() {
                check(LspServer::$server, Op::$op);
            }
        )*
    };
}

lsp_tests! {
    typescript_references: TypeScriptLanguageServer, References;
    typescript_callers: TypeScriptLanguageServer, Callers;
    typescript_callees: TypeScriptLanguageServer, Callees;
    typescript_implementations: TypeScriptLanguageServer, Implementations;
    omnisharp_references: OmniSharp, References;
    omnisharp_callers: OmniSharp, Callers;
    omnisharp_callees: OmniSharp, Callees;
    omnisharp_implementations: OmniSharp, Implementations;
    pyright_references: Pyright, References;
    pyright_callers: Pyright, Callers;
    pyright_callees: Pyright, Callees;
    pyright_implementations: Pyright, Implementations;
    rust_analyzer_references: RustAnalyzer, References;
    rust_analyzer_callers: RustAnalyzer, Callers;
    rust_analyzer_callees: RustAnalyzer, Callees;
    rust_analyzer_implementations: RustAnalyzer, Implementations;
    gopls_references: Gopls, References;
    gopls_callers: Gopls, Callers;
    gopls_callees: Gopls, Callees;
    gopls_implementations: Gopls, Implementations;
    elixir_ls_references: ElixirLs, References;
    elixir_ls_callers: ElixirLs, Callers;
    elixir_ls_callees: ElixirLs, Callees;
    elixir_ls_implementations: ElixirLs, Implementations;
    jdtls_references: Jdtls, References;
    jdtls_callers: Jdtls, Callers;
    jdtls_callees: Jdtls, Callees;
    jdtls_implementations: Jdtls, Implementations;
    kotlin_references: KotlinLanguageServer, References;
    kotlin_callers: KotlinLanguageServer, Callers;
    kotlin_callees: KotlinLanguageServer, Callees;
    kotlin_implementations: KotlinLanguageServer, Implementations;
    clangd_references: Clangd, References;
    clangd_callers: Clangd, Callers;
    clangd_callees: Clangd, Callees;
    clangd_implementations: Clangd, Implementations;
    ruby_lsp_references: RubyLsp, References;
    ruby_lsp_callers: RubyLsp, Callers;
    ruby_lsp_callees: RubyLsp, Callees;
    ruby_lsp_implementations: RubyLsp, Implementations;
    sqls_references: Sqls, References;
    sqls_callers: Sqls, Callers;
    sqls_callees: Sqls, Callees;
    sqls_implementations: Sqls, Implementations;
    intelephense_references: Intelephense, References;
    intelephense_callers: Intelephense, Callers;
    intelephense_callees: Intelephense, Callees;
    intelephense_implementations: Intelephense, Implementations;
    metals_references: Metals, References;
    metals_callers: Metals, Callers;
    metals_callees: Metals, Callees;
    metals_implementations: Metals, Implementations;
    sourcekit_lsp_references: SourceKitLsp, References;
    sourcekit_lsp_callers: SourceKitLsp, Callers;
    sourcekit_lsp_callees: SourceKitLsp, Callees;
    sourcekit_lsp_implementations: SourceKitLsp, Implementations;
    dart_references: Dart, References;
    dart_callers: Dart, Callers;
    dart_callees: Dart, Callees;
    dart_implementations: Dart, Implementations;
}
