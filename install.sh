#!/bin/sh
# Install act-lite from the GitHub releases of act101-ai/act-lite.
#
#   curl -fsSL https://raw.githubusercontent.com/act101-ai/act-lite/main/install.sh | sh
#
# Environment:
#   ACT_LITE_VERSION      release tag to install, for example v0.1.0 (default: the latest release)
#   ACT_LITE_INSTALL_DIR  install directory (default: ~/.local/bin)
#
# Test seams (not for normal use):
#   ACT_LITE_BASE_URL     base of the download URLs; the archive and its .sha256 are fetched from
#                         <base>/<tag>/<file> (default: https://github.com/act101-ai/act-lite/releases/download)
#   ACT_LITE_UNAME_S      replaces the output of `uname -s`
#   ACT_LITE_UNAME_M      replaces the output of `uname -m`
#
# Supported: Linux x86_64 and aarch64 with glibc 2.31 or later, macOS arm64 and x86_64.
# The script never uses sudo. It writes only to the install directory and to a
# temporary directory that it removes on exit.

set -eu

REPO=act101-ai/act-lite

die() {
  printf 'install.sh: %s\n' "$*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required but was not found"
}

detect_target() {
  os=${ACT_LITE_UNAME_S:-$(uname -s)}
  arch=${ACT_LITE_UNAME_M:-$(uname -m)}
  case "$os/$arch" in
    Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
    Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-gnu ;;
    Darwin/x86_64) target=x86_64-apple-darwin ;;
    Darwin/arm64 | Darwin/aarch64) target=aarch64-apple-darwin ;;
    *) die "unsupported platform: $os $arch (supported: Linux and macOS on x86_64 and arm64)" ;;
  esac
  if [ "$os" = Linux ]; then
    check_glibc
  fi
}

check_glibc() {
  if command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
    die "unsupported platform: this Linux uses musl libc; act-lite needs glibc 2.31 or later"
  fi
  glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null || true)
  case "$glibc" in
    glibc\ *) ;;
    *) die "unsupported platform: glibc was not detected; act-lite needs glibc 2.31 or later" ;;
  esac
  glibc=${glibc#glibc }
  major=${glibc%%.*}
  minor=${glibc#*.}
  minor=${minor%%[!0-9]*}
  if [ "$major" -lt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -lt 31 ]; }; then
    die "unsupported platform: glibc $glibc found; act-lite needs glibc 2.31 or later"
  fi
}

latest_tag() {
  tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null |
    sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1) || tag=
  if [ -z "$tag" ]; then
    final=$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" 2>/dev/null) || final=
    tag=${final##*/}
  fi
  [ -n "$tag" ] || die "could not determine the latest release; set ACT_LITE_VERSION=vX.Y.Z"
  printf '%s\n' "$tag"
}

verify() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$1"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  else
    die "shasum or sha256sum is required but was not found"
  fi
}

main() {
  need curl
  need tar
  detect_target

  tag=${ACT_LITE_VERSION:-}
  if [ -z "$tag" ]; then
    tag=$(latest_tag)
  fi
  case "$tag" in
    v[0-9]*) ;;
    *) die "invalid release tag: $tag (expected a tag such as v0.1.0)" ;;
  esac

  base=${ACT_LITE_BASE_URL:-https://github.com/$REPO/releases/download}
  dir=${ACT_LITE_INSTALL_DIR:-$HOME/.local/bin}
  name=act-lite-$tag-$target

  tmp=$(mktemp -d) || die "could not create a temporary directory"
  staged=
  trap 'rm -rf "$tmp"; [ -z "$staged" ] || rm -f "$staged"' EXIT
  trap 'exit 1' HUP INT TERM

  printf 'Installing act-lite %s for %s\n' "$tag" "$target"
  curl -fsSL -o "$tmp/$name.tar.gz" "$base/$tag/$name.tar.gz" ||
    die "download failed: $base/$tag/$name.tar.gz"
  curl -fsSL -o "$tmp/$name.tar.gz.sha256" "$base/$tag/$name.tar.gz.sha256" ||
    die "download failed: $base/$tag/$name.tar.gz.sha256"

  printf 'Verifying checksum\n'
  (cd "$tmp" && verify "$name.tar.gz.sha256") ||
    die "checksum mismatch for $name.tar.gz; nothing was installed"

  tar -xzf "$tmp/$name.tar.gz" -C "$tmp" || die "could not extract $name.tar.gz"
  [ -f "$tmp/$name/act-lite" ] || die "$name.tar.gz does not contain $name/act-lite"

  mkdir -p "$dir" || die "could not create $dir"
  [ -d "$dir" ] && [ -w "$dir" ] || die "$dir is not a writable directory"
  [ ! -d "$dir/act-lite" ] || die "$dir/act-lite is a directory; remove it or choose another ACT_LITE_INSTALL_DIR"

  staged=$dir/.act-lite.$$.tmp
  install -m 0755 "$tmp/$name/act-lite" "$staged" || die "could not write into $dir"
  "$staged" --version >/dev/null 2>&1 || die "the downloaded act-lite does not run; the existing install was not changed"
  mv -f "$staged" "$dir/act-lite" || die "could not replace $dir/act-lite"
  staged=

  printf 'Installed %s\n' "$dir/act-lite"
  "$dir/act-lite" --version

  case ":$PATH:" in
    *":$dir:"*) ;;
    *) printf '%s is not on your PATH; add it to PATH in your shell profile\n' "$dir" ;;
  esac
}

main "$@"
