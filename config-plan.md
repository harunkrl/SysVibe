# Config Extension Plan — SysVibe

## Summary
Extended the Config system with widget visibility toggles, alert thresholds, and per-widget refresh rate overrides. Integrated alerts into the footer and widget visibility into tab rendering.

## Changes Made

### 1. `src/config.rs` — Config struct extended
**New widget visibility fields** (all `bool`, default `true`, serde default):
- `show_cpu_graph` — CPU history graph on dashboard
- `show_per_core` — per-core CPU usage on hardware tab
- `show_memory` — memory panel
- `show_network` — network panel
- `show_processes` — processes panel
- `show_temperatures` — temperature sensors panel
- `show_battery` — battery panel
- `show_logs` — logs tab

**New alert threshold fields** (all `Option<f32>`, default `None`):
- `cpu_alert_threshold` — CPU % (0-100)
- `memory_alert_threshold` — RAM % (0-100)
- `temperature_alert_threshold` — °C (0-150)
- `disk_alert_threshold` — disk usage % (0-100)

**New refresh override fields** (all `Option<u64>`, default `None`):
- `cpu_refresh_ms` — CPU refresh interval
- `network_refresh_ms` — Network refresh interval
- `disk_refresh_ms` — Disk I/O refresh interval
- `process_refresh_ms` — Process refresh interval
- `sensor_refresh_ms` — Sensor refresh interval
- `gpu_refresh_ms` — GPU refresh interval

**Validation**: All new fields are validated (thresholds clamped, refresh overrides clamped).

**Backward compatibility**: All new fields use `#[serde(default)]` or `#[serde(default = "default_true")]`, so existing configs without these fields load without issues.

**`generate_default_file()`**: Updated header comments to document the new field categories.

### 2. `src/app/mod.rs` — Alert checking
- Added `active_alerts: Vec<String>` field to `App`
- Added `check_alerts()` method that evaluates thresholds vs current metrics
- `check_alerts()` runs every ~4 ticks (~1s) from `on_tick()`
- Added `active_alerts()` public accessor for UI

### 3. `src/ui/footer.rs` — Alert display
- Footer now shows active alerts (with ⚠ icon) when no status message is present
- Alerts are displayed in yellow with the WARNING icon

### 4. `src/ui/tabs/dashboard.rs` — Widget visibility
- Dashboard layout dynamically adapts based on visibility toggles
- Hidden widgets don't consume layout space (rows/columns collapse)
- Battery visibility also checked in system+disk panel

### 5. `src/ui/tabs/hardware.rs` — Widget visibility
- Hardware tab layout dynamically adapts based on:
  - `show_cpu_graph` / `show_per_core` → CPU panel visibility
  - `show_memory` → Memory panel visibility
  - `show_network` → Network panel visibility
  - `show_disk_io` → Disk I/O panel visibility
  - `show_temperatures` → Temperature panel visibility

### 6. `src/ui/tabs/system.rs` — Widget visibility
- Battery panel on System tab hidden when `show_battery` is false
- When battery hidden, disk partitions take full right column

## Backward Compatibility
- All new config fields are optional with sensible defaults
- Existing config.toml files without the new fields load correctly
- `serde(default)` attributes ensure missing fields get default values
- No breaking changes to existing API

## Validation
- `cargo build --release` — passes ✓
- `cargo clippy -- -D warnings` — passes ✓
