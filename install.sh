#!/bin/sh
# grove installer — no Homebrew, no compiler, no root. Downloads the prebuilt
# static binaries for your OS/arch and drops them in a bin dir on your PATH.
#
#   curl -fsSL https://raw.githubusercontent.com/jakobhviid/grove/main/install.sh | sh
#
# Override the target dir with:  GROVE_BIN_DIR=/usr/local/bin  (may need sudo)
set -eu

REPO="jakobhviid/grove"
NAME="grove"
BIN_DIR="${GROVE_BIN_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
    Linux)  target_os="unknown-linux-musl" ;;
    Darwin) target_os="apple-darwin" ;;
    *) echo "unsupported OS: $os" >&2; exit 1 ;;
esac

case "$arch" in
    x86_64 | amd64)   target_arch="x86_64" ;;
    aarch64 | arm64)  target_arch="aarch64" ;;
    *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

asset="${NAME}-${target_arch}-${target_os}.tar.gz"
url="https://github.com/${REPO}/releases/latest/download/${asset}"

echo "Installing ${NAME} (${target_arch}-${target_os}) → ${BIN_DIR}"
mkdir -p "$BIN_DIR"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if ! curl -fsSL "$url" | tar xz -C "$tmp"; then
    echo "download/extract failed: $url" >&2
    exit 1
fi

installed=""
for f in "$tmp"/*; do
    [ -f "$f" ] || continue
    name="$(basename "$f")"
    install -m 0755 "$f" "$BIN_DIR/$name"
    installed="${installed} ${name}"
done

echo "Installed:${installed}"

# Best-effort zsh completions: grove emits a single _grove file (covering grove
# and lg/lgs/lgp/lgpp/lt) that we drop in a data dir. We can't edit the user's shell
# config here, so we print the one line that puts it on fpath.
COMP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
if mkdir -p "$COMP_DIR" 2>/dev/null && "$BIN_DIR/grove" completions zsh > "$COMP_DIR/_grove" 2>/dev/null; then
    echo "zsh completions: ${COMP_DIR}/_grove"
    echo "  to enable, add before \`compinit\` in ~/.zshrc:"
    echo "      fpath=(${COMP_DIR} \$fpath)"
fi

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: ${BIN_DIR} is not on your PATH — add it, e.g.:"
       echo "      export PATH=\"${BIN_DIR}:\$PATH\"" ;;
esac

echo "Done. Run \`${NAME}\` for an overview, or \`${NAME} setup\` to enable the short aliases (gs ga gc … and lg lgs lgp lgpp lt)."
