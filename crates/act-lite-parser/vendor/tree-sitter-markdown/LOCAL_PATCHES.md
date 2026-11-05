# tree-sitter-markdown local patches

Vendored at `crates/act-lite-parser/vendor/tree-sitter-markdown` (path
dependency `tree-sitter-markdown 0.7.1` in act-lite-parser; upstream
ikatyang/tree-sitter-markdown 0.7.1 from crates.io, MIT — Ika).

Initial state: the crates.io `tree-sitter-markdown-0.7.1` package.
`src/parser.c`, `src/node-types.json`, `src/tree_sitter/parser.h` and
`LICENSE` are unchanged. Not vendored: `bindings/`, `grammar.js`,
`src/grammar.json`, `CHANGELOG.md`, `README.md`.

## Rust packaging

- `Cargo.toml` rewritten: edition 2021; `build = "build.rs"`;
  `[lib] path = "lib.rs"`; depends on `tree-sitter-language 0.1` instead
  of `tree-sitter 0.19` (`tree-sitter 0.25` is a dev-dependency only), so
  the grammar links against the workspace's tree-sitter ABI; `include`
  limited to `src/*` and `LICENSE`.
- `build.rs` (replaces `bindings/rust/build.rs`): compiles `src/parser.c`
  as C11 and `src/scanner.cc` as C++ with
  `TREE_SITTER_MARKDOWN_AVOID_CRASH` defined, so internal asserts throw
  instead of calling `abort()`.
- `lib.rs` (replaces `bindings/rust/lib.rs`): exports `LANGUAGE`
  (`tree_sitter_language::LanguageFn`) and `NODE_TYPES`.

## Serialized-state size preflight

`Scanner::serialize` wrote the whole scanner state into tree-sitter's
1,024-byte serialization buffer and checked the size only after writing,
so a state that did not fit (for example hundreds of pending inline
delimiters on one long line) wrote past the buffer. It now computes the
byte count first and returns an empty state (0) when it does not fit; a
state that fits is serialized exactly as before.

- `src/tree_sitter_markdown/lexer.h`, `src/tree_sitter_markdown/lexer.cc`,
  `src/tree_sitter_markdown/block_context.h`,
  `src/tree_sitter_markdown/block_context.cc`,
  `src/tree_sitter_markdown/block_delimiter.h`,
  `src/tree_sitter_markdown/block_delimiter.cc`,
  `src/tree_sitter_markdown/inline_delimiter.h`,
  `src/tree_sitter_markdown/inline_delimiter.cc`: each class gains
  `size_t serialized_size() const`, the byte count its `serialize` writes.
- `src/scanner.cc`: `Scanner::serialize` sums the four sizes plus one,
  returns 0 above `TREE_SITTER_SERIALIZATION_BUFFER_SIZE`, and asserts
  `i == expected_size` (was `i <= TREE_SITTER_SERIALIZATION_BUFFER_SIZE`).

## Exception catch in serialize and deserialize

With `TREE_SITTER_MARKDOWN_AVOID_CRASH`, an internal assert throws a C++
exception. Upstream catches it only in the scan entry point; an exception
from serialize or deserialize crossed the C frames into Rust and aborted
the process.

- `src/scanner.cc`: `tree_sitter_markdown_external_scanner_serialize`
  catches and returns 0; `tree_sitter_markdown_external_scanner_deserialize`
  catches and leaves the scanner as deserialize left it. Without the
  define, both call through unchanged.
