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
