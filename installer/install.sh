#!/bin/sh
set -e

# Pace Toolchain Installer for Linux and macOS

REPO="pace-lang/pace"
INSTALL_DIR="$HOME/.pace"
BIN_DIR="$INSTALL_DIR/bin"

# Colors
GREEN=$(printf '\033[0;32m')
CYAN=$(printf '\033[0;36m')
YELLOW=$(printf '\033[0;33m')
RED=$(printf '\033[0;31m')
BOLD=$(printf '\033[1m')
NC=$(printf '\033[0m') # No Color

# Logging functions
log_info() { echo "-> $1"; }
log_success() { echo "✅ ${GREEN}${BOLD}$1${NC}"; }
log_warn() { echo "⚠️  ${YELLOW}$1${NC}"; }
log_error() { echo "❌ ${RED}${BOLD}Error: $1${NC}" >&2; }

# Parse arguments
INTERACTIVE=0
UNINSTALL=0

for arg in "$@"; do
    case "$arg" in
        --interactive|-i) INTERACTIVE=1 ;;
        --uninstall|-u) UNINSTALL=1 ;;
        --help|-h)
            echo "Pace Toolchain Installer"
            echo "Usage: ./install.sh [OPTIONS]"
            echo "Options:"
            echo "  -i, --interactive   Prompt for confirmation before changes"
            echo "  -u, --uninstall     Uninstall the Pace Toolchain"
            echo "  -h, --help          Show this help message"
            exit 0
            ;;
        *)
            log_warn "Unknown argument: $arg"
            ;;
    esac
done

if [ "$UNINSTALL" -eq 1 ]; then
    log_info "Uninstalling Pace Toolchain..."
    if [ "$INTERACTIVE" -eq 1 ]; then
        printf "Are you sure you want to remove %s? [y/N] " "$INSTALL_DIR"
        read -r CONFIRM
        if [ "$CONFIRM" != "y" ] && [ "$CONFIRM" != "Y" ]; then
            log_info "Uninstall cancelled."
            exit 0
        fi
    fi
    
    if [ -d "$INSTALL_DIR" ]; then
        rm -rf "$INSTALL_DIR"
        log_success "Pace Toolchain removed from $INSTALL_DIR"
    else
        log_info "Pace Toolchain is not installed at $INSTALL_DIR"
    fi
    log_warn "Note: You may need to manually remove the PATH entry from your shell profile."
    exit 0
fi

echo "${CYAN}${BOLD}✨ Installing Pace Toolchain...${NC}"

# Detect OS
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$OS" in
    linux) OS_NAME="linux" ;;
    darwin) OS_NAME="macos" ;;
    *) log_error "Unsupported OS: $OS"; exit 1 ;;
esac

# Detect Architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64) ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
    *) log_error "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Cleanup trap
TMP_DIR=""
cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT INT TERM

# Fetch latest release version
log_info "Detecting latest version..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    log_warn "Could not determine the latest release version."
    # Fallback to v0.1.0 if API fails (rate limits, etc)
    LATEST_RELEASE="v0.1.0"
    log_info "Falling back to $LATEST_RELEASE"
fi

if [ -x "$BIN_DIR/pace" ]; then
    # Extract just the version number to ignore ANSI color codes and extraneous text
    CURRENT_VERSION=$("$BIN_DIR/pace" --version 2>/dev/null | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1)
    if [ "v$CURRENT_VERSION" = "$LATEST_RELEASE" ] || [ "$CURRENT_VERSION" = "$LATEST_RELEASE" ]; then
        echo ""
        log_success "Congrats! You're already on the latest version of Pace (which is ${LATEST_RELEASE})"
        exit 0
    fi
fi

if [ "$INTERACTIVE" -eq 1 ]; then
    printf "Install Pace %s to %s? [Y/n] " "$LATEST_RELEASE" "$INSTALL_DIR"
    read -r CONFIRM
    if [ "$CONFIRM" = "n" ] || [ "$CONFIRM" = "N" ]; then
        log_info "Installation cancelled."
        exit 0
    fi
fi

FILENAME="pace-${LATEST_RELEASE}-${OS_NAME}-${ARCH_NAME}.tar.gz"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/$FILENAME"

log_info "Downloading Pace $LATEST_RELEASE for $OS_NAME-$ARCH_NAME..."

# Create temp dir for download
TMP_DIR=$(mktemp -d)
(
    cd "$TMP_DIR"
    if ! curl -L --progress-bar -o "$FILENAME" "$DOWNLOAD_URL"; then
        log_error "Failed to download $DOWNLOAD_URL"
        exit 1
    fi
)

log_info "Extracting toolchain..."
mkdir -p "$INSTALL_DIR"

# Extract stripping the top-level 'pace' directory from the tarball
tar -xzf "$TMP_DIR/$FILENAME" -C "$INSTALL_DIR" --strip-components=1

# Add to PATH
SHELL_NAME=$(basename "$SHELL")
PROFILE_FILE=""

case "$SHELL_NAME" in
    bash) PROFILE_FILE="$HOME/.bashrc" ;;
    zsh) PROFILE_FILE="$HOME/.zshrc" ;;
    *) PROFILE_FILE="$HOME/.profile" ;;
esac

# Ensure bin directory exists and has correct permissions
chmod +x "$BIN_DIR/pace"

if ! grep -q "$BIN_DIR" "$PROFILE_FILE" 2>/dev/null; then
    log_info "Configuring PATH..."
    UPDATE_PATH=1
    
    if [ "$INTERACTIVE" -eq 1 ]; then
        printf "Do you want to automatically update your PATH in %s? [Y/n] " "$PROFILE_FILE"
        read -r CONFIRM
        if [ "$CONFIRM" = "n" ] || [ "$CONFIRM" = "N" ]; then
            UPDATE_PATH=0
        fi
    fi
    
    if [ "$UPDATE_PATH" -eq 1 ]; then
        echo "" >> "$PROFILE_FILE"
        echo "# Pace Toolchain" >> "$PROFILE_FILE"
        echo "export PATH=\"\$PATH:$BIN_DIR\"" >> "$PROFILE_FILE"
        echo ""
        log_success "Pace Toolchain installed successfully!"
        echo "Please restart your terminal or run:"
        echo "    source $PROFILE_FILE"
    else
        echo ""
        log_success "Pace Toolchain installed successfully!"
        log_warn "You chose not to update PATH automatically."
        echo "Please add the following line to your shell profile:"
        echo "    export PATH=\"\$PATH:$BIN_DIR\""
    fi
else
    echo ""
    log_success "Pace Toolchain installed successfully!"
    echo "PATH is already configured."
fi

# Check for C compiler
if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1 && ! command -v clang >/dev/null 2>&1; then
    echo ""
    log_warn "A C compiler (cc, gcc, or clang) was not found in your PATH."
    echo "Pace requires a C compiler to link executables."
    echo "Please install build-essential (Linux) or Xcode Command Line Tools (macOS) before running Pace projects."
fi

echo ""
echo "Try running:"
echo "    pace --version"
echo "    pace create hello"
