#!/bin/bash

set -e

echo "========================================="
echo "   Installing SysVibe System Monitor"
echo "========================================="

# 1. Check for Rust and Cargo
if ! command -v cargo &> /dev/null; then
    echo "Error: Cargo is not installed. Please install Rust via rustup (https://rustup.rs/)."
    exit 1
fi

# 2. Build and install via Cargo
echo "--> Compiling and installing SysVibe via Cargo..."
cargo install --path .

# 3. Create Desktop Entry for Linux App Menu
echo "--> Creating Application Menu shortcut..."

DESKTOP_DIR="$HOME/.local/share/applications"
mkdir -p "$DESKTOP_DIR"

cat > "$DESKTOP_DIR/sysvibe.desktop" << 'EOF'
[Desktop Entry]
Name=SysVibe
Comment=A visually striking system monitor TUI for Linux
Exec=sysvibe
Icon=utilities-system-monitor
Terminal=true
Type=Application
Categories=System;Monitor;ConsoleOnly;
Keywords=system;monitor;task;manager;cpu;ram;network;
EOF

# Update desktop database if the command exists
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$DESKTOP_DIR" || true
fi

echo "========================================="
echo " Installation Complete! "
echo "========================================="
echo "You can now run 'sysvibe' from any terminal."
echo "You can also find SysVibe in your application menu."
echo "Note: Make sure ~/.cargo/bin is in your PATH."
