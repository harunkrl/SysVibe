# SysVibe

<p align="center">
  <strong>A modern, zero-flicker system monitor TUI for Linux &amp; Android (Termux)</strong>
</p>

<p align="center">
  Built with <code>ratatui</code> · <code>crossterm</code> · <code>sysinfo</code> · <code>tokio</code><br>
  Colored by <a href="https://github.com/catppuccin/catppuccin">Catppuccin</a> (+ 6 more themes) · Graphed with braille &amp; half block
</p>

---

## Highlights

- **Six live tabs** — Dashboard, System, Hardware, Processes, Logs, GPU.
- **Hero stat cards** — at-a-glance CPU / RAM / GPU / Net / Temp / Battery with mini sparklines; the Temp card stacks the CPU and GPU readings.
- **Live theming** — 7 built-in themes plus custom TOML themes, switchable at runtime (`T`).
- **Command palette** — fuzzy action menu (`:`): jump tabs, switch theme, export, quit, and more.
- **Alerts and toasts** — configurable CPU / Mem / Temp / Disk thresholds with a prominent toast banner.
- **Compact mode** — narrow terminals (Android/Termux portrait) auto-stack to a single column.
- **Low CPU overhead** — async event loop, background `std::thread` collectors, tiered refresh.
- **Deep hardware** — CPU caches/microcode/flags, RAM/battery breakdown, storage devices, network interfaces, temperatures, disk I/O, and NVIDIA/AMD/Intel GPUs.
- **Log viewer** — `journalctl -o json` / `dmesg` (Linux), `logcat` (Android): accurate timestamps/levels/sources, kernel/system scope, severity filters, scrollable history.
- **Mouse and keyboard** — click tabs, scroll lists, *and* full vim-style keyboard control.

---

## Installation

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

## Usage

```bash
sysvibe                          # run with default settings
sysvibe --init-config            # create/edit the config file, then exit
sysvibe --list-themes            # list available themes, then exit
```

The first run writes a default config to `~/.config/sysvibe/config.toml` (XDG).
Edit it to change theme, refresh rates, alert thresholds, and more.

---

## Tabs

- **1 - Dashboard** — hero stat cards (Temp card shows stacked CPU + GPU readings), a CPU history graph with per-core clusters and the CPU temperature shown on the frequency line, a GPU Info panel (braille usage trend + Power/Temp/Clock/VRAM), memory and disk overview, and a sortable top-processes list.
- **2 - System** — two-column inventory: identity (OS/kernel/host, motherboard/BIOS, session, boot/security/locale, app-about) and hardware (CPU caches/microcode/flags, RAM, storage devices, network interfaces, GPUs). Theme-coloured panels.
- **3 - Hardware** — per-core CPU clusters, memory & swap breakdown, battery charge plus a power-draw trend graph (1 s sampling, 0-30 W scale), deduplicated and ordered temperature sensors (CPU to GPU to NVMe to WiFi to ACPI), fan/power-profile, and disk I/O graphs.
- **4 - Processes** — frozen table (refresh on `r`), sort plus direction, name/PID/cmdline filter, multi-select and marked-only view, USER column (root highlighted), gradient bars, tree view, and kill via sysinfo (with confirmation modal).
- **5 - Logs** — real-time `journalctl -o json` (accurate timestamps/levels/sources) or `dmesg` (`logcat` on Android), kernel/system scope toggle, scrollable history, level filters, follow mode.
- **6 - GPU** — usage (1 Hz braille trend on the Dashboard), VRAM (dedicated percent gauge; iGPUs/APUs honestly labelled "Shared RAM"), temperature, power, fan, clock per GPU (NVIDIA/AMD/Intel). NVIDIA GPUs also show per-process attribution — which process is using GPU/VRAM (`nvidia-smi --query-compute-apps`).

> Tabs shrink to a single stacked column when the terminal is narrow (under 90 columns).

---

## Keybindings

### Global

| Key | Action |
|---|---|
| `1`-`6` | Jump to tab |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `[` / `]` | Cycle panel focus |
| `:` | Command palette |
| `T` | Cycle theme |
| `t` | Toggle Celsius / Fahrenheit |
| `h` / `?` | Help modal |
| `/` | Filter |
| `q` / `Esc` | Quit |

### Navigation

| Key | Action |
|---|---|
| `j` / `ArrowDown` · `k` / `ArrowUp` | Move down / up |
| `PageDown` / `PageUp` | Page down / up |
| `Home` / `End` | Top / bottom |

### Processes tab

