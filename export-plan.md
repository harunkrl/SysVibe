# Export Feature - Implementation Plan & Notes

## Overview
Added export functionality to SysVibe that saves current system state to JSON (or CSV) files.

## Files Changed
| File | Change |
|------|--------|
| `Cargo.toml` | Added `serde_json = "1"` |
| `src/app/collectors/export.rs` | **NEW** — Export module with snapshot builder, CSV/JSON writers |
| `src/app/collectors/mod.rs` | Registered `pub mod export;` |
| `src/app/mod.rs` | Added `export_snapshot()` method to `App` struct |
| `src/app/events.rs` | Added `'e'` key for export (non-Logs tabs), `Shift+E` for export on all tabs |

## Design Decisions
- **JSON default**: Export uses JSON with pretty-print by default. CSV is supported but requires code change to switch.
- **Manual CSV**: No csv crate added. CSV is hand-generated with proper quoting for fields containing commas/quotes.
- **Serde-derived structs**: Separate `ExportXxx` structs (not reusing state types directly) to keep serialization clean and avoid polluting state types with serde derives.
- **Key binding**: `'e'` on non-Logs tabs triggers export. On Logs tab, `'e'` preserves the existing log error toggle. `Shift+E` (`'E'`) triggers export from any tab.
- **File location**: Uses `dirs::data_dir()` → `$XDG_DATA_DIR/sysvibe/exports/`
- **Error handling**: Errors shown via `set_error()` in the footer status message.

## Data Exported
- System info (OS, kernel, hostname, uptime, CPU brand, cores, architecture, load averages)
- CPU usage (overall %, per-core %)
- Memory (RAM and Swap used/total in GiB)
- Network (per interface: rx/tx speed, total bytes)
- Disk I/O (read/write speed, IOPS)
- Disk partitions (mount, device, fs type, total/used/available)
- GPU (name, usage %, VRAM, temperature) — if available
- Top processes (PID, name, CPU%, MEM%)
