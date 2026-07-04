#!/bin/bash
#
# Vitalis — installer
#
# Builds via `cargo install`, generates a default config if none exists, and
# (on Linux desktops) creates an application-menu shortcut. Termux/Android is
# detected automatically (no desktop entry; suggests nerd_fonts = false).
#
set -euo pipefail

echo "========================================="
echo "   Installing Vitalis System Monitor"
echo "========================================="

# 0. Detect Termux / Android
IS_TERMUX=0
if [ -n "${PREFIX:-}" ] && echo "$PREFIX" | grep -qi "com.termux"; then
	IS_TERMUX=1
fi

# 1. Rust toolchain
if ! command -v cargo >/dev/null 2>&1; then
	echo "Error: cargo not found."
	echo "       Install Rust via https://rustup.rs"
	echo "       Termux: pkg install rust"
	exit 1
fi

# 2. Build & install
echo "--> Compiling and installing Vitalis via Cargo..."
cargo install --path .
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
BIN="$CARGO_BIN/vitalis"

# 3. Default config (if missing)
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/vitalis"
CONFIG_FILE="$CONFIG_DIR/config.toml"
if [ ! -f "$CONFIG_FILE" ] && [ -x "$BIN" ]; then
	echo "--> Generating default config at $CONFIG_FILE ..."
	"$BIN" --init-config >/dev/null 2>&1 ||
		echo "    (run 'vitalis --init-config' later to create it)"
fi

# 4. Desktop entry (Linux desktop only — skip on Termux/Android)
if [ "$IS_TERMUX" -eq 0 ] && [ -n "${HOME:-}" ]; then
	echo "--> Creating application menu shortcut..."
	DESKTOP_DIR="$HOME/.local/share/applications"
	mkdir -p "$DESKTOP_DIR"
	cat >"$DESKTOP_DIR/vitalis.desktop" <<'EOF'
[Desktop Entry]
Name=Vitalis
Comment=A modern system monitor TUI for Linux
Exec=vitalis
Icon=utilities-system-monitor
Terminal=true
Type=Application
Categories=System;Monitor;ConsoleOnly;
Keywords=system;monitor;task;manager;cpu;ram;network;
EOF
	if command -v update-desktop-database >/dev/null 2>&1; then
		update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
	fi
fi

echo "========================================="
echo " Installation Complete!"
echo "========================================="
echo "Run    : vitalis"
echo "Themes : vitalis --list-themes"
echo "Config : $CONFIG_FILE"
if [ "$IS_TERMUX" -eq 1 ]; then
	echo "Termux : set 'nerd_fonts = false' in the config for clean fallback icons."
fi
echo "Make sure ${CARGO_BIN} is on your PATH."
