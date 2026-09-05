#!/bin/sh
# Install cairn from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/oddurs/cairn/main/install.sh | sh
#
# Honours: CAIRN_VERSION (default: latest), CAIRN_BINDIR (default: the first
# writable of ~/.local/bin, /usr/local/bin).
set -eu

REPO="${CAIRN_REPO:-oddurs/cairn}"
VERSION="${CAIRN_VERSION:-latest}"

die() { printf 'install: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Darwin) case "$arch" in
              arm64|aarch64) echo aarch64-apple-darwin ;;
              x86_64)        echo x86_64-apple-darwin ;;
              *) die "unsupported macOS architecture: $arch" ;;
            esac ;;
    Linux)  case "$arch" in
              x86_64|amd64)  echo x86_64-unknown-linux-musl ;;
              aarch64|arm64) echo aarch64-unknown-linux-musl ;;
              *) die "unsupported Linux architecture: $arch" ;;
            esac ;;
    *) die "unsupported system: $os (build from source with \`cargo install cairn-md\`)" ;;
  esac
}

bindir() {
  if [ -n "${CAIRN_BINDIR:-}" ]; then echo "$CAIRN_BINDIR"; return; fi
  for d in "$HOME/.local/bin" /usr/local/bin; do
    if [ -d "$d" ] && [ -w "$d" ]; then echo "$d"; return; fi
  done
  echo "$HOME/.local/bin"
}

have curl || have wget || die "needs curl or wget"
fetch() {
  if have curl; then curl -fsSL "$1" -o "$2"; else wget -qO "$2" "$1"; fi
}

TARGET=$(target)
if [ "$VERSION" = latest ]; then
  BASE="https://github.com/$REPO/releases/latest/download"
else
  BASE="https://github.com/$REPO/releases/download/v${VERSION#v}"
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

printf 'Looking for cairn (%s, %s)...\n' "$VERSION" "$TARGET"
# The archive name carries the version, which "latest" does not know; the
# checksum file names every artefact, so it is also the index.
fetch "$BASE/SHA256SUMS" "$TMP/SHA256SUMS" || die "no release found at $BASE"
ARCHIVE=$(awk -v t="$TARGET" '$2 ~ t {print $2}' "$TMP/SHA256SUMS" | head -1)
[ -n "$ARCHIVE" ] || die "release has no binary for $TARGET"

printf 'Downloading %s...\n' "$ARCHIVE"
fetch "$BASE/$ARCHIVE" "$TMP/$ARCHIVE" || die "could not download $ARCHIVE"

want=$(awk -v a="$ARCHIVE" '$2 == a {print $1}' "$TMP/SHA256SUMS")
if have shasum; then got=$(shasum -a 256 "$TMP/$ARCHIVE" | cut -d' ' -f1)
elif have sha256sum; then got=$(sha256sum "$TMP/$ARCHIVE" | cut -d' ' -f1)
else got=""; printf 'warning: no sha256 tool; skipping checksum verification\n' >&2
fi
if [ -n "$got" ] && [ "$got" != "$want" ]; then
  die "checksum mismatch for $ARCHIVE (expected $want, got $got)"
fi

tar xzf "$TMP/$ARCHIVE" -C "$TMP"
BIN=$(find "$TMP" -type f -name cairn -perm -u+x | head -1)
[ -n "$BIN" ] || die "archive did not contain a cairn binary"

DIR=$(bindir)
mkdir -p "$DIR"
install -m 755 "$BIN" "$DIR/cairn" 2>/dev/null || { cp "$BIN" "$DIR/cairn"; chmod 755 "$DIR/cairn"; }

printf '\ncairn installed to %s\n' "$DIR/cairn"
case ":$PATH:" in
  *":$DIR:"*) ;;
  *) printf 'Add it to your PATH:\n  export PATH="%s:$PATH"\n' "$DIR" ;;
esac
printf 'Get started:\n  cairn init\n  cairn --help\n'
