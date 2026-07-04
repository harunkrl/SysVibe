# Changelog

All notable changes to Vitalis are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-06-24 — UI redesign & bug fixes

### Added

- **Dashboard hero stat cards** (CPU/RAM/GPU/Net/Temp/Battery) with mini sparklines and adaptive count
- **Command palette** (`:`): fuzzy action menu — jump tabs, switch theme, export, toggle units, quit
- **Live theme switching** (`T`) across 7 themes — no restart needed
- **Rich battery panel**: power draw (W), health %, cycle count
- **Hardware tab**: temperature sensors, disk I/O read/write graphs, network mirrored RX↑/TX↓ graph
- **Toast alert banner** for CPU/Mem/Temp/Disk threshold breaches
- **Compact/stacked layout** for narrow terminals (auto-engages `<90` cols; Android/Termux)
- **Logcat** integration + dynamic "Logcat Logs" tab title on Android
- **Unified bar language** (`█`/`░`) across all tabs via `usage_bar`/`usage_bar_spans`
- **Opt-in public-IP resolution** (`resolve_public_ip`, off by default — no outbound calls)
- Number-key tab shortcuts (`1`–`6`) with visible hints; mouse click covers all pill rows

### Changed

- Header rewritten: all 6 tabs visible, navigation order matches visual order (single source of truth)
- Process table: value-colored text instead of jarring full-background rows
- Replaced semi-circle radial gauges with unified usage bars (removed `radial` module)
- Fixed broken fallback icons (double-width emoji → Termux-safe geometric glyphs)
- README and install/uninstall scripts rewritten (Termux detection, config generation)

### Fixed

- Memory breakdown (used/cache/free) was frozen since startup — now fed fresh via the background channel
- Total process count was stale — now live
- Log de-dup no longer drops same-second entries that have different messages

### Removed

- `daemon_enabled`/`daemon_port` config fields (unimplemented "daemon" feature)
- Dead code: `header_block`, `braille_mini`, `radial` module
- Leftover `scratch_canvas.rs`

## [0.4.0] - 2026-06-01

### Added

- **Deep Hardware Integration**: New `HardwareData` collector fetches static hardware details once at startup
  - Motherboard: vendor, model, revision, BIOS vendor/version/date via `/sys/class/dmi/id/`
  - GPU(s): model, PCI slot, driver via `lspci -nn` + SysFS driver symlink
  - RAM: DDR type, speed (MT/s), DIMM count, form factor via `lshw` → `dmidecode` → SysFS → CPU heuristic fallback
- **CPU Micro-Sparklines**: Per-core 4-char braille sparklines in Hardware tab (btop-style)
- **Network Heartbeat Graph**: Mirrored braille graph (RX ▲ SKY / TX ▼ MAUVE) with center axis
- **Embedded Border Titles**: Interior sub-headers removed; titles break the top border line natively

### Changed

- **Performance overhaul**: Reduced CPU overhead from ~15% to ~1-3%
  - Removed `refresh_processes(All, true)` from every tick — now manual-only via `r` key
  - Cached `local_ip` at startup instead of opening UDP socket every tick
  - Merged dual `/proc/diskstats` reads into single `read_diskstats()` call
  - Eliminated `NetworkStats` clone on every refresh via `swap_remove()`
- **Process CPU% normalization**: Divided by core count to show 0-100% range (matching htop/btop)
- **Temperature bars**: Color-coded `[████░░░░]` bars with temperature scaling
- **Power draw graph**: Braille line graph showing 0 → max W with Y-axis labels
- **README**: Complete rewrite with architecture diagram, feature table, and configuration reference

### Fixed

- Process CPU percentages exceeding 100% on multi-core systems (e.g. Brave showing 300%)
- Vitalis consuming 11-15% CPU due to per-tick `refresh_processes`
- RAM details missing when `lshw` not installed — now falls back through 4 detection tiers
- No `/proc/diskstats` double-read per tick

## [0.3.0] - 2026-06-01

### Added

- **Kernel Log Viewer**: Real-time kernel logs via journalctl/dmesg with color-coded log levels
- **Redesigned System Tab**: 2x2 grid layout with Quick Stats (CPU/RAM/Swap/Battery gauges) and CPU history sparkline
- **Modular Architecture**: Codebase restructured into ~20 focused modules for maintainability
- **Local Timezone**: Clock now displays local time instead of UTC (via chrono)
- **Mouse Support**: Click tabs to switch, scroll wheel in process list
- **Page Navigation**: PageUp/PageDown/Home/End in process list
- **Visual Process Bars**: Mini CPU/MEM bars in process table rows
- **Sort Indicator**: Active sort column shows ▼ marker in header
- **Multi-select Kill Info**: Kill confirmation modal now shows count for multi-select
- **Expanded Configuration**: New config options for refresh rates, log source, default tab
- **SIGKILL Option**: Kill modal now shows [K] for force kill (SIGKILL)
- **LICENSE File**: MIT License added
- **CHANGELOG**: This file

### Changed

- **Performance**: Tiered refresh rates — sensors every 5s, processes every 2s, CPU/network every tick
- **Help Modal**: Now lists ALL keybindings including Tab, Space, r, t, c, f
- **Sensors Panel**: Shows up to 6 sensors instead of 3
- **System Tab**: Information-dense 2x2 layout replaces sparse 2-column layout
- **Config**: Expanded with `process_refresh_rate`, `sensor_refresh_rate`, `log_source`, `log_max_lines`, `show_gpu`, `default_tab`
- **Version**: Bumped to 0.3.0

### Fixed

- High CPU usage (~43% → target <10%) via tiered data collection
- Swap gauge not visible when swap space is 0
- UTC-only clock display replaced with local timezone
- CPU brand text truncation in narrow panels

## [0.2.0] - Initial Release

### Features

- CPU & Memory Monitoring with Braille sparkline graphs
- Per-core CPU usage grid
- Network I/O with real-time speed and history
- Disk I/O monitoring
- Process Manager with filter, sort, and kill
- Hardware sensor temperatures and battery status
- Catppuccin Macchiato color palette
- Zero-flicker rendering with Crossterm
- XDG-compliant configuration
- Desktop entry installation script