| Key | Action |
|---|---|
| `s` | Cycle sort (CPU to Mem to PID to Name) |
| `S` | Toggle sort direction (asc / desc) |
| `g` | Toggle CPU view: per-core (raw) / normalized *(Dashboard & Hardware tabs only)* |
| `r` | Refresh (the table is otherwise frozen so browsing isn't disrupted) |
| `/` | Filter by name, PID, or command line |
| `Space` | Toggle select |
| `m` | Toggle marked-only view (show just space-selected processes) |
| `x` | Kill selected (confirm) |
| `p` / `F5` | Toggle tree view |
| `c` | Clear selection |
| `E` | Export snapshot (JSON) |

### Logs tab

| Key | Action |
|---|---|
| `f` | Toggle follow |
| `ArrowUp` / `ArrowDown` · `PgUp` / `PgDn` · `Home` / `End` | Scroll the log view |
| `s` | Toggle scope: kernel-only to full system journal |
| `e` / `w` / `i` | Toggle Error / Warning / Info level filter |
| `n` / `d` | Toggle Notice / Debug level filter |
| `r` | Refresh |

### Command palette (`:`)

Type to fuzzy-match; arrow keys navigate, `Enter` runs, `Esc` cancels, `Ctrl+U` clears.

---

## Configuration

Config lives at `~/.config/sysvibe/config.toml` (XDG). Run `sysvibe --init-config`
to (re)generate it with comments. Key fields:

| Field | Default | Description |
|---|---|---|
| `theme` | `catppuccin-macchiato` | One of the built-in themes (see below) |
| `default_tab` | `dashboard` | Startup tab |
| `nerd_fonts` | `true` | Nerd Font icons; `false` for the geometric fallback (Termux-friendly) |
| `data_refresh_rate` | `1000` | Fast metrics refresh interval (ms) |
| `process_refresh_rate` | `2000` | Background process refresh (ms); the table only updates the display on `r` |
| `sensor_refresh_rate` | `5000` | Temperature/sensor refresh (ms) |
| `max_processes` | `50` | Max processes shown |
| `temperature_unit` | `celsius` | `celsius` or `fahrenheit` |
| `log_source` | `auto` | `journalctl`, `dmesg`, `logcat`, or `auto` (kernel/system scope also toggles live with `s`) |
| `log_max_lines` | `1000` | Log buffer size |
| `show_gpu` | `true` | Show GPU tab/card |
| `show_battery` | `true` | Show battery panel/card |
| `resolve_public_ip` | `false` | Opt-in: resolve public IP via HTTPS request |
| `cpu_alert_threshold` | *unset* | CPU percent alert (0-100) |
| `memory_alert_threshold` | *unset* | RAM percent alert (0-100) |
| `temperature_alert_threshold` | *unset* | Temp alert (Celsius) |
| `disk_alert_threshold` | *unset* | Disk usage percent alert (0-100) |

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

## Themes

Seven built-in themes, switchable live with `T` or set in config:

`catppuccin-macchiato` · `catppuccin-mocha` · `dracula` · `nord` · `gruvbox` · `tokyo-night` · `one-dark`

Run `sysvibe --list-themes` to preview the names. Custom themes can be provided as a
TOML file passed to the theme loader.

---

## Architecture

```
src/
├── main.rs              # entry: terminal setup, async event loop (tokio::select!),
│                        # background collector threads -> mpsc StateUpdate channel
├── app/
│   ├── mod.rs           # App struct + constructor + static helpers
│   ├── accessors.rs     # public read-only accessors (data the UI renders)
│   ├── mutations.rs     # event-driven state mutations
│   ├── state_update.rs  # apply StateUpdate messages from collector threads
│   ├── tick.rs          # lightweight per-tick update
│   ├── refresh.rs       # tiered heavy data refresh
│   ├── process_ops.rs   # process refresh/kill/mark/sort
│   ├── events_dispatch.rs # top-level key routing
│   ├── sample.rs        # preview-only sample data (svshot; behind `preview` feature)
│   ├── state.rs         # data structs, AppTab, AppMode
│   ├── events.rs        # key/mouse dispatch
│   ├── processes.rs     # process table/tree logic
│   ├── error.rs         # typed errors (thiserror)
│   └── collectors/
│       ├── mod.rs
│       ├── cpu.rs memory.rs network.rs sensors.rs disk.rs  # cross-platform
│       └── linux/  android/                            # platform backends
│           (linux: battery, logs(journalctl -o json), gpu, hardware, sensors)
├── config.rs            # XDG TOML config + validation + auto-generation
└── ui/
    ├── mod.rs           # draw: header / tabs / footer / toast / modals
    ├── header.rs footer.rs helpers.rs palette.rs theme.rs icons.rs
    ├── tabs/            # dashboard, system, hardware, processes, logs, gpu
    └── widgets/         # sparkline (braille/halfblock/mirrored), modal
```

- **Data flow:** background `std::thread` collectors (fast metrics, processes,
  sensors+fans+power-profile, GPU, logs) push `StateUpdate` messages over an mpsc
  channel; the main async loop applies them. Heavy blocking I/O never stalls the
  render loop. The process table is intentionally frozen (refresh on `r`) and
  the battery power-draw graph samples at 1 s alongside the fast metrics.
- **GPU trend sampling:** AMD/Intel GPUs sample usage via a cheap sysfs read at
  1 Hz; NVIDIA keeps the 5 s sensor cadence (nvidia-smi is too heavy per tick).
- **Theming:** pluggable `Theme` with a thread-local palette accessor.

---

## Building from source

Requires Rust 1.88+ (edition 2024, uses let-chains).

```bash
git clone https://github.com/harunkrl/SysVibe.git
cd SysVibe
cargo run --release
```

Lint and test:

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## Uninstall

```bash
./uninstall.sh          # cargo uninstall + remove menu shortcut
```

The script also offers to remove the config directory.

---

## License

See `LICENSE` (or the crate metadata) for details.
