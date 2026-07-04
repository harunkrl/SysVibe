#!/bin/bash
#
# Vitalis — uninstaller
#
# Removes the cargo-installed binary, the desktop entry, and (optionally, with
# confirmation) the user config directory.
#
set -uo pipefail # no -e: keep going even when some steps are no-ops

echo "========================================="
echo "   Uninstalling Vitalis System Monitor"
echo "========================================="

# 1. Binary
if command -v cargo >/dev/null 2>&1; then
	echo "--> Uninstalling via Cargo..."
	cargo uninstall vitalis || echo "    (Not installed via cargo or already removed)"
else
	echo "--> Cargo not found, skipping cargo uninstall."
fi

# 2. Desktop entry
DESKTOP_DIR="$HOME/.local/share/applications"
if [ -f "$DESKTOP_DIR/vitalis.desktop" ]; then
	echo "--> Removing application menu shortcut..."
	rm -f "$DESKTOP_DIR/vitalis.desktop"
	if command -v update-desktop-database >/dev/null 2>&1; then
		update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
	fi
else
	echo "--> No desktop shortcut found, skipping."
fi

# 3. Config (optional, prompted)
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/vitalis"
if [ -d "$CONFIG_DIR" ]; then
	printf "Remove config directory '%s'? [y/N] " "$CONFIG_DIR"
	read -r ans
	case "$ans" in
	y | Y | yes | YES)
		rm -rf "$CONFIG_DIR"
		echo "    Removed $CONFIG_DIR"
		;;
	*)
		echo "    Kept $CONFIG_DIR"
		;;
	esac
fi

echo "========================================="
echo " Uninstallation Complete!"
echo "========================================="
