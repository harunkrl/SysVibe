# SysVibe

<p align="center">
  <strong>A modern, zero-flicker system monitor TUI for Linux &amp; Android (Termux)</strong>
</p>

<p align="center">
  Built with <code>ratatui</code> · <code>crossterm</code> · <code>sysinfo</code> · <code>tokio</code><br>
  Colored by <a href="https://github.com/catppuccin/catppuccin">Catppuccin</a> (+ 6 more themes) · Graphed with braille &amp; half-block
</p>

---

## ✨ Highlights

| | Feature | Detail |
|---|---|---|
| 🎛️ | **6 live tabs** | Dashboard · System · Hardware · Processes · Logs · GPU |
| 📊 | **Hero stat cards** | At-a-glance CPU/RAM/GPU/Net/Temp/Battery with mini sparklines |
| 🎨 | **Live theming** | 7 built-in themes + custom TOML themes, switchable at runtime (`T`) |
| ⌨️ | **Command palette** | Fuzzy action menu (`:`) — jump tabs, switch theme, export, quit, … |
| 🔔 | **Alerts & toasts** | Configurable CPU/Mem/Temp/Disk thresholds with a prominent toast banner |
| 🪟 | **Compact mode** | Narrow terminals (Android/Termux portrait) auto-stack to a single column |
| ⚡ | **Low CPU overhead** | Async event loop + background `std::thread` collectors + tiered refresh |
| 🐧 | **Deep hardware** | CPU clusters, RAM/battery breakdown, temps, disk I/O, NVIDIA/AMD/Intel GPU |
| 📜 | **Log viewer** | `journalctl`/`dmesg` on Linux, `logcat` on Android, with severity filters |
| 🖱️ | **Mouse + keyboard** | Click tabs, scroll lists, *and* full vim-style keyboard control |

---

## 📦 Installation

### Option A — install script (recommended)

```bash
git clone https://github.com/<you>/SysVibe.git
cd SysVibe
./install.sh
```

The script builds via `cargo install --path .`, creates an application-menu shortcut
on Linux desktops, and generates a default config if one doesn't exist.

### Option B — manual

```bash
cargo install --path .
sysvibe --init-config      # write ~/.config/sysvibe/config.toml (optional)
```

### Android / Termux

SysVibe builds and runs under Termux. The UI auto-switches to a compact stacked
layout on narrow terminals and labels the Logs tab **Logcat**.

```bash
pkg install rust           # or: pkg install clang && cargo ...
cargo install --path .
sysvibe
```

> `nerd_fonts` defaults to `true`; on Termux (no Nerd Font) set `nerd_fonts = false`
> to use the clean geometric fallback icon set.

---

## 🚀 Usage

```bash
sysvibe                          # run with default settings
sysvibe --init-config            # create/edit the config file, then exit
sysvibe --list-themes            # list available themes, then exit
```

The first run writes a default config to `~/.config/sysvibe/config.toml` (XDG).
Edit it to change theme, refresh rates, alert thresholds, and more.

---

## 🗂️ Tabs

- **1 · Dashboard** — hero stat cards + CPU history graph + memory bars + system/network overview + top processes.
- **2 · System** — OS/kernel/host/uptime, motherboard/BIOS, RAM DIMMs, GPU, disks, desktop session.
- **3 · Hardware** — per-core CPU clusters, memory & battery breakdown (power/health/cycles), network RX↑/TX↓ graph, temperature sensors, disk I/O graphs.
- **4 · Processes** — sort/filter, multi-select, tree view, and kill (with confirmation modal).
- **5 · Logs** — real-time `journalctl`/`dmesg` (or `logcat` on Android), level filters, follow mode.
- **6 · GPU** — usage, VRAM, temperature, power, fan, clock per GPU (NVIDIA/AMD/Intel).

> Tabs shrink to a single stacked column when the terminal is narrow (`< 90` cols).

---

## ⌨️ Keybindings

### Global

| Key | Action |
|---|---|
| `1`–`6` | Jump to tab |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `[` / `]` | Cycle panel focus |
| `:` | **Command palette** |
| `T` | Cycle theme |
| `t` | Toggle °C / °F |
| `h` / `?` | Help modal |
| `/` | Filter |
| `q` / `Esc` | Quit |

### Navigation

| Key | Action |
|---|---|
| `j` / `↓` · `k` / `↑` | Move down / up |
| `PageDown` / `PageUp` | Page down / up |
| `Home` / `End` | Top / bottom |

### Processes tab

