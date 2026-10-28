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

# Best-effort completions, one file per shell, each dropped where that shell (or
# bash-completion) looks for a user's own. zsh's folder has to be on fpath, and we
# can't edit the user's shell config here, so we print the one line that adds it.
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
COMP_DIR="$DATA_DIR/zsh/site-functions"
if mkdir -p "$COMP_DIR" 2>/dev/null && "$BIN_DIR/grove" completions zsh > "$COMP_DIR/_grove" 2>/dev/null; then
    echo "zsh completions: ${COMP_DIR}/_grove"
    echo "  to enable, add before \`compinit\` in ~/.zshrc:"
    echo "      fpath=(${COMP_DIR} \$fpath)"
fi
BASH_COMP_DIR="${BASH_COMPLETION_USER_DIR:-$DATA_DIR/bash-completion}/completions"
if command -v bash >/dev/null 2>&1 && mkdir -p "$BASH_COMP_DIR" 2>/dev/null \
    && "$BIN_DIR/grove" completions bash > "$BASH_COMP_DIR/grove" 2>/dev/null; then
    echo "bash completions: ${BASH_COMP_DIR}/grove (loaded by bash-completion)"
fi
FISH_COMP_DIR="$DATA_DIR/fish/vendor_completions.d"
if command -v fish >/dev/null 2>&1 && mkdir -p "$FISH_COMP_DIR" 2>/dev/null \
    && "$BIN_DIR/grove" completions fish > "$FISH_COMP_DIR/grove.fish" 2>/dev/null; then
    echo "fish completions: ${FISH_COMP_DIR}/grove.fish"
fi

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: ${BIN_DIR} is not on your PATH — add it, e.g.:"
       echo "      export PATH=\"${BIN_DIR}:\$PATH\"" ;;
esac

echo "Done. Run \`${NAME}\` for an overview, or \`${NAME} setup\` to enable the short aliases (gs ga gc … and lg lgs lgp lgpp lt)."
