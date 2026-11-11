#!/usr/bin/env bash
# Check the published act-lite release assets and run this host's archived binary.
set -euo pipefail

if [ "$#" -ne 1 ] || [[ ! "$1" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 vX.Y.Z" >&2
  exit 2
fi
tag=$1
version=${tag#v}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

gh release download "$tag" --repo act101-ai/act-lite --dir "$tmp"

targets=(
  x86_64-unknown-linux-gnu
  aarch64-unknown-linux-gnu
  aarch64-apple-darwin
  x86_64-apple-darwin
  x86_64-pc-windows-msvc
)
for target in "${targets[@]}"; do
  name="act-lite-${tag}-${target}"
  if [ "$target" = x86_64-pc-windows-msvc ]; then ext=zip; else ext=tar.gz; fi
  archive="$name.$ext"
  checksum="$archive.sha256"
  for asset in "$archive" "$checksum"; do
    if [ ! -f "$tmp/$asset" ]; then
      echo "missing release asset: $asset" >&2
      exit 1
    fi
  done
  read -r recorded recorded_name < "$tmp/$checksum"
  if command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$archive")
  else
    actual=$(sha256sum "$tmp/$archive")
  fi
  actual=${actual%% *}
  if [ "$recorded_name" != "$archive" ] || [ "$recorded" != "$actual" ]; then
    echo "checksum mismatch: $archive" >&2
    exit 1
  fi
done

case "$(uname -s):$(uname -m)" in
  Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin:x86_64) target=x86_64-apple-darwin ;;
  Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
  MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64) target=x86_64-pc-windows-msvc ;;
  *) echo "no release archive for this host" >&2; exit 1 ;;
esac
name="act-lite-${tag}-${target}"
if [ "$target" = x86_64-pc-windows-msvc ]; then
  unzip -q "$tmp/$name.zip" -d "$tmp"
  binary="$tmp/$name/act-lite.exe"
else
  tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
  binary="$tmp/$name/act-lite"
fi
if [ ! -f "$binary" ]; then
  echo "missing binary in $name archive" >&2
  exit 1
fi
actual_version=$("$binary" --version)
if [ "$actual_version" != "act-lite $version" ]; then
  echo "archived binary reports '$actual_version', expected 'act-lite $version'" >&2
  exit 1
fi
printf '%s\n' "$actual_version"
