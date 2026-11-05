# act-lite

act-lite is an open-source, navigation-only code engine for AI agents from
act101. It parses source files with tree-sitter and answers structural
questions without reading whole files: a file's skeleton, its symbols, a
repository outline, where a symbol is defined, and where it is used. It does
not edit code. It also sequesters a project's secrets out of the working
tree and runs commands with them injected and redacted (`act-lite env`); the
only file it writes is the project's `.env`, and only the value tokens of the
lines it sequesters.

act-lite ships one binary, `act-lite`, with three commands:

- `act-lite query` runs one query and prints JSON.
- `act-lite mcp serve` serves the same queries as MCP tools over stdio.
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

Install a prebuilt binary from the GitHub releases and put `act-lite` on
your `PATH`, or build it from source with `cargo build`.

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

The server exposes eight tools: `status`, `skeleton`, `symbols`,
`symbols_batch`, `repo_outline`, `references`, `callers`, `definition`.
`status` lists the 30 languages and, for each language server, whether it is
`ready` or `not_installed`.

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
and adds five skills:

- `define-work` turns a request into an approved design before code is
  written: repo recon, one question at a time, three fresh reads of the
  design (comprehension, critic, sufficiency), and an approval gate.
- `plan-work` plans the work as a resumable work graph.
- `execute-work` runs the work graph to completion.
- `deliver-work` takes finished work to a reviewed, CI-green pull request and
  reports it.
- `diagnose-defect` finds the root cause of a defect and fixes it with a test
  that failed first.

Put `act-lite` on your `PATH`, then:

```text
/plugin marketplace add act101-ai/act-lite
/plugin install act-lite@act-lite-marketplace
```

## License

act-lite is licensed under the MIT license; see [LICENSE](LICENSE).
Third-party grammar licenses are listed in [NOTICE](NOTICE).