| Key | Action |
|---|---|
| `s` | Cycle sort (CPU → Mem → PID → Name) |
| `r` | Refresh |
| `Space` | Toggle select |
| `x` | Kill selected (confirm) |
| `p` / `F5` | Toggle tree view |
| `c` | Clear selection |
| `E` | Export snapshot (JSON) |

### Logs tab

| Key | Action |
|---|---|
| `f` | Toggle follow |
| `e` / `w` / `i` | Toggle Error / Warning / Info level filter |
| `r` | Refresh |

### Command palette (`:`)

Type to fuzzy-match; `↑`/`↓` navigate, `Enter` run, `Esc` cancel, `Ctrl+U` clear.

---

## ⚙️ Configuration

Config lives at `~/.config/sysvibe/config.toml` (XDG). Run `sysvibe --init-config`
to (re)generate it with comments. Key fields:

| Field | Default | Description |
|---|---|---|
| `theme` | `catppuccin-macchiato` | One of the built-in themes (see below) |
| `default_tab` | `dashboard` | Startup tab |
| `nerd_fonts` | `true` | Nerd Font icons; `false` → geometric fallback (Termux-friendly) |
| `data_refresh_rate` | `1000` | Fast metrics refresh interval (ms) |
| `process_refresh_rate` | `2000` | Process list refresh (ms) |
| `sensor_refresh_rate` | `5000` | Temperature/sensor refresh (ms) |
| `max_processes` | `50` | Max processes shown |
| `temperature_unit` | `celsius` | `celsius` or `fahrenheit` |
| `log_source` | `journalctl` | `journalctl`, `dmesg`, or `logcat` |
| `log_max_lines` | `1000` | Log buffer size |
| `show_gpu` | `true` | Show GPU tab/card |
| `show_battery` | `true` | Show battery panel/card |
| `resolve_public_ip` | `false` | **Opt-in**: resolve public IP via HTTPS request |
| `cpu_alert_threshold` | *unset* | CPU % alert (0–100) |
| `memory_alert_threshold` | *unset* | RAM % alert (0–100) |
| `temperature_alert_threshold` | *unset* | Temp alert (°C) |
| `disk_alert_threshold` | *unset* | Disk usage % alert (0–100) |

### Example

```toml
theme = "tokyo-night"
default_tab = "dashboard"
nerd_fonts = false
data_refresh_rate = 1000
resolve_public_ip = false

cpu_alert_threshold = 90.0
temperature_alert_threshold = 80.0
```

---

## 🎨 Themes

Seven built-in themes, switchable live with `T` or set in config:

`catppuccin-macchiato` · `catppuccin-mocha` · `dracula` · `nord` · `gruvbox` · `tokyo-night` · `one-dark`

Run `sysvibe --list-themes` to preview the names. Custom themes can be provided as a
TOML file passed to the theme loader.

---

## 🏗️ Architecture

```
src/
├── main.rs              # entry: terminal setup, async event loop (tokio::select!),
│                        # background collector threads → mpsc StateUpdate channel
├── app/
│   ├── mod.rs           # App state, navigation, alerts, command palette, export
│   ├── state.rs         # data structs, AppTab, AppMode
│   ├── events.rs        # key/mouse dispatch
│   ├── processes.rs     # process table/tree logic
│   ├── error.rs         # typed errors (thiserror)
│   └── collectors/
│       ├── mod.rs
│       ├── cpu.rs memory.rs network.rs sensors.rs     # cross-platform
│       └── linux/  android/                            # platform backends
├── config.rs            # XDG TOML config + validation + auto-generation
└── ui/
    ├── mod.rs           # draw: header / tabs / footer / toast / modals
    ├── header.rs footer.rs helpers.rs palette.rs theme.rs icons.rs
    ├── tabs/            # dashboard, system, hardware, processes, logs, gpu
    └── widgets/         # sparkline (braille/halfblock/mirrored), modal
```

- **Data flow:** background `std::thread` collectors (fast metrics, processes,
  sensors, GPU, logs) push `StateUpdate` messages over an mpsc channel; the main
  async loop applies them. Heavy blocking I/O never stalls the render loop.
- **Theming:** pluggable `Theme` with a thread-local palette accessor.

---

## 🛠️ Building from source

Requires Rust 1.88+ (edition 2024, uses let-chains).

```bash
git clone https://github.com/<you>/SysVibe.git
cd SysVibe
cargo run --release
```

Lint & test:

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 🧹 Uninstall

```bash
./uninstall.sh          # cargo uninstall + remove menu shortcut
```

The script also offers to remove the config directory.

---

## 📄 License

See `LICENSE` (or the crate metadata) for details.
