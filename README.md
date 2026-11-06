# act-lite

act-lite is an open-source code engine for AI agents from act101. It parses
source files with tree-sitter and answers structural questions without reading
whole files: a file's skeleton, its symbols, a repository outline, where a
symbol is defined, and where it is used. It also measures code health:
functions over size, nesting, branching or parameter thresholds, the files
where complexity concentrates, and duplicated code. It maps file
dependencies: the import edges of a scope, file-level cycles, coupling counts
per file, and the files that depend on a given file. It does not edit code. It also sequesters a project's secrets out of the working
tree and runs commands with them injected and redacted (`act-lite env`); the
only file it writes is the project's `.env`, and only the value tokens of the
lines it sequesters.

act-lite ships one binary, `act-lite`, with four commands:

- `act-lite query` runs one query and prints JSON.
- `act-lite analyze` runs one code-health or dependency analysis and prints
  JSON.
- `act-lite mcp serve` serves the same queries and analyses as MCP tools over
  stdio.
- `act-lite env` sequesters secrets, lists them by name, and runs commands
  with them injected and their output redacted.

## Languages

act-lite supports 40 languages:

Bash, C, C#, C++, CMake, CSS, Dart, Dockerfile, Elixir, Go, GraphQL,
HCL/Terraform, HTML, Java, JavaScript, JSX, JSON, JSONC, Julia, Kotlin, Lua,
Make, Markdown, Nix, PHP, Protocol Buffers, Python, R, Ruby, Rust, Scala, SCSS,
SQL, Swift, TOML, TSX, TypeScript, XML, YAML, Zig.

A file in any other language is refused with an error that names its
extension and lists these languages.

A file is recognized by its name first, then by its extension. The names are
exact and case-sensitive. The ten languages added after the first thirty are
recognized as follows; a file with any other name or extension stays in the
language it had, or stays unrecognized (`mytsconfig.json` is JSON,
`Makefile.bak` and `cmakelists.txt` are unrecognized):

| Language | File names | Extensions |
|---|---|---|
| JSONC | `tsconfig.json`, `jsconfig.json`, `tsconfig.*.json`, `jsconfig.*.json`, `devcontainer.json`, `.devcontainer.json`, `.eslintrc.json` | `.jsonc`, `.code-workspace` |
| XML | none | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.svg`, `.plist`, `.csproj`, `.fsproj`, `.vbproj`, `.props`, `.targets`, `.resx`, `.xaml` |
| GraphQL | none | `.graphql`, `.graphqls`, `.gql` |
| Protocol Buffers | none | `.proto` |
| Make | `Makefile`, `makefile`, `GNUmakefile` | `.mk`, `.mak` |
| CMake | `CMakeLists.txt` | `.cmake` |
| Zig | none | `.zig` |
| R | `.Rprofile` | `.R`, `.r` |
| Julia | none | `.jl` |
| Nix | none | `.nix` |

In `tsconfig.*.json` and `jsconfig.*.json`, `*` is one or more characters. Any
other `.json` file, such as `.vscode/settings.json`, stays JSON.

### Capabilities by language

| Language | `skeleton`, `symbols` | `definition` | `references` | `callers`, `callees`, `implementations` | `patterns` | `hotspots` | `clones` |
|---|---|---|---|---|---|---|---|
| Bash | yes | tree-sitter | name-level | not available | yes | yes | yes |
| C | yes | tree-sitter | language server | language server | yes | yes | yes |
| C# | yes | tree-sitter | language server | language server | yes | yes | yes |
| C++ | yes | tree-sitter | language server | language server | yes | yes | yes |
| CMake | yes | tree-sitter; case-sensitive names | name-level | not available | not applicable | not applicable | not applicable |
| CSS | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Dart | yes | tree-sitter | language server | language server | yes | yes | yes |
| Dockerfile | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Elixir | yes | tree-sitter | language server | language server | yes | yes | yes |
| Go | yes | tree-sitter | language server | language server | yes | yes | yes |
| GraphQL | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| HCL/Terraform | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| HTML | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Java | yes | tree-sitter | language server | language server | yes | yes | yes |
| JavaScript | yes | tree-sitter | language server | language server | yes | yes | yes |
| JSX | yes | tree-sitter | language server | language server | yes | yes | yes |
| JSON | yes | tree-sitter; keys without quotes | name-level | not available | not applicable | not applicable | not applicable |
| JSONC | yes | tree-sitter; keys without quotes | name-level | not available | not applicable | not applicable | not applicable |
| Julia | yes | tree-sitter; dotted names | name-level | not available | yes | yes | yes |
| Kotlin | yes | tree-sitter | language server | language server | yes | yes | yes |
| Lua | yes | tree-sitter | name-level | not available | yes | yes | yes |
| Make | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Markdown | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Nix | yes | tree-sitter; parameters, dotted attribute paths | name-level | not available | not applicable | not applicable | not applicable |
| PHP | yes | tree-sitter | language server | language server | yes | yes | yes |
| Protocol Buffers | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Python | yes | tree-sitter | language server | language server | yes | yes | yes |
| R | yes | tree-sitter; parameters | name-level | not available | yes | yes | yes |
| Ruby | yes | tree-sitter | language server | language server | yes | yes | yes |
| Rust | yes | tree-sitter | language server | language server | yes | yes | yes |
| Scala | yes | tree-sitter | language server | language server | yes | yes | yes |
| SCSS | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| SQL | yes | tree-sitter | language server | language server | not applicable | not applicable | not applicable |
| Swift | yes | tree-sitter | language server | language server | yes | yes | yes |
| TOML | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| TSX | yes | tree-sitter | language server | language server | yes | yes | yes |
| TypeScript | yes | tree-sitter | language server | language server | yes | yes | yes |
| XML | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| YAML | yes | tree-sitter | name-level | not available | not applicable | not applicable | not applicable |
| Zig | yes | tree-sitter; parameters are not declarations | name-level | not available | yes | yes | yes |

`skeleton`, `symbols`, `symbols-batch` and `repo-outline` work for every
language, and `definition` uses tree-sitter for every language. `references`
and the three call queries use a language server where the table says
`language server`; the table's `references` cell is `name-level` for a
language with no server. A language with a server falls back to a name-level
search when its server is not installed (see [`act-lite query`](#act-lite-query)).
`not available` means that the query fails with an error that names the
language and says that act-lite has no language server for it. The analyzer
columns say `yes` for the 23 languages the analyzers support and
`not applicable` for the 17 data, schema and markup formats (see
[Code-health analyzers](#code-health-analyzers)).

`definition` notes. JSON and JSONC keys are returned without their quotes. A
CMake name matches by exact text, so `ADD_TOOL(cli)` does not find
`function(add_tool …)`. Nix and Julia match a dotted name (`packages.default`,
`Base.show`) as a whole. A function parameter is a declaration in R and Nix; a
Zig parameter is not.

Examples for the added languages:

```bash
act-lite query skeleton Makefile                  # targets, variables, includes
act-lite query symbols schema.graphql             # types, fields, enum values
act-lite query references Order --file shop.proto --line 5 --column 9   # name matches
```

### Limits of the added languages

**JSONC.** `//` and `/* */` comments are comments: a key-like text inside one
is not reported. A trailing comma before `}` or `]` is not valid in the
grammar: the file parses with error recovery, `symbols` carries
`"parse_error": true`, and every key is still reported at its location. JSONC
answers like JSON for the same content: skeleton and symbols give the same
names, and the symbols names keep their quotes.

**XML.** An `element` in the skeleton is an element, and in the symbols an
element name is a `class` and an attribute name a `field`. There are no
build-system semantics: a Maven `<dependency>` or an MSBuild
`<PackageReference>` is an element like any other, no dependency is reported,
`${…}` properties are not expanded, parent POMs, imports and includes are not
followed, and namespace prefixes are not resolved. Text content, comments,
processing instructions, the XML declaration, `DOCTYPE` and internal DTD
declarations are not reported. act-lite reads UTF-8 only:
a binary property list (`bplist00`) or a UTF-16 file named `.plist` or `.xml`
is refused by `query` with the not-UTF-8 error, and `repo-outline --symbols`
lists it without symbols.

**GraphQL.** Types, interfaces, inputs, enums, unions, scalars, directives,
named operations and fragments are declarations; fields and enum values are
symbols. A schema extension without a body, such as
`extend schema @link(url: "x")` (the Apollo Federation header), is not
accepted by the grammar: its error recovery swallows the first definition
after it, and the result carries `parse_error: true`. With a body
(`extend schema @link(url: "x") { query: Q }`) the file parses cleanly. Not
reported: `schema { … }`, anonymous operations, field arguments, operation
variables and selections.

**Protocol Buffers.** Messages, enums, services and `rpc` methods are
declarations; fields and enum values are symbols; `import` and `package` are
skeleton entries. Not reported: `syntax`, `option`, `reserved`, and `extend`
blocks with the fields in them. A proto2 `group` is reported as a `struct`
(its message) and a `field` (the group name as written). A qualified type such
as `shop.v1.Order` is matched one identifier at a time, so `Order` there is a
reference to `Order`, and a `package` name has no references.

**Make.** Targets are functions and variable assignments are variables;
GNU special targets (`.PHONY`) and target-specific assignments are not
reported. Recipes are not run and not parsed as shell, variables are not
expanded, conditionals are not evaluated (an assignment in each branch of an
`ifeq` is reported once per branch), included files are not read, and targets
created by `$(eval …)` or by expanded variables are not reported.

**CMake.** Configuration is not run. Variables, generator expressions and
`if()` conditions are not evaluated: a command inside `if(WITH_TESTS)` is
reported whatever the option's value. `include()`d files and subdirectories
are not read, and a call such as `add_tool(cli)` is a use, not a target. The
built-in commands are recognized in any letter case, but name matches are
case-sensitive: `ADD_TOOL(cli)` is not a reference to `function(add_tool …)`.

**Zig.** tree-sitter-zig requires at least one member, so an empty
`struct {}`, `enum {}`, `union {}` or `opaque {}` parses with an error node:
navigation results carry `parse_error: true`, and an analyzer lists the file
in `parse_failures` and says `complete: false`. `comptime` is not evaluated:
a type built by a call (`const T = List(u8)`) is a `constant`, not a `struct`.
`usingnamespace` is not modeled. Parameters are not declarations.
`test "name" { … }` is a `function` in `skeleton` and `symbols`, named by the
string content (`test "point len"` is named `point len`); `test name { … }` (a
doctest) is a use of `name`, not a declaration.

**R.** A function is an assignment whose value is a `function` or `\(…)`
definition. Not modeled: `function(…) -> name`, assignments inside function
bodies other than functions, `assign()`, and classes made by calls
(`setClass`, `setRefClass` and `R6Class`).

**Julia.** Each method of a function is a separate entry, and multiple
dispatch is not resolved. `export` statements are not reported. Not modeled:
functions defined by macros or `@eval`, anonymous functions and `do` blocks
(the analyzers do measure them).

**Nix.** Results are structural, not evaluated: `//`, `rec`, `with`,
overlays, `lib.mkIf`, `lib.mkMerge` and the module system are not applied. A
binding under `mkIf false { … }` is reported, `a // b` reports the bindings
of both sides as written, an interpolated attribute name is reported as
written, and `imports = [ ./a.nix ]` is a binding whose list is not followed.
The kind comes from the syntax of the bound expression.

Every `references` answer for the ten added languages is a name match, as the
next section says; none of them has a language server.

## Install

Install the latest release with one command:

```bash
curl -fsSL https://raw.githubusercontent.com/act101-ai/act-lite/main/install.sh | sh
```

The script detects your platform, downloads the archive, checks its SHA-256
checksum, and installs `act-lite` into `~/.local/bin`. It never uses `sudo`.

- `ACT_LITE_VERSION` installs a given release instead of the latest, for
  example `ACT_LITE_VERSION=v0.1.0`.
- `ACT_LITE_INSTALL_DIR` sets the install directory (default `~/.local/bin`).

```bash
curl -fsSL https://raw.githubusercontent.com/act101-ai/act-lite/main/install.sh \
  | ACT_LITE_VERSION=v0.1.0 ACT_LITE_INSTALL_DIR=/usr/local/bin sh
```

