# SysVibe — Unit Test Implementation Plan

## Summary
35 unit tests added across 5 modules. All tests use mock data, no real system dependencies. Tests run in < 1 second total.

## Test Modules

### 1. `src/config.rs` — 9 tests
| Test | Description |
|------|-------------|
| `test_default_config_values` | Verifies all default values match expected constants |
| `test_config_validate_clamps` | Values below minimum are clamped up |
| `test_config_validate_clamps_upper` | Values above maximum are clamped down |
| `test_config_invalid_temperature_unit` | "kelvin" → "celsius" |
| `test_config_valid_temperature_units` | All valid units preserved (case-insensitive) |
| `test_config_invalid_tab` | "nonexistent" → "dashboard" |
| `test_config_valid_tabs` | All 6 valid tabs preserved |
| `test_config_invalid_theme` | "nonexistent-theme" → "catppuccin-macchiato" |
| `test_config_valid_themes` | All 7 valid themes preserved |

### 2. `src/app/state.rs` — 9 tests
| Test | Description |
|------|-------------|
| `test_app_tab_default` | Default tab is Dashboard |
| `test_panel_focus_cycle_next` | Panel1→Panel6 wraps correctly |
| `test_panel_focus_cycle_prev` | Panel1←Panel6 wraps correctly |
| `test_panel_focus_round_trip` | 6 next() or 6 prev() returns to start |
| `test_panel_focus_is_focused` | is_focused() matches equality |
| `test_log_level_filter_default_shows_all` | Default shows all levels |
| `test_log_level_filter_allows_all_levels` | all() allows every level |
| `test_log_level_filter_selective` | Selective filtering works per-level |
| `test_sort_by_default` | Default sort is Cpu |
| `test_app_mode_default` | Default mode is Normal |

### 3. `src/app/processes.rs` — 5 tests
| Test | Description |
|------|-------------|
| `test_process_sort_by_cpu` | Descending CPU% order |
| `test_process_sort_by_mem` | Descending memory% order |
| `test_process_sort_by_pid` | Ascending PID order |
| `test_process_sort_by_name` | Alphabetical name order |
| `test_process_sort_by_cpu_tiebreak_pid` | Same CPU% tiebreaks by PID |

Uses `sort_process_entries()` helper with mock `ProcessEntry` data.

### 4. `src/app/collectors/sensors.rs` — 8 tests
| Test | Description |
|------|-------------|
| `test_clean_sensor_label_tctl` | "Tctl"/"tdie" → "CPU" |
| `test_clean_sensor_label_package` | "Package id 0" → "CPU Package" |
| `test_clean_sensor_label_nvme` | "NVMe"/"Composite" → correct labels |
| `test_clean_sensor_label_gpu` | "GPU"/"edge" → "GPU" |
| `test_clean_sensor_label_generic_sensor_filtered` | "sensor 3"/"temp3" → "" |
| `test_clean_sensor_label_wifi` | "iwlwifi"/"wlan0" → "WiFi" |
| `test_clean_sensor_label_title_case_fallback` | "some_custom" → "Some custom" |
| `test_clean_sensor_label_empty` | Empty string → "" |

### 5. `src/app/helpers.rs` — 3 tests
| Test | Description |
|------|-------------|
| `test_push_history_basic` | Push values, verify order |
| `test_push_history_evicts_oldest` | HISTORY_LEN+1 pushes evicts oldest |
| `test_push_history_exact_capacity` | Exactly HISTORY_LEN pushes, no eviction |

## Pre-existing Bugs Fixed
1. Missing `}` closing brace for `App` struct in `src/app/mod.rs`
2. `&bool` → `*v` dereference fix in `src/ui/tabs/dashboard.rs` line 37
