#!/bin/sh
set -e

# Pace Toolchain Installer for Linux and macOS

REPO="pace-lang/pace"
INSTALL_DIR="$HOME/.pace"
BIN_DIR="$INSTALL_DIR/bin"

echo "✨ Installing Pace Toolchain..."

# Detect OS
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$OS" in
    linux) OS_NAME="linux" ;;
    darwin) OS_NAME="macos" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Detect Architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64) ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Fetch latest release version
echo "-> Detecting latest version..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    echo "Error: Could not determine the latest release version."
    # Fallback to v0.1.0 if API fails (rate limits, etc)
    LATEST_RELEASE="v0.1.0"
    echo "Falling back to $LATEST_RELEASE"
fi

FILENAME="pace-${LATEST_RELEASE}-${OS_NAME}-${ARCH_NAME}.tar.gz"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/$FILENAME"

echo "-> Downloading Pace $LATEST_RELEASE for $OS_NAME-$ARCH_NAME..."

# Create temp dir for download
TMP_DIR=$(mktemp -d)
cd "$TMP_DIR"

if ! curl -L --progress-bar -o "$FILENAME" "$DOWNLOAD_URL"; then
    echo "Error: Failed to download $DOWNLOAD_URL"
    exit 1
fi

echo "-> Extracting toolchain..."
mkdir -p "$INSTALL_DIR"

# Extract stripping the top-level 'pace' directory from the tarball
tar -xzf "$FILENAME" -C "$INSTALL_DIR" --strip-components=1

# Clean up
rm -rf "$TMP_DIR"

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
    echo "-> Configuring PATH in $PROFILE_FILE..."
    echo "" >> "$PROFILE_FILE"
    echo "# Pace Toolchain" >> "$PROFILE_FILE"
    echo "export PATH=\"\$PATH:$BIN_DIR\"" >> "$PROFILE_FILE"
    
    echo ""
    echo "✅ Pace Toolchain installed successfully!"
    echo "Please restart your terminal or run:"
    echo "    source $PROFILE_FILE"
else
    echo ""
    echo "✅ Pace Toolchain installed successfully!"
    echo "PATH is already configured."
fi

echo ""
echo "Try running:"
echo "    pace --version"
echo "    pace new hello"
