# SysVibe

<p align="center">
  <strong>A premium, zero-flicker system monitor TUI for Linux</strong>
</p>

<p align="center">
  Built with <code>ratatui</code> · <code>crossterm</code> · <code>sysinfo</code><br>
  Colored by <a href="https://github.com/catppuccin/catppuccin">Catppuccin Macchiato</a> · Graphed with <a href="https://en.wikipedia.org/wiki/Braille_Patterns">Braille</a>
</p>

---

## ✨ Highlights

| | Feature | Detail |
|---|---|---|
| 🎨 | **Catppuccin Macchiato palette** | Muted borders, neon data colors, full terminal transparency |
| 📊 | **Braille graphs** | Sparklines, line charts, and mirrored heartbeat graphs — all braille-rendered |
| ⚡ | **Low CPU overhead** | Native async event stream + virtual scrolling + tiered refresh (CPU/Memory fast, sensors slow) |
| 🖥️ | **Deep hardware integration** | Motherboard (DMI/SysFS), GPU (NVIDIA/AMD/Intel), RAM details (`lshw`/`dmidecode`) |
| 🔍 | **Process manager** | Sort, filter, multi-select, and kill processes with confirmation modal |
| 📋 | **Kernel log viewer** | Real-time logs via `journalctl`/`dmesg` with color-coded severity |
| 🖱️ | **Mouse support** | Click tabs, scroll process list |
| ⌨️ | **Full keyboard control** | Vim-style navigation, search, batch operations |

## 📸 Tabs

### System Tab
- OS, kernel, hostname, uptime, architecture
- Motherboard vendor/model, BIOS version/date
- CPU brand with core count
- RAM with DDR type, speed (MT/s), DIMM count & form factor
- GPU(s) with driver info
- Desktop environment & display server
- Load averages (1/5/15 min)
- Quick stat gauges (CPU / RAM / Swap / Battery)
- CPU history braille sparkline
- Disk partitions with usage bars
- Sensor temperatures with color-coded bars
- Battery status with power draw braille line graph

### Hardware Tab
- Per-core CPU usage with 4-char braille micro-sparklines
- Memory & swap gauges with usage bars
- Network I/O with compact speed summary + mirrored heartbeat graph (RX ▲ / TX ▼)
- Disk I/O with read/write speeds and IOPS

### Processes Tab
- Virtual scrolling for ultra-fast rendering of 1000+ processes
- Sortable process table (CPU / Memory / PID / Name)
- Real-time search/filter
- Multi-select with `Space`
- Safe kill with `[Y/N]` confirmation modal

### Logs Tab
- Kernel log viewer with `journalctl` / `dmesg`
- Color-coded severity levels
- Auto-scroll (follow mode) with `f`
- Manual refresh with `r`

## 🚀 Installation

### Easy Install (Recommended)

```bash
git clone https://github.com/harunkrl/SysVibe.git
cd SysVibe
./install.sh
```

This compiles the project and creates a desktop shortcut so you can launch SysVibe from your application menu.

### Uninstallation

```bash
./uninstall.sh
```

### Manual Install via Cargo

```bash
git clone https://github.com/harunkrl/SysVibe.git
cd SysVibe
cargo install --path .
```

> Make sure `~/.cargo/bin` is in your `$PATH`.

### Run without Installing

```bash
git clone https://github.com/harunkrl/SysVibe.git
cd SysVibe
cargo run --release
```

## ⌨️ Keybindings

| Key | Action |
|---|---|
| `q` / `Esc` | Quit |
| `h` / `?` | Toggle help panel |
| `Tab` / `Shift+Tab` | Cycle tabs |
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `PageUp` / `PageDown` | Scroll by page |
| `Home` / `End` | Jump to start / end |
| `s` | Cycle sort mode (CPU → Mem → PID → Name) |
| `/` | Search / filter processes |
| `Enter` | Apply active filter |
| `Space` | Toggle multi-select on process |
| `c` | Clear selection |
| `x` | Kill selected process(es) |
| `K` | Force kill (SIGKILL) selected process(es) |
| `r` | Manual refresh (processes or logs) |
| `t` | Toggle temperature units (°C / °F) |
| `f` | Toggle log auto-scroll (follow mode) |
| Mouse click | Switch tabs |
| Mouse scroll | Scroll process list |

## ⚙️ Configuration

Config file: `~/.config/sysvibe/config.toml` (XDG-compliant)

If the file doesn't exist, SysVibe runs with sensible defaults.

```toml
[tui]
# Maximum number of processes to display
max_processes = 200

# Refresh interval in milliseconds for CPU/memory/network/disk
data_refresh_rate = 250

# Refresh interval in milliseconds for sensors & battery
sensor_refresh_rate = 5000

# Kernel log source: "journalctl" or "dmesg"
log_source = "journalctl"

# Maximum log lines to keep
log_max_lines = 500

# Show GPU info in System tab
show_gpu = true

# Default tab on startup: "system", "hardware", "processes", "logs"
default_tab = "system"
```

## 🏗️ Architecture

```
src/
├── main.rs                    # Entry point, terminal setup, render loop
├── config.rs                  # XDG config loading
├── app/
│   ├── mod.rs                 # App state, tiered refresh logic
│   ├── state.rs               # Data structures (SystemInfo, NetworkStats, etc.)
│   ├── events.rs              # Keyboard/mouse event handling
│   ├── helpers.rs             # Shared utilities (push_history, etc.)
│   ├── processes.rs           # Process listing, sorting, kill
│   └── collectors/
│       ├── cpu.rs             # CPU history (overall + per-core)
│       ├── network.rs         # Network speed deltas & history
│       ├── disk.rs            # Disk I/O speeds, IOPS, partitions
│       ├── sensors.rs         # Temperatures & battery (sysinfo)
│       ├── hardware.rs        # Static hardware (DMI, lspci, lshw)
│       └── logs.rs            # Kernel log collection
└── ui/
    ├── mod.rs                 # Root layout, tab dispatch
    ├── palette.rs             # Catppuccin Macchiato constants
    ├── header.rs              # Tab bar
    ├── footer.rs              # Status bar with key hints
    ├── helpers.rs             # Shared UI helpers (panel_block, etc.)
    ├── tabs/
    │   ├── system.rs          # System info tab
    │   ├── hardware.rs        # Hardware monitoring tab
    │   ├── processes.rs       # Process manager tab
    │   └── logs.rs            # Kernel log viewer tab
    └── widgets/
        ├── sparkline.rs       # Braille sparkline & mirrored graph engine
        └── modal.rs           # Confirmation / help modal
```

## 🔧 Requirements

- **Linux** (tested on Fedora, Arch, Ubuntu)
- **Rust 1.85+** (edition 2024)
- **Optional tools** for deep hardware info:
  - `lspci` — GPU detection (usually pre-installed)
  - `lshw` or `dmidecode` — RAM speed/type details (may need root)
  - `journalctl` — kernel logs (systemd)

## 📄 License

[MIT](LICENSE)
