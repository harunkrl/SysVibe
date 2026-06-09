#!/bin/bash

set -e

echo "========================================="
echo "   Uninstalling SysVibe System Monitor"
echo "========================================="

# 1. Uninstall via Cargo
if command -v cargo &> /dev/null; then
    echo "--> Uninstalling SysVibe via Cargo..."
    cargo uninstall sysvibe || echo "    (Not installed via cargo or already removed)"
else
    echo "    Cargo not found, skipping cargo uninstall."
fi

# 2. Remove Desktop Entry
echo "--> Removing Application Menu shortcut..."

DESKTOP_DIR="$HOME/.local/share/applications"
if [ -f "$DESKTOP_DIR/sysvibe.desktop" ]; then
    rm "$DESKTOP_DIR/sysvibe.desktop"
    echo "    Removed $DESKTOP_DIR/sysvibe.desktop"
else
    echo "    Shortcut not found, skipping."
fi

# Update desktop database if the command exists
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$DESKTOP_DIR" || true
fi

echo "========================================="
echo " Uninstallation Complete! "
echo "========================================="