You can also install by hand from a release archive, or build from source
with `cargo build`.

### From a release

Download the archive for your platform and its `.sha256` file from the
[releases page](https://github.com/act101-ai/act-lite/releases):

- `act-lite-<version>-x86_64-unknown-linux-gnu.tar.gz`: Linux x86_64
- `act-lite-<version>-aarch64-unknown-linux-gnu.tar.gz`: Linux aarch64
- `act-lite-<version>-aarch64-apple-darwin.tar.gz`: macOS on Apple silicon
- `act-lite-<version>-x86_64-apple-darwin.tar.gz`: macOS on Intel

`<version>` is the release tag (for example `v0.1.0`) and `<target>` is one
of the four names above. Check the archive, extract it, and copy `act-lite`
into a directory on your `PATH` (here `~/.local/bin`; add it to `PATH` if it
is not there):

```bash
shasum -a 256 -c act-lite-<version>-<target>.tar.gz.sha256
tar -xzf act-lite-<version>-<target>.tar.gz
mkdir -p ~/.local/bin
install -m 0755 act-lite-<version>-<target>/act-lite ~/.local/bin/
```

### Platforms

- Linux x86_64 and aarch64 with glibc 2.31 or later (Ubuntu 20.04, Debian 11
  and later).
- macOS 11 or later, on Apple silicon and Intel.

Windows is not supported.

### From source

Rust 1.90 or later:

```bash
cargo build --release -p act-lite-cli
# binary: target/release/act-lite
```

## `act-lite query`

```bash
act-lite query skeleton src/main.rs                 # declarations without bodies
act-lite query symbols src/main.rs                  # every symbol with kind and range
act-lite query symbol main --file src/main.rs       # one symbol
act-lite query symbols-batch --files a.ts,b.ts --kinds function
act-lite query repo-outline --depth 2 --max-files 200 --include '*.rs'
act-lite query definition add --file src/use.ts --line 3 --column 15
act-lite query references add --file src/lib.ts --line 1 --column 17
act-lite query callers add --file src/lib.ts        # needs a language server
act-lite query callees add --file src/lib.ts        # needs a language server
act-lite query implementations Shape --file src/shape.ts  # needs a language server
```

Global flags:

- `--format json|text` — compact JSON (default) or indented JSON.
- `--workspace <dir>` — the workspace root (default: the current directory).
  A relative file path resolves against it, not the current directory. The
  `references` fallback searches the files of the same language under it
  (JSON and JSONC share one search, below).
- `--disable-lsp` (or `ACT_LITE_DISABLE_LSP=1`) — never start a language server.

`ACT_LITE_DISABLE_LSP` and `ACT_LITE_INSTALL_LSP` take `1`, `true`, `yes`,
`on`, `y` or `t` (any case) as on, and `0`, `false`, `no`, `off`, `n` or `f`
as off.

`references` uses a language server when one is available. Without one it
falls back to a name-level tree-sitter search over files of the same
language, and the output says so with `"source": "tree-sitter"`. `callers`,
`callees` and `implementations` need a language server; when it is missing,
the error names the server binary, its install command and its homepage.

act-lite installs a language server only when `ACT_LITE_INSTALL_LSP=1` is
set. Then a query that would use a missing server runs the server's install
command (below) through `sh -c`; after 10 minutes it kills the shell and
every process the shell started. When the install fails, the hint says that
the command ran and why the server is still unavailable; the next `act-lite`
command or MCP server process runs the install again. Without
`ACT_LITE_INSTALL_LSP=1`, act-lite runs nothing: install the server yourself
and put it on your `PATH`.

Once per session — one `act-lite` command, or one `act-lite mcp serve`
process — the first `references` answer that falls back because its server
is not installed carries a hint with the server binary, its install command
and its homepage. The CLI prints it as one `hint:` line on stderr and leaves
the JSON unchanged; the MCP server adds it to the tool result, after the
name-level note that every fallback result carries. With
`--disable-lsp` or `ACT_LITE_DISABLE_LSP=1` there is no install hint.

| Server | Languages | Install command | Homepage |
| --- | --- | --- | --- |
| `typescript-language-server` | TypeScript, TSX, JavaScript, JSX | `npm install -g typescript-language-server typescript` | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) |
| `OmniSharp` | C# | `brew install omnisharp-roslyn` | [omnisharp-roslyn](https://github.com/OmniSharp/omnisharp-roslyn) |
| `pyright-langserver` | Python | `pip install pyright` | [pyright](https://github.com/microsoft/pyright) |
| `rust-analyzer` | Rust | `rustup component add rust-analyzer` | [rust-analyzer](https://rust-analyzer.github.io) |
| `gopls` | Go | `go install golang.org/x/tools/gopls@latest` | [gopls](https://pkg.go.dev/golang.org/x/tools/gopls) |
| `elixir-ls` | Elixir | `brew install elixir-ls` | [elixir-ls](https://github.com/elixir-lsp/elixir-ls) |
| `jdtls` | Java | `brew install jdtls` | [eclipse.jdt.ls](https://github.com/eclipse-jdtls/eclipse.jdt.ls) |
| `kotlin-language-server` | Kotlin | `brew install kotlin-language-server` | [kotlin-language-server](https://github.com/fwcd/kotlin-language-server) |
| `clangd` | C, C++ | `brew install llvm` | [clangd](https://clangd.llvm.org) |
| `ruby-lsp` | Ruby | `gem install ruby-lsp` | [ruby-lsp](https://github.com/Shopify/ruby-lsp) |
| `sqls` | SQL | `go install github.com/sqls-server/sqls@latest` | [sqls](https://github.com/sqls-server/sqls) |
| `intelephense` | PHP | `npm install -g intelephense` | [intelephense](https://intelephense.com) |
| `metals` | Scala | `cs install metals` | [metals](https://scalameta.org/metals) |
| `sourcekit-lsp` | Swift | `xcode-select --install` | [sourcekit-lsp](https://github.com/swiftlang/sourcekit-lsp) |
| `dart` | Dart | `brew install dart` | [dart](https://dart.dev/get-dart) |

Without a language server, queries match a symbol by name, not by scope: they
use the nearest declaration with that name, and YAML and JSON keys with the
same name merge.

The `references` and `definition` walks read the files of one language, except
JSON and JSONC: from a `.jsonc` or `.code-workspace` file they read JSON and
JSONC files; from a JSON file, or from a JSONC file recognized by its name
(such as `tsconfig.json`), they read JSON files and the JSONC files recognized
by name only. The `references` walk skips every path component that starts with
`.`, so `.eslintrc.json`, `.devcontainer.json`, `.devcontainer/…` and
`.Rprofile` are never listed from another file; `definition` and `repo-outline`
see them.

For the ten added languages every `references` answer is a name match
(`"source": "tree-sitter"`), and none of them has a language server:
`callers`, `callees` and `implementations` fail for them, and `status` lists no
server for them.

Language server limitations:

- `kotlin-language-server` 1.3.13: `references` omit the declaration, even
  though act-lite asks for it.
- `elixir-ls` 0.30: `callees` report each callee in the caller's file, not in
  the file that defines it.

These language servers were not installed on the machine where the
language-server tests ran, so they are untested:

- `OmniSharp` (C#): the .NET SDK was not installed.
- `gopls` (Go) and `sqls` (SQL): the Go toolchain was not installed.
- `ruby-lsp` (Ruby): the only Ruby was 2.6; `ruby-lsp` needs 3.0 or later.
- `metals` (Scala): Coursier, sbt, Scala and a Java runtime were not installed.
- `dart` (Dart): the Dart SDK was not installed.

`repo-outline` skips `.git`, `target` and `node_modules`. When `--max-files`
cuts the list, the output says `"truncated": true`. `repo-outline` counts every
recognized file against `--max-files`, so the files of the added languages
(`.svg`, `.xml`, `Makefile`, and others) use part of the budget: a truncated
outline of a repository that has them lists fewer of its other files.

## `act-lite mcp serve`

```bash
act-lite mcp serve                    # workspace: nearest git root, else the current directory
act-lite --workspace /path/to/repo mcp serve
```

The server exposes fifteen tools: `status`, `skeleton`, `symbols`,
`symbols_batch`, `repo_outline`, `references`, `callers`, `definition`, the
code-health analyzers `analyze_patterns`, `analyze_hotspots` and
`analyze_clones` (see [Code-health analyzers](#code-health-analyzers)), and
the dependency analyzers `graph`, `analyze_cycles`, `analyze_coupling` and
`analyze_impact` (see [Dependency analyzers](#dependency-analyzers)).
`status` lists the 40 languages and, for each language server, whether it is
`ready` or `not_installed`. MCP `skeleton` omits `import` entries for every
language. It keeps `include` entries: C and C++ `#include`, Make `include`,
CMake `include()` and `add_subdirectory()`, R `source()` and Julia
`include()`. `act-lite query skeleton` and `symbols-batch` list both.

**Analyzer tools.** Each analyzer tool (`graph` and the `analyze_*` tools) returns the same JSON
as its `act-lite analyze` command (see [`act-lite analyze`](#act-lite-analyze)):
for the same workspace root, language-server setting and parameters, the first
content block, parsed, equals the command's stdout, parsed. The tool
parameters follow one rule: a parameter `snake_case` is the flag
`--kebab-case`, an array is a repeatable flag, a boolean (default `false`) is a
presence flag, and `paths` is the positional arguments. `--workspace`,
`--format` and `--disable-lsp` are fixed when the server starts. An array can
also be sent as a string that holds a JSON array, and `limit` and `max_depth` as a numeric
string. A request error is a JSON-RPC `invalid_params` error (-32602) with the
command's message; an internal failure is `internal_error` (-32603). A
cancelled request stops its analysis and gets no response.

The second content block is a hint, and is not part of the result. When
`truncated` is `true`, it gives `total` and names the parameters that narrow
the result: `paths`, `include`, `exclude` and `limit`; for `graph` and
`analyze_impact` also `focus`, and `max_depth` when the request has a focus.
When `excluded` or
`incomplete` has a list cut at 100 entries, it says so and gives that object's
`counts`. Otherwise it gives the analyzer's usual next step. The first block
is always the JSON result alone.

## `act-lite analyze`

```bash
act-lite analyze <analyzer> [PATH]... [OPTIONS]
act-lite --workspace /path/to/repo analyze <analyzer> src --include-tests --limit 20
```

The command prints one JSON object, the result described under
[Analysis results](#analysis-results), and nothing else. The workspace root is
`--workspace`, else the current directory.

**Names.** An analyzer's command is its MCP tool name without the `analyze_`
prefix (`graph` has none), with `_` changed to `-`; the `analysis` value in the result is the
command name. A tool parameter `snake_case` is the flag `--kebab-case`, an
array is a repeatable flag, and a boolean is a presence flag. `paths` is the
positional arguments.

| MCP tool | Command | `analysis` |
|---|---|---|
| `analyze_patterns` | `act-lite analyze patterns` | `patterns` |
| `analyze_hotspots` | `act-lite analyze hotspots` | `hotspots` |
| `analyze_clones` | `act-lite analyze clones` | `clones` |
| `graph` | `act-lite analyze graph` | `graph` |
| `analyze_cycles` | `act-lite analyze cycles` | `cycles` |
| `analyze_coupling` | `act-lite analyze coupling` | `coupling` |
| `analyze_impact` | `act-lite analyze impact` | `impact` |

**Shared parameters.** Every analyzer takes these:

| Option | Default | Meaning |
|---|---|---|
| `[PATH]...` | the workspace root | Files or directories to analyze. A relative path resolves against the workspace root, not the current directory. Each path must exist and be inside the root. |
| `--include <GLOB>` | none | Repeatable. When given, a file must match at least one glob. |
| `--exclude <GLOB>` | none | Repeatable. A file that matches any glob is left out, and is not reported. |
| `--include-tests` | off | Analyze test files and embedded test code. |
| `--include-generated` | off | Analyze generated files and directories. |
| `--include-vendored` | off | Analyze vendored directories. |
| `--include-ignored` | off | Analyze ignored paths. |
| `--limit <N>` | 100 | The most findings returned, 0 to 1000. |

**Glob syntax.** A glob is matched against the workspace-relative path of a
file, with `/` separators. `*` matches any characters except `/`. `?` matches
one character except `/`. `**` as a whole path component matches zero or more
components. Every other character matches itself, case-sensitively. There are
no character classes, and a pattern that ends in `/` matches no file. A
pattern without `/` matches the file name at any depth: `*.rs` matches `a.rs`
and `src/x/a.rs`. A pattern with `/` matches the whole path: `src/**/x.rs`
matches `src/x.rs` and `src/a/b/x.rs`, and not `src/ax.rs`.

**Exit status.** A result exits 0, whatever its `status`. A request error
exits 1 and prints one line to stderr:
`{"error":{"message":"<message>","chain":[]}}` (under `--format text`,
`Error: <message>`). An argument that clap rejects, such as `--limit abc`,
exits 2.

| Request error | Message |
|---|---|
| A path does not exist | the not-found text |
| A path is outside the workspace root, after symlinks and `..` are resolved | `path must be inside the workspace root: <path>` |
| `--limit` is above 1000 | `limit must be between 0 and 1000, got <n>` |
| No recognized file in a supported language is left in scope | `no files to analyze in the requested scope: <t> excluded as tests, <g> as generated, <v> as vendored, <i> as ignored; <f> left out by include or exclude; <u> unrecognized. Re-run with the matching include option (tests, generated, vendored, ignored).` |
| An internal failure | `<analysis> failed: <cause>` |

The paths are checked in order, and the first that fails is reported.

**Narrowing a truncated result.** `truncated` is `true` when `total` is more
than the number of findings returned. Narrow the request with `PATH`
arguments, `--include` and `--exclude` (for `graph` and `impact` also
`--focus` and `--max-depth`), or change `--limit`. `--limit 0`
returns the counts only. When `excluded.truncated` or `incomplete.truncated`
is `true`, a list was cut at 100 entries and its `counts` hold the totals.

**Cancellation.** `analyze` installs no signal handler. SIGINT, SIGTERM and
SIGHUP end the process with their default behavior, and the exit status is the
signal's (a shell shows 130 for SIGINT). The result is written only after the
analysis finishes, so an interrupted run prints nothing to stdout. An analysis
writes no file, so nothing needs clean-up. A closed stdout exits 141.

## Analysis results

Every analyzer returns one JSON object. These top-level keys are always
present, in this order, including empty lists and zero counts:

| Key | Meaning |
|---|---|
| `analysis` | The analyzer name, for example `patterns`. |
| `status` | `findings`, `no_findings`, `unsupported` or `incomplete` (below). |
| `complete` | `true` if and only if every count in `incomplete.counts` is 0. |
| `findings` | The returned findings, in the order below, cut to `limit`. |
| `total` | The number of findings before the cut. |
| `limit` | The effective limit. |
| `truncated` | `true` if and only if `total` is more than the length of `findings`. |
| `coverage` | `analyzed_files`; `languages`, a list of `{ "language", "files", "supported" }`; `unrecognized_files`. |
| `excluded` | `tests`, `generated`, `vendored`, `ignored` (lists of `{ "path", "range"? }`), `counts` (the same four keys), `truncated`. |
| `incomplete` | `skipped_files`, `parse_failures`, `unresolved`, `limits_hit` (lists), `counts` (the same four keys), `truncated`. |
| `evidence` | `sources` (list of strings), `language_servers` (list). |

An analyzer can add one more top-level key, `details` (an object), for
example a ranking formula and its weights. It holds no per-file data.

**Status.** The status is derived from the other keys. The rules are checked
in this order:

1. `unsupported`: `total` is 0, `complete` is `true`, and
   `coverage.analyzed_files` is 0. Every recognized file in scope is in a
   language the analyzer does not support, and no file in a supported
   language was removed by a file-level step (`--include` or `--exclude`, or
   exclusion as a test file, a generated file name or a generated header).
   When such a file was removed, the request is the empty-scope error, which
   names the include options. Example: `analyze patterns Cargo.toml`.
2. `findings`: `total` is 1 or more, whether `complete` is `true` or `false`.
   With `complete: false`, more findings can exist in the missing evidence.
3. `incomplete`: `total` is 0 and `complete` is `false`.
4. `no_findings`: `total` is 0, `complete` is `true`, and
   `coverage.analyzed_files` is 1 or more.

Missing evidence is never a clean result: `no_findings` requires complete
evidence. Files in an unsupported language beside supported files do not make
a result incomplete; they appear in `coverage.languages` with
`supported: false`.

**Incomplete evidence.**

- `skipped_files`: `{ "file", "kind", "reason" }`. `kind` is `size_limit`,
  `not_utf8`, `unreadable`, `symlink_outside_workspace` or `walk_error`.
- `parse_failures`: `{ "file", "range" }`, the first syntax error in the
  file. The file is still analyzed on the recovered tree.
- `unresolved`: `{ "file", "range", "kind", "target", "reason" }`, a
  relationship that the analysis needs and could not resolve.
- `limits_hit`: `{ "limit", "value", "detail" }`, an analysis bound (a
  request parameter) that stopped the analysis early.

**Locations.** `file` and `path` are relative to the workspace root, with `/`
separators and no leading `./`. A directory ends with `/`; the root is `"."`.
An invalid UTF-8 sequence in a name is written as U+FFFD. `range` is
`"L:C-L:C"`: lines are 1-indexed, columns are 1-indexed UTF-8 byte offsets,
the start is inclusive and the end is exclusive. An item that concerns a whole
file has no `range`.

**Order.** Every list has one total order, so the same request on the same
files gives byte-identical output.

- Paths compare as byte strings: `a-b/x.rs` sorts before `a/b.rs`.
- Location order: `file`, then start line, start column, end line, end
  column. An item without `range` sorts first.
- `findings`: location order, unless the analyzer defines a rank (for example
  a score, descending), with location order to break ties. Each analyzer
  names a last key that makes the order total.
- `coverage.languages` and `evidence.language_servers`: by `language`.
  `excluded` lists: by `path`, then `range`. `skipped_files`: by `file`, then
  `kind`. `parse_failures`: by `file`. `unresolved`: by `file`, `range`,
  `kind`, then `target`. `limits_hit`: by `limit`, then `value`.
  `evidence.sources`: byte order.
- A non-integer number is rounded half to even to 3 decimal places and has no
  trailing zeros. An integral value is an integer.

**Bounds.** `limit` is 0 to 1000 (default 100). Another value is an error:
`limit must be between 0 and 1000, got <n>`. `limit: 0` returns only the
counts. Each list in `excluded` and `incomplete` holds at most 100 entries;
`counts` gives the full totals, and that object's `truncated` is `true` when
a list was cut. A list inside a finding or inside `details` has a fixed
maximum and a sibling integer `<name>_total` with its full length. To narrow a
result, give paths, `--include`, `--exclude` or a smaller `--limit`.

**Evidence.** `evidence.sources` lists every instrument that produced part of
the result: `"tree-sitter"`, or `"lsp:<binary>"` (for example
`"lsp:rust-analyzer"`). It is sorted, has no duplicates, and is `[]` when no
file was analyzed. When it has more than one value, every finding and every
analyzer-specific relationship object (for example a dependency edge)
carries `"source"` with one value from the list. With a single source, items
omit `source`. `evidence.language_servers` has one
`{ "language", "server", "state", "reason"? }` entry per language whose server
the analyzer can use; `state` is `used`, `disabled`, `not_installed` or
`failed` (with `reason`). An analyzer that uses no language server has `[]`.

## Code-health analyzers

Three analyzers measure code health: `analyze_patterns`
(`act-lite analyze patterns`), `analyze_hotspots`
(`act-lite analyze hotspots`) and `analyze_clones`
(`act-lite analyze clones`). Each returns the result described under
[Analysis results](#analysis-results), with `details` always present.

**Languages.** The three analyzers support 23 programming languages: Bash, C,
C++, C#, Dart, Elixir, Go, Java, JavaScript, JSX, Julia, Kotlin, Lua, PHP,
Python, R, Ruby, Rust, Scala, Swift, TSX, TypeScript and Zig. They do not
support the 17 data, schema and markup formats: CMake, CSS, Dockerfile,
GraphQL, HCL/Terraform, HTML, JSON, JSONC, Make, Markdown, Nix, Protocol
Buffers, SCSS, SQL, TOML, XML and YAML. Those files are listed in `coverage.languages` with
`supported: false`; a scope that holds only such files gives `unsupported`, and
a mixed scope is not made incomplete by them. A Bash function declares no
parameters (it reads `$1`, `$2`, …), so for Bash `parameters` is `null`, the
`long_parameter_list` rule is never evaluated, and `patterns` says so in
`details.not_applicable`.

**Functions.** `patterns` and `hotspots` measure functions. A function is a
function construct of its language that has a body: named functions, methods,
constructors, accessors, and anonymous functions (lambdas, closures, Ruby
blocks). A declaration without a body (a C prototype, a Java interface method,
a Rust trait method signature) is not a function. A function nested in another
is a separate function, and each function is measured on its own body: its
code minus the functions nested in it. Top-level code outside any function (a
script body, a module's statements) is not measured, so a script with no
function gives `no_findings`; `details.functions` says how many functions were
measured. `function` is the function's name, or `null` for an anonymous
function; `range` is the function's own range. Blocks passed to calls (Ruby
`do … end`, Kotlin trailing lambdas) are functions, with `"function": null`.
Each clause of an Elixir multi-clause `def` is a separate function.
Zig functions are `fn` declarations with a body; a `test` block is a function
only with `--include-tests`. R functions are `function(…)` and `\(…)`
definitions, named by the `<-`, `=` or `<<-` assignment whose right side they
are, else `null`. Julia functions are long-form and short-form definitions
(`f(x) = …`, also with a `::T` return type or a `where` clause), macros,
anonymous `function (x) … end` and `x -> …` functions, and `do` blocks;
`function f end` is not a function.

**Metrics.**

| Metric | Definition |
|---|---|
| `lines` | The lines on which a token of the function's own body (its header included) starts, ends or spans. Blank lines, comment-only lines and lines that hold only nested-function code do not count. A multi-line string counts every line it spans, except a Python docstring, whose lines do not count. |
| `nesting` | The deepest chain of nesting constructs (if, multi-way, loop, try) in the function's own body. An `if` in the else position of another `if` does not add a level. Arms, handlers, conditional expressions and logical operators do not nest. |
| `complexity` | Cyclomatic complexity: 1 + the decision points in the function's own body. Each if and else-if, each arm that is not a default arm, each loop, each exception handler, each conditional expression and each logical-operator occurrence (`&&`, `\|\|`, `and`, `or`, `??` and their language forms) is one decision point. |
| `parameters` | The declared parameters. A receiver (`self`, `this`, a Go method receiver) does not count; a destructured parameter, a parameter with a default value and a variadic parameter each count 1. `null` for Bash. |

Constructs not modeled: Python comprehension `for` and `if` clauses and the
compound logical assignments `&&=`, `||=`, `??=` are not decision points.
Scala placeholder lambdas (`_ + 1`) and partial-function literals are not
functions; their code belongs to the enclosing function. A multi-label case
counts by the grammar's shape: C# `case 1: case 2:` is two decision points,
Java `case 1, 2 ->` is one. An Elixir `rescue` block and a Scala `catch`
clause count once each, whatever the number of clauses in them. Loops without
a condition (Rust `loop`, Go `for {}`) count as one decision point, like every
loop. An exhaustive multi-way construct with no default arm (a Rust `match`, a
Kotlin `when` on a sealed type) counts every arm, one more than McCabe's
n − 1. Dart collection `if` and `for` elements, Elixir `with … else` clauses,
optional chaining (`?.`) and C# `and` / `or` patterns are not decision points.
Zig `try` and `orelse`, R `ifelse()`, `switch()` and `tryCatch()` calls, and
Julia comprehension `for` and `if` clauses are not decision points.

**`analyze_patterns`.** A rule is exceeded when the metric is strictly greater
than its threshold. The thresholds are fixed:

| `rule` | `metric` | `threshold` |
|---|---|---|
| `oversized_function` | `lines` | 50 |
| `deep_nesting` | `nesting` | 4 |
| `excessive_branching` | `complexity` | 10 |
| `long_parameter_list` | `parameters` | 5 |

There is one finding per function per exceeded rule: `file`, `range`, `rule`,
`function`, `metric`, `value`, `threshold`, and `metrics` with all four values
of the function. Findings are in location order, then by `rule`. `details`
gives `thresholds`, `counts_by_rule` (the findings per rule before the
`limit` cut), `functions`, and `not_applicable` (one entry per supported
language with a rule that does not apply: Bash, `long_parameter_list`). The
analyzer has exactly these four rules: it does not check language-version
upgrades, deprecated APIs or porting readiness. A finding states a measured
value over a fixed threshold, not a verdict.

**`analyze_hotspots`.** For each function, score = complexity × lines; for
each file, score = the sum of its functions' scores. Each analyzed file with at
least one function is one item: `file` (no `range`), `score`, `functions`,
`complexity` and `lines` (the sums the score uses), and `top_functions`, at
most 5 functions by score (each with `function`, `range`, `score` and its four
metrics), with `top_functions_total`. Items are ranked by `score`, highest
first, then by path. A file without a function is not an item, so
`no_findings` means that no analyzed file has a function. `details` states the
formula and the definitions of its inputs, and `dependency_information` is
`false`: the ranking uses no dependency data. A hotspot is a place to start
investigating, not a verdict.

**`analyze_clones`.** A clone class is a set of two or more syntax subtrees in
one language with the same normalized form.

- *Exact* matching compares the syntax tree, ignoring formatting and comments.
- *Renamed* matching also renames identifiers consistently by first appearance
  (the same name maps to the same placeholder inside a subtree, different
  names to different placeholders) and abstracts each string, character,
  numeric and symbol literal; `true`, `false`, `null`, `nil` and `None` are
  kept. A class is `exact` when all members have the same exact form, else
  `renamed`, with `variants` the number of exact forms.
- The literal kinds abstracted are, per language: Bash `string`, `raw_string`,
  `ansi_c_string`, `translated_string`, `number`; C and C++ `string_literal`,
  `raw_string_literal` (C++), `concatenated_string`, `char_literal`,
  `number_literal`, `user_defined_literal` (C++); C# `string_literal`,
  `verbatim_string_literal`, `raw_string_literal`,
  `interpolated_string_expression`, `character_literal`, `integer_literal`,
  `real_literal`; Dart `string_literal`, `symbol_literal`,
  `decimal_integer_literal`, `hex_integer_literal`,
  `decimal_floating_point_literal`; Elixir `string`, `charlist`, `sigil`,
  `char`, `integer`, `float`, `atom`, `quoted_atom`; Go
  `interpreted_string_literal`, `raw_string_literal`, `rune_literal`,
  `int_literal`, `float_literal`, `imaginary_literal`; Java `string_literal`,
  `character_literal`, `decimal_integer_literal`, `hex_integer_literal`,
  `octal_integer_literal`, `binary_integer_literal`,
  `decimal_floating_point_literal`, `hex_floating_point_literal`; JavaScript,
  JSX, TypeScript and TSX `string`, `template_string`, `number`, `regex`;
  Julia `string_literal`, `prefixed_string_literal`, `command_literal`,
  `prefixed_command_literal`, `character_literal`, `integer_literal`,
  `float_literal`; Kotlin `string_literal`, `character_literal`, `integer_literal`,
  `long_literal`, `hex_literal`, `bin_literal`, `unsigned_literal`,
  `real_literal`; Lua `string`, `number`; PHP `string`, `encapsed_string`,
  `heredoc`, `nowdoc`, `integer`, `float`; Python `string`,
  `concatenated_string`, `integer`, `float`; R `string`, `integer`, `float`,
  `complex`; Ruby `string`, `chained_string`,
  `heredoc_body`, `character`, `simple_symbol`, `delimited_symbol`,
  `hash_key_symbol`, `regex`, `integer`, `float`, `rational`, `complex`; Rust
  `string_literal`, `raw_string_literal`, `char_literal`, `integer_literal`,
  `float_literal`; Scala `string`, `interpolated_string`,
  `character_literal`, `integer_literal`, `floating_point_literal`; Swift
  `line_string_literal`, `multi_line_string_literal`, `raw_string_literal`,
  `regex_literal`, `integer_literal`, `hex_literal`, `oct_literal`,
  `bin_literal`, `real_literal`; Zig `string`, `multiline_string`,
  `character`, `integer`, `float`. A string with an embedded expression is not
  abstracted; its identifiers are renamed.
- A candidate has at least 50 tokens of its normalized form (a literal counts
  1). The file's root node and subtrees that hold a syntax error are not
  candidates.
- Only the outermost duplication is reported: a class whose every member lies
  inside a member of a reported class is not reported again. When two
  functions are clones, their bodies and statements are not listed again.
- Members in different languages never match. JavaScript and JSX are
  different languages, and so are TypeScript and TSX: byte-identical code in
  a `.js` and a `.jsx` file is no clone.
- A finding gives the first member's `file` and `range`, `kind`, `language`,
  `tokens` (the normalized size), `members` (at most 20, each with `file`,
  `range` and `variant`) with `members_total`, and `variants`. The largest
  duplication comes first: `tokens`, then `members_total`, highest first.
- `focus` (`--focus <PATH>`, repeatable): report only the classes that have a
  member in a file at or below a focus path. Classes are still built from the
  whole scope, so a change is compared with the rest of the repository; with
  `focus`, the members in focus files are listed first. A focus path must
  exist, be inside the workspace root and be at or below a `PATH` argument
  (`focus path is not inside the requested paths: <path>`); it is checked
  after the paths and `--limit`. `details.focus_not_analyzed` lists the focus
  paths under which no file was analyzed.

Not reported: two subtrees that differ by an inserted, removed or reordered
statement, however similar they look; and a run of statements that is
duplicated but is not the whole of one syntax node on both sides.

A clone class shows that code has the same syntax tree, apart from formatting, comments and, for renamed clones, identifier names and literal values. It does not show that the members behave the same, that they can share one function, or that extracting them is safe.

**Bounds.** These analyzers read every in-scope file of up to 8 MiB and have
no hard time or memory cap; cancel a run to bound its time. They have no
analysis bound that stops early, so `incomplete.limits_hit` is always empty.
They use no language server: `evidence` is
`{"sources":["tree-sitter"],"language_servers":[]}`.

## Dependency analyzers

Four analyzers read the file dependencies of a repository: `graph`
(`act-lite analyze graph`), `analyze_cycles` (`act-lite analyze cycles`),
`analyze_coupling` (`act-lite analyze coupling`) and `analyze_impact`
(`act-lite analyze impact`). Each returns the result described under
[Analysis results](#analysis-results), with `details` always present. They
report which files import which other files, with the import statements as
evidence. They do not report which symbols a file uses.

**Languages.** The four analyzers support seven languages, in four families:
JS/TS (JavaScript, JSX, TypeScript, TSX), Python, Go and Rust. The other 33
languages are listed in `coverage.languages` with `supported: false`; a scope
that holds only such files gives `unsupported`, and a mixed scope is not made
incomplete by them. A language gets dependency resolution when its import
construct names a file, or a package directory of files, by a rule that its
language or toolchain documentation fixes and that act-lite can apply from the
files in the repository, without compiler flags, build-system evaluation or
the network.

| Language | Relationship constructs | Static file rule | Supported | Reason when not supported |
|---|---|---|---|---|
| JavaScript | `import`, `export … from`, `import()`, `require()` | yes | yes | — |
| JSX | `import`, `export … from`, `import()`, `require()` | yes | yes | — |
| TypeScript | `import`, `export … from`, `import()`, `require()` | yes | yes | — |
| TSX | `import`, `export … from`, `import()`, `require()` | yes | yes | — |
| Python | `import`, `from … import` | yes | yes | — |
| Go | `import` | yes | yes | — |
| Rust | `mod`, `use`, `extern crate`, `include!` | yes | yes | — |
| Dart | `import`/`export` URIs | relative URIs yes; `package:` needs `.dart_tool/package_config.json`, which a tool generates | no | Later candidate: user value below the seven. |
| Ruby | `require_relative` (relative to the file), `require` (`$LOAD_PATH` at run time) | `require_relative` only | no | Later candidate; `require` has no static rule. |
| Bash | `source`, `.` | no: the path is relative to the shell's working directory or `PATH` at run time | no | No static rule. |
| C | `#include "…"`, `#include <…>` | no: search paths come from compiler flags | no | Needs build-system input. |
| C++ | `#include "…"`, `#include <…>` | no: search paths come from compiler flags | no | Needs build-system input. |
| C# | `using` | no: names a namespace, not a file | no | Not a file relationship. |
| Java | `import` | no: names a type or package; files of one package use each other without an import; source roots come from the build tool | no | Not a file relationship without build input. |
| Kotlin | `import` | no: names a type or package; files of one package use each other without an import; source roots come from the build tool | no | Not a file relationship without build input. |
| Scala | `import` | no: names a type or package; files of one package use each other without an import; source roots come from the build tool | no | Not a file relationship without build input. |
| Swift | `import` | no: names a module (a build target); files of one module see each other | no | Not a file relationship. |
| PHP | `use`, `require`/`include` | no: `use` names a namespace (autoload maps it through `composer.json`); `require` paths are often computed | no | Needs Composer autoload configuration. |
| Elixir | `alias`, `import`, `use`, `require` | no: names modules; the build compiles every file | no | Not a file relationship. |
| Lua | `require` | no: searched through `package.path` at run time | no | No static rule. |
| CSS | `@import`, `@use`, `@forward` | CSS relative URLs yes; Sass load paths come from configuration | no | Later candidate: data and style formats are outside the supported set. |
| SCSS | `@import`, `@use`, `@forward` | CSS relative URLs yes; Sass load paths come from configuration | no | Later candidate: data and style formats are outside the supported set. |
| HCL/Terraform | module `source = "./…"` | yes for local paths (a directory) | no | Later candidate (directory targets). |
| HTML | `<script src>`, `<link href>` | no: URLs resolve against a server root | no | Not a file relationship. |
| Dockerfile | `FROM`, `COPY` | no: image names and build-context paths | no | Not a code dependency. |
| JSON | none in the format (links and `$ref` are conventions of other tools) | — | no | No relationship construct. |
| YAML | none in the format (links and `$ref` are conventions of other tools) | — | no | No relationship construct. |
| TOML | none in the format (links and `$ref` are conventions of other tools) | — | no | No relationship construct. |
| Markdown | none in the format (links and `$ref` are conventions of other tools) | — | no | No relationship construct. |
| SQL | none in the format (links and `$ref` are conventions of other tools) | — | no | No relationship construct. |
| JSONC | none in the format | — | no | No relationship construct modeled. |
| XML | none (no build-system semantics for XML) | — | no | No relationship construct modeled. |
| GraphQL | none in the specification | — | no | No relationship construct. |
| Protocol Buffers | `import "x.proto"` | no: resolved against `--proto_path` flags | no | Needs build input. |
| Make | `include` | no: relative to make's working directory and `-I` | no | Needs build input. |
| CMake | `include()`, `add_subdirectory()` | partly: `add_subdirectory` is relative to the current source directory; `include()` searches `CMAKE_MODULE_PATH` | no | Later candidate (`add_subdirectory`); `include()` needs configuration evaluation. |
| Zig | `@import("x.zig")` | yes for a path literal (relative to the file); package names come from `build.zig` | no | Later candidate. |
| R | `source()`, `library()` | no: `source` is relative to the working directory; `library` names packages | no | No static rule. |
| Julia | `include("x.jl")`, `using`, `import` | `include` yes (relative to the file); `using` names packages | no | Later candidate. |
| Nix | `import ./x.nix`, `<nixpkgs>` | yes for a path literal; `<…>` uses `NIX_PATH` | no | Later candidate. |

**Parameters.** Each analyzer takes the shared parameters. `paths` is the
*scope*: the files whose relationships are read. Only files in scope can be
the source of an edge, so a dependent outside the scope is not found; the
default scope is the workspace root. `graph` and `analyze_impact` also take:

| MCP parameter | Option | Default | Meaning |
|---|---|---|---|
| `focus` | `--focus <PATH>`, repeatable, one path per flag | none | The files whose connections (`graph`) or dependents (`analyze_impact`, which needs at least one) are reported. A directory means every file below it. Each path must exist, be inside the workspace root and be at or below a `PATH` argument. |
| `direction` | `--direction <in\|out\|both>` | `both` | `graph` with a focus only. `out`: what the focus files depend on. `in`: what depends on them. `both`: the union. |
| `max_depth` | `--max-depth <N>` | `graph`: 1; `analyze_impact`: 1000 | How many edges from a focus file the traversal follows, 1 to 1000. For `graph`, with a focus only. |

The focus files are the files that discovery reaches at or below a `focus`
path, analyzed or not: discovery's directory pruning applies (`.git`, ignored,
vendored and generated directories are not walked), and a file that a later
step leaves out (unrecognized, tests, generated, globs, unsupported language,
skipped) is still a focus file. A focus path that names a file, or a directory
inside a pruned directory, is walked as named: `--focus vendor/lib/x.js` gives
that one file. A focus file need not be analyzed: as the target of an `in`
traversal it only needs analyzed sources (an `import './style.css'` makes
`src/view.tsx` a dependent of the CSS file); only the `out` direction reads a
focus file's own relationships. `graph` explores, and its first useful answer
is the direct neighbourhood, so its default depth is 1. `analyze_impact` asks
for direct and transitive dependents, so its default follows chains to the
end; reaching 1000 is disclosed in `limits_hit`. The traversal costs time in
proportion to the edges it visits, whatever the depth.

**Relationships.** A *relationship* is one occurrence of an import construct in
an analyzed file, with a `kind`, a `specifier` (the target text as written,
without quotes) and a `range`. One statement can hold several relationships
(`import a, b` in Python, `use a::{b, c}` in Rust, a Go `import (…)` block);
they share the statement's range. Relationships inside removed test regions
are not extracted. A file with a parse error gives the relationships of its
recovered tree.

| Family | Construct | `kind` | `specifier` | `range` |
|---|---|---|---|---|
| JS/TS | `import … from '…'`, `import '…'` | `import`; `type_import` for `import type` | the source string | the statement |
| JS/TS | `export … from '…'`, `export * from '…'` | `reexport` | the source string | the statement |
| JS/TS | `import(…)` | `dynamic_import` | the first argument when it is a string or a template string without a substitution; otherwise the argument's text, `unresolved` with reason `specifier is not a string literal` | the call |
| JS/TS | `require(…)`; TS `import x = require('…')` | `require` | as `dynamic_import` | the call; the statement for `import x = require` |
| Python | `import a.b.c`, `import a as b` | `import` | the dotted name | the statement |
| Python | `from X import n`, `from X import *` | `from_import`, one per imported name, or one for `*` | the module name and the imported name joined by `.`, unless the module name ends with `.` (`from .models import User` gives `.models.User`, `from . import helpers` gives `.helpers`, `from a import *` gives `a.*`) | the statement |
| Go | each import spec | `import` | the import path | the spec |
| Rust | `mod x;` (without a body) | `module` | the name; a preceding `#[path = "…"]` replaces the file probe | the `mod` item, without its attributes |
| Rust | `use` declaration, one per leaf path (`use a::{b, c::d}` gives `a::b` and `a::c::d`; `self` in a list gives the list's prefix; `use a::*` gives `a`; `use a::b as c` gives `a::b`) | `use` | the leaf path | the declaration |
| Rust | `extern crate x;` | `extern_crate` | the name | the declaration |
| Rust | `include!`, `include_str!`, `include_bytes!` | `include` | the string literal, or the argument's text when it is not a string literal (`unresolved`) | the invocation |
| Rust | `mod x;` and `use …;` token sequences inside a macro invocation at module level (wrapper macros such as `cfg_rt! { pub mod a; }`) | `module`, `use` | as above | from the first token (`pub`, `mod` or `use`) to the `;` |

The kinds are `dynamic_import`, `extern_crate`, `from_import`, `import`,
`include`, `module`, `reexport`, `require`, `type_import` and `use`. A Python `from __future__ import …` is not a relationship.

**Outcomes.** Each relationship has exactly one outcome:

| Outcome | Meaning | Where it appears | Effect on `complete` |
|---|---|---|---|
| `resolved` | The family rule finds one or more existing files inside the workspace. | an edge per target file | none |
| `external` | The family rule places the target outside the workspace by a stated test (below). | `details.relationships.external`, and `external` per `analyze_coupling` item | none: the target is not a workspace file |
| `unresolved` | The family rule cannot decide, or decides that the target should be a workspace file and finds none. | `incomplete.unresolved`, with `file`, `range`, `kind`, `target` (the specifier) and `reason` | `false` when the entry is relevant to the question (below) |

An import is never `external` by default. It is `external` only by these
tests: JS/TS, a Node built-in module name, a package that a `package.json` in
the importing file's directory or an ancestor declares in a dependency table
with a registry version, or a `node_modules/<package>/` directory there;
Python, a top-level name that exists under none of the roots and is a
standard-library module or a declared dependency; Go, a standard-library path
(no `.` in the first path element), or a path under no workspace module from a
file that belongs to a workspace module; Rust, a built-in crate name (`std`,
`core`, `alloc`, `proc_macro`, `test`) or a dependency key of the crate's
`Cargo.toml`; every family, a path that leaves the workspace root. Two
overrides apply: a name that the test would call `external` but that names a
project of the workspace outside the vendored directories is `unresolved`,
because a workspace file could be its target; and when a resolution input of
the family could not be read, every relationship of that family that would be
`external` is `unresolved`, reason `<file> could not be read`. Vendored
directories are outside the analyzed project: a package name that resolves
only into one is `external`.

**Resolution.** Every probe lists the parent directory and compares names byte
for byte, so the result is the same on case-sensitive and case-insensitive file
systems. A probe resolves a symlink: a target inside the workspace is used at
its own path, a target outside makes the relationship `external`, and a broken
symlink is no match. A relative path is normalized (`.` and `..` removed)
against the importing file's directory; one that leaves the workspace root is
`external` without a probe. A probe can match a file that discovery excluded or
that is outside `paths`: the relationship is `resolved` and the edge has
`"to_analyzed": false`.

- **JS/TS.** A specifier that starts with `./` or `../` is probed, first match
  wins: for `.js`, `.jsx`, `.mjs` or `.cjs`, the TypeScript source first
  (`.ts`, `.tsx`, `.d.ts` for `.js`; `.tsx` for `.jsx`; `.mts`, `.d.mts` for
  `.mjs`; `.cts`, `.d.cts` for `.cjs`); the path as a file (any extension,
  `./style.css` too); the path plus `.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`,
  `.mjs`, `.cjs`, `.mts`, `.cts`, `.json`; the path as a directory, by its
  `package.json` `main`, then `index` plus each of those extensions. No match
  is `unresolved`, reason `no file matches <S>`. A specifier that starts with
  `/` is `unresolved` (`root-relative specifier is not applied`); `node:` is
  `external`; `#` is `unresolved` (`package.json "imports" is not applied`).
  A bare specifier uses the nearest `tsconfig.json` or `jsconfig.json`
  (`tsconfig.json` when both exist): first `compilerOptions.paths` (the
  matching pattern with the longest prefix before `*`; each replacement is
  probed from `baseUrl`, or from the declaring config's directory), then
  `compilerOptions.baseUrl`, then a workspace package whose `package.json`
  `name` is the package name (a package with an `exports` field is
  `unresolved`, `package.json "exports" of <name> is not applied`; a name that
  two `package.json` files declare is `unresolved`), then a dependency whose
  value is `file:`, `link:`, `portal:` or `workspace:` and names a workspace
  location, then the `external` test. Otherwise it is `unresolved`, reason
  `bare specifier is not a declared dependency: <package name>`. A relative
  `extends` of a config is read, and its `paths` and `baseUrl` apply unless the
  extending config sets them.
- **Python.** A file's *own root* is the parent of the topmost directory of the
  chain of directories with `__init__.py` above it, or its own directory when
  that has no `__init__.py`. The *roots*, in order, are the own root, the
  workspace root, and `src/` when it is a directory. `a.b.c` exists under a
  root as `a/b/c/__init__.py`, else `a/b/c.py`; a directory without
  `__init__.py` is a namespace package (it exists, with no file). `import
  a.b.c` uses the first root under which `a` is a module or regular package,
  else the first under which it is a namespace package. `from X import n`
  targets `X`'s file when a top-level statement of it binds `n`, else the
  submodule `X.n`, and also `X`'s file when it has a `from … import *`. A
  relative import (`.`, `..models`) resolves in the package of the file, and
  is `unresolved` outside a package (`relative import outside a package`) or
  above the top-level package (`relative import beyond the top-level
  package`). A top-level name found under no root is `unresolved` when a
  module or regular package of that name exists elsewhere in the workspace
  (`<a> is a module of the workspace outside the roots: <path>`), `external`
  when it passes the external test (a standard-library module of the list
  below, or a distribution name, normalized by PEP 503, declared in a
  `pyproject.toml`, `requirements*.txt` or `setup.cfg` in the file's directory
  or an ancestor), else `unresolved`.
- **Go.** The module map is the set of `go.mod` files in the workspace (not in
  directories named `testdata` or starting with `_` or `.`). An import path
  under a module path (or under a `replace` directive with a relative path
  inside the workspace) names the package directory below that module's
  directory. When exactly one such directory exists, belongs to that module and
  holds a `.go` file that is not a `*_test.go` file, the import has one edge to
  each such file in it. Two are `unresolved` (`ambiguous import: <module
  paths>`); none is `unresolved` (`no Go package files for <I> in the
  workspace`). `C` (cgo) is `external`. A file under no `go.mod` of the
  workspace gives `unresolved` (`file is under no go.mod in the workspace`) for
  a path that is not standard library. Test files are never edge targets.
- **Rust.** The crates are the `[package]` entries of the workspace's
  `Cargo.toml` files, with their roots (`[lib]`, else `src/lib.rs`; `[[bin]]`
  paths, else `src/main.rs`, `src/bin/*.rs`, `src/bin/*/main.rs`; `build.rs`;
  `examples/*.rs`, `benches/*.rs`, `tests/*.rs`) and their dependency keys. The
  module tree follows the Rust Reference: `mod x;` probes `<dir>/x.rs`, then
  `<dir>/x/mod.rs`, where `<dir>` is the module file's directory when the file
  is a crate root, a `mod.rs` or a `#[path]` file, else that directory plus the
  file's stem; `#[path = "p"]` replaces the probe. A `use` path starts at the
  crate root (`crate`), the current module (`self`), its parent (`super`), a
  child module, the package's own lib (from a bin, example, bench, test or
  `build.rs`), a path dependency or a workspace crate that the crate depends
  on; it follows module segments, and its first remaining segment must be an
  item that the reached module's file declares (or the file has a glob `use`).
  `include!` paths are relative to the declaring file. A `Cargo.toml` that
  cannot be read makes every relationship of its package `unresolved`.

**What the dependency analyzers read.** Besides the files in scope, they read:

- *Resolution inputs*: the configuration files found in the workspace,
  whatever `paths` says, outside pruned directories, and only when an analyzed
  file of their family needs them. Each is listed in
  `details.resolution_inputs` (`{ "file", "kind" }`, at most 100, with
  `resolution_inputs_total`); one that cannot be read or parsed keeps its entry
  with an `error` key, and the relationships whose rule reads it are
  `unresolved`, reason `<file> could not be read`.

  | File | `kind` | Family | What is read |
  |---|---|---|---|
  | `package.json` | `package.json` | JS/TS | `name`, `main`, the presence of `exports`, the keys of `dependencies`, `devDependencies`, `peerDependencies`, `optionalDependencies` |
  | `tsconfig.json`, `jsconfig.json`, and each file a relative `extends` names (comments and trailing commas accepted) | `tsconfig` | JS/TS | `compilerOptions.baseUrl`, `compilerOptions.paths`, `extends` |
  | `go.mod` | `go.mod` | Go | `module`, `replace` lines with a relative path |
  | `Cargo.toml` | `Cargo.toml` | Rust | `[package] name`, `[lib] name` and `path`, `[[bin]] path`, the dependency table keys |
  | `pyproject.toml`, `requirements*.txt`, `setup.cfg` | `pyproject.toml`, `requirements`, `setup.cfg` | Python | the declared distribution names |

  A `package.json`, `go.mod` or `Cargo.toml` below a test directory (`test`,
  `tests`, `__tests__`, `spec`, `testdata`) adds no workspace package, module
  or crate, so a test fixture that copies a real package does not create a
  duplicate.
- *Support reads*: source files that are not analyzed but that resolution
  needs: the Rust files on the module path from a crate root to an analyzed
  file, and a module file whose names a `from X import n` or a `use` checks,
  when that file is outside `paths` or excluded by discovery. Each is read and
  parsed only for that purpose, under the 8 MiB, regular-file and UTF-8 bounds,
  inside the workspace and outside pruned directories. It counts nowhere in
  `coverage`, and its own relationships are not edges. When it cannot be read,
  the relationship that needed it is `unresolved`, reason `<file> could not be
  read`.
- *Probe listings*: the directory listings of the probes, including the
  existence check of `node_modules/<package>/` in directories that discovery
  prunes (metadata only; nothing in them is read).
- *Removed Rust test regions*: the text of a removed test region is read only
  for its `mod` declarations, which build the module tree (so a `src/tests.rs`
  that `#[cfg(test)] mod tests;` loads is reached). It is never analyzed or
  reported.

Nothing else is read, and nothing is written.

**Edges.** An *edge* is an ordered pair (source file, target file) with at
least one resolved relationship from the source to the target. The source is
an analyzed file and the target is a workspace file. All relationships from
one source to one target form one edge. A relationship whose target is its own
source gives no edge and counts as `resolved`. `to_analyzed` is `true` when the
target is an analyzed file of the same scope, else `false` (a CSS file, an
excluded test file, a file outside `paths`); a traversal does not continue from
a target with `to_analyzed: false`, because its relationships are unknown.

```json
{"file":"src/main.ts","to":"src/a.ts","to_analyzed":true,
 "kinds":["import"],
 "evidence":[{"range":"1:1-1:25","kind":"import","specifier":"./a"}],
 "evidence_total":1}
```

`kinds` is the set of kinds in the edge; `evidence` holds at most 5
relationships, in location order, with `evidence_total`. A *step*, used in
cycle paths and impact chains, is the compact form
`{ "file", "range", "kind", "specifier", "to" }` of an edge's first
relationship.

**Common `details`.** Every dependency analyzer returns
`relationships` (`total`, `resolved`, `external`, `unresolved`, counted over
every relationship of every analyzed file, also for `graph` and
`analyze_impact`), `kinds` with `kinds_total` (the kinds that occur, in byte
order), and `resolution_inputs` with `resolution_inputs_total`.

**Which unresolved relationships count.** An unresolved relationship is listed
in `incomplete.unresolved`, and makes `complete` `false`, when it is relevant
to the question:

| Analyzer | Relevant unresolved relationships |
|---|---|
| `analyze_coupling`, `graph` without `focus` | every one |
| `analyze_cycles` | every one, except kind `module` (a `module` edge cannot be part of a cycle) |
| `graph` with `direction: out` | those of the visited files whose outgoing relationships the traversal reads |
| `graph` with `direction: in`, `analyze_impact` | those of every analyzed file in a family of a focus file, plus every unresolved JS/TS relationship and every unresolved Rust `include` (either can name a file of any language) |
| `graph` with `direction: both` | the union of the two rows above |

An irrelevant one is not listed, but still counts in
`details.relationships.unresolved`. So an unresolved import never implies that
a file has no dependents: an `analyze_impact` result with a relevant
unresolved relationship is never `no_findings`.

**Not modeled.** These relationships are not seen or not followed:

1. Python `sys.path` entries other than the roots (`PYTHONPATH`, `.pth`
   files, test-runner `pythonpath` settings): a module found only through them
   is `external` or `unresolved`.
2. Implicit relationships without an import construct: Go files of one
   package use each other without an import; Python parent-package
   `__init__.py` files that an import also runs; Rust paths in expressions
   without `use`; JavaScript globals.
3. Bundler constructs such as Vite's `import.meta.glob`, and relative imports
   inside a namespace-only Python tree (no `__init__.py`), which are
   `unresolved` (`relative import outside a package`) although Python accepts
   them when the module runs as part of the package.
4. Dynamic imports with a computed specifier (`import(name)`, `require(x)`):
   extracted and `unresolved`, never dropped.
5. Other constructs: Python `importlib` and `__import__`, JS
   `new URL(…, import.meta.url)`, triple-slash references, Rust `macro_rules!`
   imports.
6. Build configuration: Go build constraints, Rust `cfg`,
   `cfg_attr(…, path)` and features, bundler aliases (webpack, Vite) and
   root-relative `/…` specifiers, `package.json` `exports` and `imports` maps,
   non-relative `tsconfig` `extends` and project `references`, `go.work` (each
   workspace module is found through its own `go.mod`).
7. Names the rules cannot map: a Python import name that differs from its
   distribution name (`yaml` for `PyYAML`) is `unresolved`; Rust macro output
   other than the passed-through `mod`, `use` and item forms is not seen, so a
   `use` into it is `unresolved`; Rust 2015-edition paths; a package name that
   resolves only into a vendored directory is `external`, so the dependents of
   a vendored file are found only through relative paths.
8. Symbol-level relationships: an edge says that a file imports another file,
   not which symbols it uses or whether it uses them.

**`graph`.** The findings are edge objects with one more field, `depth`.
Without `focus`, every edge whose source is an analyzed file, with `depth`
`null`; `direction` and `max_depth` are errors there. With `focus`, a
breadth-first traversal from the focus files (distance 0) visits files in
distance order, then path order: `out` follows edges from a visited file to
its targets, `in` from their sources to a visited file. An edge is reported
when its near end (the source for `out`, the target for `in`) has a distance
*d* below `max_depth`; its `depth` is *d* + 1. `both` reports the union, an
edge found by both with the smaller `depth`. When an edge that the traversal
would follow from a file at distance `max_depth` is not in the result,
`limits_hit` has `{"limit":"max_depth","value":<max_depth>,"detail":"<n> files
at depth <max_depth> have relationships that were not followed"}` and
`complete` is `false`. Order: `depth`, then `file`, then `to`. `details` adds
`direction` and `max_depth` (the effective values, `null` without focus),
`focus` (the focus paths, at most 30) with `focus_total`, `focus_files` (the
number of focus files), `focus_not_analyzed` (focus paths under which no file
was analyzed, at most 30), `focus_files_not_analyzed` (each focus file that is
not analyzed, `{ "file", "reason" }`, at most 30), each with its `_total`,
`files` (the distinct files in all reported edges, before the `limit` cut) and
`not_followed` (the targets with `to_analyzed: false` that the traversal
reached, `{ "file" }`, at most 100, with `not_followed_total`). A reason is one
of `excluded as tests`, `excluded as generated`, `excluded as vendored`,
`excluded as ignored`, `left out by include or exclude`, `unrecognized`,
`language not supported: <language>` and `skipped: <kind>`.

**`analyze_cycles`.** The cycle graph is the set of edges among analyzed files
with `to_analyzed: true`, without the edges whose only kind is `module`: a
Rust `mod x;` states that a file contains a child module, and nearly every
child refers back to its parent, so counting containment would make almost
every crate one cycle. An edge with `module` and another kind counts. Each
strongly connected component of two or more files is one finding: `file` (the
member first in path order), `members` (in path order, at most 20, with
`members_total`), `edges_total` (the edges between members), and `path`, one
shortest cycle through `file`, as steps, starting and ending at `file` (a
breadth-first search from `file` over edges inside the component, visiting
targets in path order, stopping at the first edge back to `file`; at most 20
steps, with `path_total`). Order: `members_total`, highest first, then `file`.
`details` adds `cyclic_files` (the files in all components, before the `limit`
cut) and `not_followed` with `not_followed_total` (the targets with
`to_analyzed: false`, which the cycle graph leaves out). `no_findings` means
no cycle among the analyzed files, on complete evidence. Every unresolved
relationship except kind `module` makes `complete` `false`, because it could
close a cycle.

**`analyze_coupling`.** Over the edges (one per source and target, so repeated
imports and several kinds count once; a self-relationship counts zero):

| Value | Formula |
|---|---|
| `fan_in` | the number of distinct analyzed files with an edge to the file |
| `fan_out` | the number of distinct workspace files the file has an edge to |
| `degree` | `fan_in + fan_out` |
| `instability` | `fan_out / (fan_in + fan_out)`, rounded to 3 decimals: 0 = only depended on, 1 = only depending |
| `external`, `unresolved` | the file's relationships with that outcome; in no formula, they show how far `fan_out` is a lower bound |

One finding per analyzed file with `degree` 1 or more:
`{ "file", "fan_in", "fan_out", "degree", "instability", "external",
"unresolved" }`. Order: `degree`, highest first, then `fan_in`, highest first,
then `file`. `details` adds `formulas` (the four formulas, as text) and
`edges` (the edges among the analyzed sources). `fan_in` counts only analyzed
files: a scope narrower than the workspace undercounts it.

**`analyze_impact`.** A dependent is a file that imports a focus file, directly or through other files. It is exposed to a change in the focus file. This is not proof that it will break, and a file that is not listed is not proof that it is safe: relationships that act-lite does not model (see the README) are not followed.

`focus` is required. The traversal is `graph`'s `in` traversal from the focus
files, with `max_depth` 1000 by default; each analyzed file reached that is not
itself a focus file is one finding: `file`, `depth` (the distance to the
nearest focus file: 1 is a direct dependent, more is transitive), `chain` (the
steps from the dependent to a focus file, fixed by the traversal: each file
keeps the edge through which it was first reached; at most 20 steps, with
`chain_total`, equal to `depth`), and `reaches` (the focus file at the end of
`chain`). Order: `depth`, then `file`. When a file at distance `max_depth` has
a dependent that is neither a focus file nor reached, `limits_hit` has
`{"limit":"max_depth","value":<max_depth>,"detail":"<n> files at depth
<max_depth> have dependents that were not followed"}` and `complete` is
`false`. `details` adds `max_depth`, the `focus` keys of `graph`, and `direct`
and `transitive` (dependents at depth 1 and at depth 2 or more, before the
`limit` cut). A focus file that depends on another focus file is not a
dependent. `no_findings` means no dependent, on complete evidence.

**Request errors.** In addition to the shared ones, checked in this order
after `--limit`:

| Request error | Message |
|---|---|
| `--max-depth` outside 1 to 1000 | `max_depth must be between 1 and 1000, got <n>` |
| `direction` not `in`, `out` or `both` (over MCP; the command line rejects it with exit 2 and lists the values) | `direction must be one of in, out, both, got <value>` |
| `analyze impact` without `--focus` | `impact needs at least one focus path` |
| `analyze graph` with `--max-depth` and no `--focus` | `max_depth needs a focus path` |
| `analyze graph` with `--direction` and no `--focus` | `direction needs a focus path` |
| A `--focus` path does not exist | the not-found text |
| A `--focus` path is outside the workspace root | `path must be inside the workspace root: <path>` |
| A `--focus` path is not at or below a `PATH` argument | `focus path is not inside the requested paths: <path>` |
| `analyze graph --direction out` and no focus file is analyzed (checked after discovery, after the empty-scope check) | `no focus file can be analyzed: <file> (<reason>)[, …]`, at most 10 entries, then `, …`; a focus directory with no file in scope is written `<dir>/ (no file)` |

A `--max-depth` that is not an integer from 0 to 4294967295 is rejected with
exit 2.

**Bounds.** These analyzers read every in-scope file of up to 8 MiB and have no
hard time or memory cap; cancel a run to bound its time. The only analysis
bound is `max_depth`, so the `limits_hit` of `analyze_cycles` and
`analyze_coupling` is always empty. Nested lists are cut at: `evidence` 5,
`members`, `path` and `chain` 20, `focus`, `focus_not_analyzed` and
`focus_files_not_analyzed` 30, `resolution_inputs` and `not_followed` 100. They
start and use no language server: `evidence` is
`{"sources":["tree-sitter"],"language_servers":[]}` when a file is analyzed.

**Examples.** The workspace `acyclic` holds an empty `.git/` directory and four
files:

```text
src/a.ts   import { b } from './b';
           import fs from 'node:fs';
src/b.ts   import { c } from './c';
src/c.ts   export const c = 1;
src/d.ts   import './b';
           import './c';
```

The edges are `src/a.ts` → `src/b.ts`, `src/b.ts` → `src/c.ts`,
`src/d.ts` → `src/b.ts` and `src/d.ts` → `src/c.ts`, with one `external`
(`node:fs`). In every example below, `complete` is `true`, `total` is the
number of findings, `truncated` is `false`, `coverage` is 4 analyzed
TypeScript files, every `excluded` and `incomplete` list is empty, and
`details.relationships` is
`{"total":5,"resolved":4,"external":1,"unresolved":0}` with
`"kinds":["import"]` and no resolution input.

`act-lite --workspace acyclic analyze graph --focus src/b.ts --direction in`
(MCP: `graph` with `{"focus":["src/b.ts"],"direction":"in"}`) gives `status`
`findings` and:

```json
"findings":[{"file":"src/a.ts","to":"src/b.ts","to_analyzed":true,"depth":1,"kinds":["import"],"evidence":[{"range":"1:1-1:25","kind":"import","specifier":"./b"}],"evidence_total":1},
            {"file":"src/d.ts","to":"src/b.ts","to_analyzed":true,"depth":1,"kinds":["import"],"evidence":[{"range":"1:1-1:14","kind":"import","specifier":"./b"}],"evidence_total":1}]
```

with `details` `"direction":"in"`, `"max_depth":1`, `"files":3`,
`"focus":["src/b.ts"]` and `"focus_files":1`.

`act-lite --workspace acyclic analyze coupling` (MCP: `analyze_coupling` with
`{}`) gives `status` `findings` and:

```json
"findings":[{"file":"src/b.ts","fan_in":2,"fan_out":1,"degree":3,"instability":0.333,"external":0,"unresolved":0},
            {"file":"src/c.ts","fan_in":2,"fan_out":0,"degree":2,"instability":0,"external":0,"unresolved":0},
            {"file":"src/d.ts","fan_in":0,"fan_out":2,"degree":2,"instability":1,"external":0,"unresolved":0},
            {"file":"src/a.ts","fan_in":0,"fan_out":1,"degree":1,"instability":1,"external":1,"unresolved":0}]
```

with `details` `"edges":4`.

`act-lite --workspace acyclic analyze impact --focus src/c.ts` (MCP:
`analyze_impact` with `{"focus":["src/c.ts"]}`) gives `status` `findings`
and:

```json
"findings":[{"file":"src/b.ts","depth":1,"reaches":"src/c.ts","chain":[{"file":"src/b.ts","range":"1:1-1:25","kind":"import","specifier":"./c","to":"src/c.ts"}],"chain_total":1},
            {"file":"src/d.ts","depth":1,"reaches":"src/c.ts","chain":[{"file":"src/d.ts","range":"2:1-2:14","kind":"import","specifier":"./c","to":"src/c.ts"}],"chain_total":1},
            {"file":"src/a.ts","depth":2,"reaches":"src/c.ts","chain":[{"file":"src/a.ts","range":"1:1-1:25","kind":"import","specifier":"./b","to":"src/b.ts"},{"file":"src/b.ts","range":"1:1-1:25","kind":"import","specifier":"./c","to":"src/c.ts"}],"chain_total":2}]
```

with `details` `"max_depth":1000`, `"direct":2` and `"transitive":1`.

`act-lite --workspace acyclic analyze cycles` (MCP: `analyze_cycles` with
`{}`) gives `status` `no_findings`, `findings` `[]` and `details`
`"cyclic_files":0`. The workspace `cyclic` holds an empty `.git/` directory
and three files:

```text
src/a.ts     import { b } from './b';
             export const a = 1;
src/b.ts     import { a } from './a';
             export const b = a;
src/main.ts  import { a } from './a';
```

`act-lite --workspace cyclic analyze cycles` gives `status` `findings`,
`complete` `true`, `details.relationships`
`{"total":3,"resolved":3,"external":0,"unresolved":0}`, `"cyclic_files":2`,
and:

```json
"findings":[{"file":"src/a.ts","members":[{"file":"src/a.ts"},{"file":"src/b.ts"}],"members_total":2,"edges_total":2,
             "path":[{"file":"src/a.ts","range":"1:1-1:25","kind":"import","specifier":"./b","to":"src/b.ts"},
                     {"file":"src/b.ts","range":"1:1-1:25","kind":"import","specifier":"./a","to":"src/a.ts"}],"path_total":2}]
```

**Fixed name lists.** The Node built-in modules (the `node:module`
`builtinModules` list): `_http_agent`, `_http_client`, `_http_common`, `_http_incoming`, `_http_outgoing`, `_http_server`, `_stream_duplex`, `_stream_passthrough`, `_stream_readable`, `_stream_transform`, `_stream_wrap`, `_stream_writable`, `_tls_common`, `_tls_wrap`, `assert`, `assert/strict`, `async_hooks`, `buffer`, `child_process`, `cluster`, `console`, `constants`, `crypto`, `dgram`, `diagnostics_channel`, `dns`, `dns/promises`, `domain`, `events`, `fs`, `fs/promises`, `http`, `http2`, `https`, `inspector`, `inspector/promises`, `module`, `net`, `os`, `path`, `path/posix`, `path/win32`, `perf_hooks`, `process`, `punycode`, `querystring`, `readline`, `readline/promises`, `repl`, `stream`, `stream/consumers`, `stream/promises`, `stream/web`, `string_decoder`, `sys`, `timers`, `timers/promises`, `tls`, `trace_events`, `tty`, `url`, `util`, `util/types`, `v8`, `vm`, `wasi`, `worker_threads`, `zlib`.

The Python standard-library modules (Python 3.13's `sys.stdlib_module_names`):
`__future__`, `_abc`, `_aix_support`, `_android_support`, `_apple_support`, `_ast`, `_asyncio`, `_bisect`, `_blake2`, `_bz2`, `_codecs`, `_codecs_cn`, `_codecs_hk`, `_codecs_iso2022`, `_codecs_jp`, `_codecs_kr`, `_codecs_tw`, `_collections`, `_collections_abc`, `_colorize`, `_compat_pickle`, `_compression`, `_contextvars`, `_csv`, `_ctypes`, `_curses`, `_curses_panel`, `_datetime`, `_dbm`, `_decimal`, `_elementtree`, `_frozen_importlib`, `_frozen_importlib_external`, `_functools`, `_gdbm`, `_hashlib`, `_heapq`, `_imp`, `_interpchannels`, `_interpqueues`, `_interpreters`, `_io`, `_ios_support`, `_json`, `_locale`, `_lsprof`, `_lzma`, `_markupbase`, `_md5`, `_multibytecodec`, `_multiprocessing`, `_opcode`, `_opcode_metadata`, `_operator`, `_osx_support`, `_overlapped`, `_pickle`, `_posixshmem`, `_posixsubprocess`, `_py_abc`, `_pydatetime`, `_pydecimal`, `_pyio`, `_pylong`, `_pyrepl`, `_queue`, `_random`, `_scproxy`, `_sha1`, `_sha2`, `_sha3`, `_signal`, `_sitebuiltins`, `_socket`, `_sqlite3`, `_sre`, `_ssl`, `_stat`, `_statistics`, `_string`, `_strptime`, `_struct`, `_suggestions`, `_symtable`, `_sysconfig`, `_thread`, `_threading_local`, `_tkinter`, `_tokenize`, `_tracemalloc`, `_typing`, `_uuid`, `_warnings`, `_weakref`, `_weakrefset`, `_winapi`, `_wmi`, `_zoneinfo`, `abc`, `antigravity`, `argparse`, `array`, `ast`, `asyncio`, `atexit`, `base64`, `bdb`, `binascii`, `bisect`, `builtins`, `bz2`, `cProfile`, `calendar`, `cmath`, `cmd`, `code`, `codecs`, `codeop`, `collections`, `colorsys`, `compileall`, `concurrent`, `configparser`, `contextlib`, `contextvars`, `copy`, `copyreg`, `csv`, `ctypes`, `curses`, `dataclasses`, `datetime`, `dbm`, `decimal`, `difflib`, `dis`, `doctest`, `email`, `encodings`, `ensurepip`, `enum`, `errno`, `faulthandler`, `fcntl`, `filecmp`, `fileinput`, `fnmatch`, `fractions`, `ftplib`, `functools`, `gc`, `genericpath`, `getopt`, `getpass`, `gettext`, `glob`, `graphlib`, `grp`, `gzip`, `hashlib`, `heapq`, `hmac`, `html`, `http`, `idlelib`, `imaplib`, `importlib`, `inspect`, `io`, `ipaddress`, `itertools`, `json`, `keyword`, `linecache`, `locale`, `logging`, `lzma`, `mailbox`, `marshal`, `math`, `mimetypes`, `mmap`, `modulefinder`, `msvcrt`, `multiprocessing`, `netrc`, `nt`, `ntpath`, `nturl2path`, `numbers`, `opcode`, `operator`, `optparse`, `os`, `pathlib`, `pdb`, `pickle`, `pickletools`, `pkgutil`, `platform`, `plistlib`, `poplib`, `posix`, `posixpath`, `pprint`, `profile`, `pstats`, `pty`, `pwd`, `py_compile`, `pyclbr`, `pydoc`, `pydoc_data`, `pyexpat`, `queue`, `quopri`, `random`, `re`, `readline`, `reprlib`, `resource`, `rlcompleter`, `runpy`, `sched`, `secrets`, `select`, `selectors`, `shelve`, `shlex`, `shutil`, `signal`, `site`, `smtplib`, `socket`, `socketserver`, `sqlite3`, `sre_compile`, `sre_constants`, `sre_parse`, `ssl`, `stat`, `statistics`, `string`, `stringprep`, `struct`, `subprocess`, `symtable`, `sys`, `sysconfig`, `syslog`, `tabnanny`, `tarfile`, `tempfile`, `termios`, `textwrap`, `this`, `threading`, `time`, `timeit`, `tkinter`, `token`, `tokenize`, `tomllib`, `trace`, `traceback`, `tracemalloc`, `tty`, `turtle`, `turtledemo`, `types`, `typing`, `unicodedata`, `unittest`, `urllib`, `uuid`, `venv`, `warnings`, `wave`, `weakref`, `webbrowser`, `winreg`, `winsound`, `wsgiref`, `xml`, `xmlrpc`, `zipapp`, `zipfile`, `zipimport`, `zlib`, `zoneinfo`.

## Repository discovery

An analyzer walks each requested path below the workspace root: a directory
recursively, a file as itself. With no path, it walks the root. A relative
path resolves against the workspace root. A path that does not exist, or
that is outside the root after symlinks and `..` are resolved, is an error
(`path must be inside the workspace root: <path>`). A file that two paths
reach is analyzed once. No analyzer stops or refuses because the scope holds
many files.

Discovery applies these steps in order. The first step that matches decides
what happens to an entry, and in which `excluded` list it is reported:

| Step | Matches | Result | Bypassed by |
|---|---|---|---|
| 0 | An entry named `.git` | left out, not reported | — |
| 1 | An ignored path | `excluded.ignored` | `--include-ignored` |
| 2 | A vendored directory | `excluded.vendored` | `--include-vendored` |
| 3 | A generated directory | `excluded.generated` | `--include-generated` |
| 4 | Fails `--include` or matches `--exclude` | left out, not reported | — |
| 5 | Not recognized by any grammar | counted in `coverage.unrecognized_files` | — |
| 6 | A test file, by name | `excluded.tests` | `--include-tests` |
| 7 | A generated file, by name | `excluded.generated` | `--include-generated` |
| 8 | A symlink | not followed (below) | — |
| 9 | A generated file, by header | `excluded.generated` | `--include-generated` |
| — | Otherwise | in scope: counted in `coverage.languages`, then read and parsed | — |

Steps 1–3 prune directories before the globs apply, so an excluded directory
is reported even when `--include` matches nothing in it. Steps 2–3 apply to
real directories only. Steps 1 and 4 apply to a symlink by its name; steps
5–7 and 9 do not apply to a symlink. Tests are excluded by default for every
analyzer. Every name and pattern below is case-sensitive unless stated.

**Named paths.** A named path follows the same steps. Steps 0–3 also apply to
each directory between the workspace root and the path: a named
`vendor/dep/lib.rs` is excluded by its `vendor` directory and reported as
`excluded.vendored` `[{"path":"vendor/dep/lib.rs"}]`. A named test file is
excluded unless `--include-tests` is given. When nothing is left to analyze,
the request is the empty-scope error, which names the include options. A
named file that no grammar recognizes is counted in
`coverage.unrecognized_files`, not refused.

**Ignored paths.** A path is ignored when git's `gitignore` pattern rules
match it. The patterns come from the `.gitignore` files in the workspace and
in its ancestors up to the work-tree top, and from `.git/info/exclude` at the
work-tree top. The user's global excludes file is not read, and `git` is not
run. The rules apply only when the workspace root or an ancestor contains a
`.git` entry; the work-tree top is the nearest such directory. A directory
below the root that contains a `.git` entry (a nested repository or a
submodule) is walked; its own patterns apply to its contents, and the outer
patterns do not. The root and its ancestors are never ignored. An ignored
directory is reported once, as `"<dir>/"`. An ignored file is reported only
when a grammar recognizes it.

**Vendored directories.** A directory, at any depth, named `vendor`,
`third_party`, `third-party`, `node_modules`, `bower_components`, `Pods` or
`Carthage`, or a directory that contains a `pyvenv.cfg` file (a Python
virtual environment). It is reported once, as `"<dir>/"`, and is not walked.

**Test files.** A file is a test file when a directory component of its
relative path is `test`, `tests`, `__tests__`, `spec` or `testdata`, or when
its name matches a pattern of its language:

| Language | Test file-name patterns |
|---|---|
| Go | `*_test.go` |
| Python | `test_*.py`, `*_test.py`, `conftest.py` |
| JavaScript, JSX, TypeScript, TSX | `*.test.<ext>`, `*.spec.<ext>` for `js`, `mjs`, `cjs`, `jsx`, `ts`, `tsx` |
| Java, Kotlin, Scala, C#, Swift, PHP | `*Test.<ext>`, `*Tests.<ext>`, `*Spec.<ext>` for `java`, `kt`, `kts`, `scala`, `cs`, `swift`, `php` |
| Dart | `*_test.dart` |
| Ruby | `*_spec.rb`, `*_test.rb`, `test_*.rb` |
| Elixir | `*_test.exs` |
| C, C++ | `*_test.<ext>`, `*_unittest.<ext>`, `test_*.<ext>` for `c`, `cc`, `cpp`, `cxx` |
| Lua | `*_spec.lua`, `*_test.lua` |
| Bash | `*_test.sh`, `test_*.sh` |
| HCL/Terraform | `*.tftest.hcl` |
| Rust, CSS, SCSS, HTML, JSON, YAML, TOML, Markdown, SQL, Dockerfile, JSONC, XML, GraphQL, Protocol Buffers, Make, CMake, Zig, R, Julia, Nix | the directory rule only |

R `testthat` reads test files only from `tests/testthat/`, which the directory
rule covers; `R/test_x.R` is package source. Julia's `test/runtests.jl` is
covered by the directory rule.

**Generated files and directories.**

- Directory: a directory that contains a `CACHEDIR.TAG` file whose first 43
  bytes are `Signature: 8a477f597d28d172789f06886806bc55` (the Cache Directory
  Tagging Specification; Cargo writes one into `target/`). It is reported
  once, as `"<dir>/"`.
- File name: `*.min.js`, `*.min.css`, `*.pb.go`, `*_pb2.py`,
  `*_pb2_grpc.py`, `*.pb.cc`, `*.pb.h`, `*.g.dart`, `*.freezed.dart`,
  `*.designer.cs` (case-insensitive), `*.g.cs`, `*.generated.*`,
  `package-lock.json`, `pnpm-lock.yaml`.
- Header: a line among the first 20 lines (and the first 4,096 bytes) that
  starts with a comment opener (`//`, `/*`, `*`, `#`, `--`, `<!--` or `;`,
  after leading whitespace) and, on the same line, contains `Code generated`
  followed later by `DO NOT EDIT`, or `@generated`. A string literal
  `"@generated"` does not match. Only regular files in a language the
  analyzer supports are read for the header, also when they are larger than
  the size bound. When the header cannot be read, the file is in
  `skipped_files` with kind `unreadable`.

**Tests embedded in source files.**

| Language | Embedded test code | Treatment without `--include-tests` |
|---|---|---|
| Rust | An item with the attribute `#[cfg(test)]` (usually `mod tests { … }`), and a function with an attribute whose last path segment is `test` (`#[test]`, `#[tokio::test]`) outside such an item | The item's range, from its first attribute to its end, is removed from the analysis input and reported as `excluded.tests` `{ "path", "range" }`. |
| Go | None: Go test code lives only in `*_test.go` files | — |
| Zig | A `test` declaration (`test "name" { … }`, `test name { … }`, `test { … }`) | The declaration's range, from `test` to the closing `}`, is removed from the analysis input and reported as `excluded.tests` `{ "path", "range" }`, as for Rust. |
| Julia | `@test` and `@testset` in source files | Not detected. Not removed. |
| Python | Doctests in docstrings are string content, not code | Not detected. Not removed. |
| Rust, other `cfg` forms | `#![cfg(test)]` as an inner attribute of a file; `#[cfg(all(test, …))]`; `#[cfg(any(test, …))]`; `#[cfg_attr(test, …)]` | A file with `#![cfg(test)]` is a test file (whole file). An item with `cfg(all(test, …))` is a test item, like `cfg(test)`. `cfg(any(test, …))` and `cfg_attr(test, …)` are not test items: the code is also compiled outside tests. |
| Rust, out-of-line | `#[cfg(test)] mod tests;` loads `tests.rs` (or `tests/mod.rs`) beside the module | The declaration's range is removed and reported. The loaded file is excluded only by the directory rule (`tests/mod.rs`); a `src/tests.rs` is analyzed as source. |
| JavaScript/TypeScript | In-source tests (for example `if (import.meta.vitest)`) | Not detected. Not removed. |
| All other languages | No embedded-test convention is detected | — |

A parse error inside a removed region is not reported in `parse_failures`.

Files of the added languages are recognized files: they appear in `excluded.*`
and in its counts, and `coverage.unrecognized_files` falls. Findings of the
other languages do not change.

**Symlinks.** Discovery never follows a symlink. A symlink whose target is
inside the workspace is not reported; its target is analyzed at its own path
when it is in scope. A symlink named in a supported language whose target is
outside the workspace is in `skipped_files` with kind
`symlink_outside_workspace`; a broken one has kind `unreadable`. Both count
in `coverage.languages` for the language of the link's own name. A symlink
to a directory outside the workspace is in `skipped_files` as `"<link>/"`
with kind `symlink_outside_workspace`, and makes the result incomplete. Every
other symlink is neither counted nor reported.

**Walk errors and file names.** A directory that cannot be listed is in
`skipped_files` as `"<dir>/"` with kind `walk_error`. A file name that is not
valid UTF-8 gives kind `not_utf8`, with each invalid sequence written as
U+FFFD. A FIFO, socket or device file is not opened by discovery.

## Resource bounds and cancellation

- **Per file:** a file in a language the analyzer supports that is larger
  than 8 MiB (8,388,608 bytes) is not read. It is in `skipped_files` with
  kind `size_limit`, so the result is incomplete, not silently partial. A
  file in a language the analyzer does not support is counted in
  `coverage.languages` and never read, whatever its size.
- **Scope:** there is no file-count limit and no repository-size limit. No
  analyzer stops or refuses because the scope holds many files.
- **Time and memory:** there is no hard time cap and no hard memory cap. An
  analysis reads every in-scope file of up to 8 MiB, one file at a time.
  Cancellation is the way to bound time: on the command line, interrupt the
  process (for example with Ctrl-C), which then prints no result; over MCP,
  cancel the request, and the analysis stops at its next check.
- **Side effects:** an analyzer writes no file, inside or outside the
  workspace, keeps no cache, and starts no language server. The dependency
  analyzers also read configuration files and some files outside the scope
  (see [What the dependency analyzers read](#dependency-analyzers)).

## `act-lite env`

`act-lite env` keeps a project's secrets out of the working tree and out of
an agent's context. Values move to an out-of-repo store; each sequestered line
in `.env` becomes `KEY=#act:sequestered`.

```bash
act-lite env import .env            # sequester every plaintext line of a .env
act-lite env mv NAME                # sequester one existing .env key
act-lite env mv NAME --restore      # write the stored value back into .env
act-lite env add NAME               # add or update one secret (hidden prompt)
printf %s "$VALUE" | act-lite env add NAME --stdin
act-lite env rm NAME                # remove one secret from the store
act-lite env run -- <command>       # run with secrets injected, output redacted
act-lite env list                   # key names, never values
act-lite env get NAME               # redacted
act-lite env get NAME --reveal      # the plaintext value
```

- `import` replaces each plaintext value in `.env` with the marker, keeps
  every other line, and adds a short comment header once. `--force` overwrites a stored value that differs from the
  file's.
- `mv` sequesters one key; `--restore` reverses it. `--force` proceeds
  through a value collision.
- `add` reads the value from a hidden prompt, or from stdin with `--stdin`.
  The value is never an argument, so it does not land in shell history or an
  agent's context. `add` and `rm` change the store only; the repository's
  `.env` is not touched.
- `run` injects the project's secrets into the child process environment.
  The values never enter the command string, and the child's stdout and
  stderr are redacted as they stream. act-lite exits with the child's exit
  code. A value shorter than 6 characters is not redacted; `import`,
  `mv` and `add` warn when that applies.
- `get` prints a redacted line unless `--reveal` is given. `--reveal` is the
  explicit switch that accepts the value entering your terminal or context.

The store is `~/.config/act/secrets/<id>.env`, one file per project, created
with mode 0600. `<id>` is derived from the project root: the nearest ancestor
directory that contains `.git` (all linked worktrees of a repository share one
store), else the current directory. `--workspace <dir>` uses `<dir>` as the
root exactly as given, with no `.git` search. `ACT_CONFIG_DIR` replaces
`~/.config/act`; else `$XDG_CONFIG_HOME/act` is used when that variable is set.

act-lite and act101's `act` share the store location, the per-project id and
the marker. A `.env` sequestered by `act env` is read by `act-lite env`, and
the reverse; a user with both binaries sees one set of secrets per project.

`--format` has no effect on `env`; its output is the same under both values.

## Claude Code plugin

The `plugin/` directory is a Claude Code plugin that registers the MCP server
and adds nine skills: eight that you invoke and one shared skill.

- `define-work` turns a request into an approved design before code is
  written: repo recon, one question at a time, three fresh reads of the
  design (comprehension, critic, sufficiency), and an approval gate.
- `plan-work` plans the work as a resumable work graph.
- `execute-work` runs the work graph to completion.
- `deliver-work` takes finished work to a reviewed, CI-green pull request and
  reports it.
- `diagnose-defect` finds the root cause of a defect and fixes it with a test
  that failed first.
- `review-change` reviews a diff, branch or pull request for code health: the
  structural problems in the changed functions and the duplication that
  touches the change, as a report with no edit.
- `audit-debt` audits a repository or directory for technical debt: the files
  where complexity concentrates, the functions over structural thresholds in
  them, and the largest duplicated code, as a report with no edit.
- `map-repo` maps a repository for orientation: its languages and size, the
  files where complexity concentrates, and how those files connect to the rest,
  as a report with no edit.

`work-graph` is a shared skill, not one you invoke. It defines the work-graph
format that `plan-work` writes and `execute-work` runs, and holds the script
that checks a work graph. `plan-work` and `execute-work` load it.

Put `act-lite` on your `PATH`, then:

```text
/plugin marketplace add act101-ai/act-lite
/plugin install act-lite@act-lite-marketplace
```

## License

act-lite is licensed under the MIT license; see [LICENSE](LICENSE).
Third-party grammar licenses are listed in [NOTICE](NOTICE).
