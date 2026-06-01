# SysVibe

SysVibe is a visually striking, zero-flicker system monitor Terminal UI (TUI) for Linux, built with Rust and Ratatui. It provides real-time monitoring of CPU, Memory, Network I/O, Disk I/O, Sensors, and Active Processes in a clean, aesthetic interface.

![SysVibe Screenshot](https://via.placeholder.com/800x500?text=SysVibe+TUI+Screenshot)

## Features

- **Beautiful TUI:** Uses the Catppuccin Macchiato color palette and Braille sparkline graphs.
- **CPU & Memory Monitoring:** Overall and per-core CPU usage, RAM, and Swap tracking.
- **Network & Disk I/O:** Real-time transfer rates and history graphs.
- **Process Manager:** View top processes, filter by name, and securely kill tasks.
- **Dynamic Sorting:** Sort processes dynamically by CPU, Memory, PID, or Name.
- **Hardware Sensors:** GPU/CPU temperatures and Battery status.
- **Zero Flicker:** Optimized rendering with Crossterm.

## Requirements

- **Linux OS** (Tested on Arch-based distributions)
- **Rust** (Install via [rustup.rs](https://rustup.rs/))

## Installation

### Method 1: Easy Install Script (Recommended)

Clone the repository and run the provided installation script. This will compile the project and automatically create a desktop shortcut so you can launch it from your application menu!

```bash
git clone https://github.com/harunkrl/SysVibe.git
cd SysVibe
./install.sh
```

### Method 2: Manual Installation via Cargo

If you prefer installing it manually, you can use `cargo install`:

```bash
git clone https://github.com/harunkrl/SysVibe.git
cd SysVibe
cargo install --path .
```
*Note: Make sure `~/.cargo/bin` is in your `PATH`.*

## Usage

You can launch SysVibe simply by running:
```bash
sysvibe
```
Or by searching for **"SysVibe"** in your Desktop Environment's application launcher.

### Keybindings

| Key | Action |
| --- | --- |
| `q` / `Esc` | Quit SysVibe |
| `h` / `?` | Toggle the Help panel |
| `Tab` / `Shift+Tab` | Cycle through tabs (System, Hardware, Processes, Logs) |
| `↑` / `k` | Move process selection up |
| `↓` / `j` | Move process selection down |
| `s` | Cycle process sorting mode (CPU > Mem > PID > Name) |
| `/` | Search / Filter processes by name |
| `Enter` | Apply the active filter |
| `Space` | Select multiple processes for batch operations |
| `c` | Clear process selection |
| `x` | Kill the selected process(es) (opens a safe `[Y/N]` confirmation modal) |
| `r` | Manually refresh processes or logs (depending on active tab) |
| `t` | Toggle temperature units (Celsius / Fahrenheit) |
| `f` | Toggle auto-scrolling for kernel logs (Follow mode) |

## Configuration
SysVibe supports a configuration file located at `~/.config/sysvibe/config.toml` (XDG Base Directory standard). 

If the file does not exist, SysVibe will run with sensible defaults. You can use this file to tweak layout options like showing/hiding disk I/O, setting maximum processes to list, and customizing refresh rates.

## License
MIT License
