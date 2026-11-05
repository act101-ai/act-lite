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

## Releases

One version everywhere. `[workspace.package] version` in `Cargo.toml`,
`plugin/.claude-plugin/plugin.json` and the `act-lite` entry of
`.claude-plugin/marketplace.json` carry the same version, and a release tag is
`v` followed by that version. `cargo test -p act-lite-cli --test plugin_version`
fails when they differ; `release.yml` refuses a tag that does not match the
crate version. Bump all three in the same commit, on the branch that goes
through the pull request (`main` is protected), before the tag.

CI and the release `floor` job regenerate `THIRD-PARTY-LICENSES` with
`cargo about generate --config about.toml --output-file THIRD-PARTY-LICENSES about.hbs`
and fail on a difference, so run that command after any dependency change.

A tag is never moved or deleted once it is published. A defect in a release
is fixed forward: a patch version on a `release/<major>.<minor>` branch cut
from the tag, or the next release from `main`.

A tag push runs `release.yml`: the `floor` job (the checks above), one build
per target, a smoke of every archive, then the GitHub release with the four
archives and their `.sha256` files. The one-line installer
(`install.sh`) resolves the latest release, so a new tag is live for users as
soon as the run finishes. Before you announce it: download the assets, verify
the checksums, run `act-lite --version` from an archive, and run
`install.sh` once.

## Plugin skills

`plugin/` is a Claude Code plugin that other marketplaces vendor. Every
directory under `plugin/skills/` is a skill and holds a `SKILL.md`. Its
frontmatter is one YAML mapping of single-line `key: value` pairs between
`---` lines, with string values only: `name` (equal to the directory name;
`a-z`, `0-9` and `-`), `description`, and optionally `license`,
`compatibility`, `metadata`, `allowed-tools`, `argument-hint`,
`disable-model-invocation`, `user-invocable`, `model`, `context`, `agent`,
`tools`, `color`, `skills`. Quote a value that contains `: ` or starts with a
quote, `[`, `{`, `#`, `&`, `*`, `!`, `|`, `>`, `%` or `@` (single quotes;
double an inner single quote). No tabs, flow mappings, anchors, aliases, merge
keys or tags. Skill names are unique across the marketplace. Check before a
pull request:

```sh
for f in plugin/skills/*/SKILL.md; do
  awk 'NR==1 && $0!="---"{exit 1} NR>1 && $0=="---"{exit 0} NR>1{print}' "$f" \
    | ruby -ryaml -e 'd=YAML.safe_load(STDIN.read); raise d.inspect unless d.is_a?(Hash) && d.key?("name") && d.key?("description")' \
    && echo "ok  $f" || echo "BAD $f"
done
for d in plugin/skills/*/; do [ -f "$d/SKILL.md" ] || echo "missing $d/SKILL.md"; done
```

`plugin/.mcp.json` stays: act-lite's own users need it; a marketplace that
serves the tools itself drops it when vendoring.

## License

Contributions are licensed under the MIT license of this repository
(`LICENSE`).
