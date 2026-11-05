# act-lite

act-lite is an open-source code engine for AI agents from act101. It parses
source files with tree-sitter and answers structural questions without reading
whole files: a file's skeleton, its symbols, a repository outline, where a
symbol is defined, and where it is used. It also measures code health:
functions over size, nesting, branching or parameter thresholds, the files
where complexity concentrates, and duplicated code. It does not edit code. It also sequesters a project's secrets out of the working
tree and runs commands with them injected and redacted (`act-lite env`); the
only file it writes is the project's `.env`, and only the value tokens of the
lines it sequesters.

act-lite ships one binary, `act-lite`, with four commands:

- `act-lite query` runs one query and prints JSON.
- `act-lite analyze` runs one code-health analysis and prints JSON.
- `act-lite mcp serve` serves the same queries and analyses as MCP tools over
  stdio.
- `act-lite env` sequesters secrets, lists them by name, and runs commands
  with them injected and their output redacted.

## Languages

act-lite supports 30 languages:

Bash, C, C#, C++, CSS, Dart, Dockerfile, Elixir, Go, HCL/Terraform, HTML,
Java, JavaScript, JSX, JSON, Kotlin, Lua, Markdown, PHP, Python, Ruby, Rust,
Scala, SCSS, SQL, Swift, TOML, TSX, TypeScript, YAML.

A file in any other language is refused with an error that names its
extension and lists these languages.

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
  The `references` fallback searches every same-language file under it.
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
cuts the list, the output says `"truncated": true`.

## `act-lite mcp serve`

```bash
act-lite mcp serve                    # workspace: nearest git root, else the current directory
act-lite --workspace /path/to/repo mcp serve
```

The server exposes eleven tools: `status`, `skeleton`, `symbols`,
`symbols_batch`, `repo_outline`, `references`, `callers`, `definition`, and
the analyzers `analyze_patterns`, `analyze_hotspots` and `analyze_clones`
(see [Code-health analyzers](#code-health-analyzers)).
`status` lists the 30 languages and, for each language server, whether it is
`ready` or `not_installed`.

**Analyzer tools.** Each `analyze_*` tool returns the same JSON
as its `act-lite analyze` command (see [`act-lite analyze`](#act-lite-analyze)):
for the same workspace root, language-server setting and parameters, the first
content block, parsed, equals the command's stdout, parsed. The tool
parameters follow one rule: a parameter `snake_case` is the flag
`--kebab-case`, an array is a repeatable flag, a boolean (default `false`) is a
presence flag, and `paths` is the positional arguments. `--workspace`,
`--format` and `--disable-lsp` are fixed when the server starts. An array can
also be sent as a string that holds a JSON array, and `limit` as a numeric
string. A request error is a JSON-RPC `invalid_params` error (-32602) with the
command's message; an internal failure is `internal_error` (-32603). A
cancelled request stops its analysis and gets no response.

The second content block is a hint, and is not part of the result. When
`truncated` is `true`, it gives `total` and names the parameters that narrow
the result: `paths`, `include`, `exclude` and `limit`. When `excluded` or
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
prefix, with `_` changed to `-`; the `analysis` value in the result is the
command name. A tool parameter `snake_case` is the flag `--kebab-case`, an
array is a repeatable flag, and a boolean is a presence flag. `paths` is the
positional arguments.

| MCP tool | Command | `analysis` |
|---|---|---|
| `analyze_patterns` | `act-lite analyze patterns` | `patterns` |
| `analyze_hotspots` | `act-lite analyze hotspots` | `hotspots` |
| `analyze_clones` | `act-lite analyze clones` | `clones` |

The dependency analyzers (cycles, coupling, impact and graph) arrive in a later
release.

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
arguments, `--include` and `--exclude`, or change `--limit`. `--limit 0`
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

**Languages.** The three analyzers support 20 programming languages: Bash, C,
C++, C#, Dart, Elixir, Go, Java, JavaScript, JSX, Kotlin, Lua, PHP, Python,
Ruby, Rust, Scala, Swift, TSX and TypeScript. They do not support the 10 data
and markup formats: CSS, Dockerfile, HCL/Terraform, HTML, JSON, Markdown, SCSS,
SQL, TOML and YAML. Those files are listed in `coverage.languages` with
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
  Kotlin `string_literal`, `character_literal`, `integer_literal`,
  `long_literal`, `hex_literal`, `bin_literal`, `unsigned_literal`,
  `real_literal`; Lua `string`, `number`; PHP `string`, `encapsed_string`,
  `heredoc`, `nowdoc`, `integer`, `float`; Python `string`,
  `concatenated_string`, `integer`, `float`; Ruby `string`, `chained_string`,
  `heredoc_body`, `character`, `simple_symbol`, `delimited_symbol`,
  `hash_key_symbol`, `regex`, `integer`, `float`, `rational`, `complex`; Rust
  `string_literal`, `raw_string_literal`, `char_literal`, `integer_literal`,
  `float_literal`; Scala `string`, `interpolated_string`,
  `character_literal`, `integer_literal`, `floating_point_literal`; Swift
  `line_string_literal`, `multi_line_string_literal`, `raw_string_literal`,
  `regex_literal`, `integer_literal`, `hex_literal`, `oct_literal`,
  `bin_literal`, `real_literal`. A string with an embedded expression is not
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
| Rust, CSS, SCSS, HTML, JSON, YAML, TOML, Markdown, SQL, Dockerfile | the directory rule only |

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
| Python | Doctests in docstrings are string content, not code | Not detected. Not removed. |
| Rust, other `cfg` forms | `#![cfg(test)]` as an inner attribute of a file; `#[cfg(all(test, …))]`; `#[cfg(any(test, …))]`; `#[cfg_attr(test, …)]` | A file with `#![cfg(test)]` is a test file (whole file). An item with `cfg(all(test, …))` is a test item, like `cfg(test)`. `cfg(any(test, …))` and `cfg_attr(test, …)` are not test items: the code is also compiled outside tests. |
| Rust, out-of-line | `#[cfg(test)] mod tests;` loads `tests.rs` (or `tests/mod.rs`) beside the module | The declaration's range is removed and reported. The loaded file is excluded only by the directory rule (`tests/mod.rs`); a `src/tests.rs` is analyzed as source. |
| JavaScript/TypeScript | In-source tests (for example `if (import.meta.vitest)`) | Not detected. Not removed. |
| All other languages | No embedded-test convention is detected | — |

A parse error inside a removed region is not reported in `parse_failures`.

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
  workspace, keeps no cache, and starts no language server.

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
and adds seven skills:

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

Put `act-lite` on your `PATH`, then:

```text
/plugin marketplace add act101-ai/act-lite
/plugin install act-lite@act-lite-marketplace
```

## License

act-lite is licensed under the MIT license; see [LICENSE](LICENSE).
Third-party grammar licenses are listed in [NOTICE](NOTICE).
