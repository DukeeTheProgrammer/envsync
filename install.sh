#!/bin/bash
set -euo pipefail

REPO="dukeetheprogrammer/envsync"
BINARY="envsync"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}>>>${NC} $*"; }
ok()    { echo -e "${GREEN}>>>${NC} $*"; }
err()   { echo -e "${RED}error:${NC} $*" >&2; exit 1; }

# --- Try pre-built binary first ---
try_download() {
    local os arch url

    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux)  os="unknown-linux-gnu" ;;
        darwin) os="apple-darwin" ;;
        *)      return 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)             return 1 ;;
    esac

    # Try to get latest release URL from GitHub
    local tag
    tag=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name"' | head -1 | cut -d'"' -f4)

    if [ -z "$tag" ]; then
        return 1
    fi

    url="https://github.com/$REPO/releases/download/$tag/${BINARY}-${arch}-${os}.tar.gz"

    info "Downloading ${BINARY} ${tag} for ${arch}-${os}..."

    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf $tmpdir" EXIT

    if curl -sL "$url" -o "$tmpdir/$BINARY.tar.gz" 2>/dev/null; then
        tar -xzf "$tmpdir/$BINARY.tar.gz" -C "$tmpdir"
        install_binary "$tmpdir/$BINARY"
        return 0
    fi

    return 1
}

# --- Build from source ---
build_from_source() {
    info "No pre-built binary available. Building from source..."

    # Source cargo env if installed via rustup
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
    fi

    if ! command -v cargo &>/dev/null; then
        err "Rust/cargo not found. Install it: https://rustup.rs"
    fi

    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf $tmpdir" EXIT

    info "Cloning $REPO..."
    if ! git clone --depth 1 "https://github.com/$REPO.git" "$tmpdir" 2>/dev/null; then
        # If repo doesn't exist yet, build from local if we're in the project
        local script_dir
        script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        if [ -f "$script_dir/Cargo.toml" ] && grep -q 'name = "envsync"' "$script_dir/Cargo.toml" 2>/dev/null; then
            info "Building from local source..."
            cd "$script_dir"
            cargo build --release 2>&1
            install_binary "$script_dir/target/release/$BINARY"
            return 0
        fi
        err "Could not clone repo. Install Rust and run: cargo install $BINARY"
    fi

    info "Building release binary..."
    cd "$tmpdir"
    cargo build --release 2>&1
    install_binary "./target/release/$BINARY"
}

install_binary() {
    local src="$1"

    if [ ! -f "$src" ]; then
        err "Binary not found at $src"
    fi

    chmod +x "$src"

    # Ensure install directory exists
    mkdir -p "$INSTALL_DIR" 2>/dev/null || true

    # Try system dir first, fall back to ~/.local/bin
    if [ -w "$INSTALL_DIR" ] 2>/dev/null; then
        cp "$src" "$INSTALL_DIR/$BINARY"
    else
        INSTALL_DIR="$HOME/.local/bin"
        mkdir -p "$INSTALL_DIR"
        cp "$src" "$INSTALL_DIR/$BINARY"
    fi

    ok "Installed ${BOLD}$BINARY${NC} to $INSTALL_DIR/$BINARY"

    # Check if in PATH
    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        echo ""
        echo -e "${CYAN}Add to your PATH:${NC}"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
        echo "Or add it to your shell config (~/.bashrc, ~/.zshrc, etc.)"
    fi

    echo ""
    ok "Run 'envsync --help' to get started"
}

# --- Main ---
main() {
    echo ""
    echo -e "${BOLD}envsync installer${NC}"
    echo ""

    if [ -f "$INSTALL_DIR/$BINARY" ]; then
        ok "envsync is already installed at $INSTALL_DIR/$BINARY"
        info "Run 'envsync --help' or 'envsync --version'"
        exit 0
    fi

    if try_download; then
        exit 0
    fi

    build_from_source
}

main "$@"
