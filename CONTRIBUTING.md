# Contributing

## Build

```sh
cargo build --release
```

The binary is `target/release/act-lite`.

## Checks before a PR

CI runs these checks. Run them before you open a pull request:

```sh
cargo fmt --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
python3 -m unittest discover -s tests/scripts -v
```

## License

Contributions are licensed under the MIT license of this repository
(`LICENSE`).
