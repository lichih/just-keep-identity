#!/bin/bash

# Just Keep Identity (jki) Installation Script
set -e

# Default values
INSTALL_DIR="${HOME}/.local/bin"
SILENT=false
UPDATE_PATH=true
CORE_ONLY=false
SKIP_BUILD=false

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --silent) SILENT=true; UPDATE_PATH=false ;;
        --no-path) UPDATE_PATH=false ;;
        --core-only) CORE_ONLY=true ;;
        --skip-build) SKIP_BUILD=true ;;
        --install-dir) INSTALL_DIR="$2"; shift ;;
        *) echo "Unknown parameter: $1"; exit 1 ;;
    esac
    shift
done

echo "=== Just Keep Identity (jki) Installation ==="

# Check requirements
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo is not installed. Please install it from https://rustup.rs"
    exit 1
fi

# Build
if [ "$SKIP_BUILD" = true ]; then
    echo "Skipping build as requested. Installing existing binaries from target/release..."
else
    echo "Building binaries..."
    if [ "$CORE_ONLY" = true ]; then
        cargo build --release -p jki -p jkim
    else
        cargo build --release --workspace
    fi
fi

if [ "$CORE_ONLY" = true ]; then
    BINS=("jki" "jkim")
else
    BINS=("jki" "jkim" "jki-agent")
fi
for bin in "${BINS[@]}"; do
    if [ -f "target/release/$bin" ]; then
        echo "Installing $bin to $INSTALL_DIR..."
        cp "target/release/$bin" "$INSTALL_DIR/"
    else
        echo "Warning: Binary $bin not found in target/release/"
    fi
done

echo "Installation complete!"

# PATH Check
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    if [ "$UPDATE_PATH" = true ]; then
        echo "Warning: $INSTALL_DIR is not in your PATH."
        
        SHELL_RC=""
        if [[ "$SHELL" == */zsh ]]; then
            SHELL_RC="$HOME/.zshrc"
        elif [[ "$SHELL" == */bash ]]; then
            SHELL_RC="$HOME/.bashrc"
        fi

        if [ -n "$SHELL_RC" ]; then
            if [ "$SILENT" = true ]; then
                 echo "Adding $INSTALL_DIR to $SHELL_RC (Silent Mode)"
                 printf "\n# Just Keep Identity (jki) PATH\nexport PATH=\"\$PATH:%s\"\n" "$INSTALL_DIR" >> "$SHELL_RC"
            else
                read -p "Would you like to add $INSTALL_DIR to your $SHELL_RC? [y/N] " response
                if [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
                    printf "\n# Just Keep Identity (jki) PATH\nexport PATH=\"\$PATH:%s\"\n" "$INSTALL_DIR" >> "$SHELL_RC"
                    echo "PATH updated. Please run 'source $SHELL_RC' to refresh your session."
                else
                    echo "Skipping PATH update. Please add $INSTALL_DIR to your PATH manually."
                fi
            fi
        else
            echo "Could not detect shell configuration file. Please add $INSTALL_DIR to your PATH manually."
        fi
    fi
else
    echo "$INSTALL_DIR is already in your PATH."
fi

echo "Done!"
