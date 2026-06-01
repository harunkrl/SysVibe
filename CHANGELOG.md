# Changelog

All notable changes to SysVibe are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
