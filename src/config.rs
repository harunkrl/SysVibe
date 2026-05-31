//! SysVibe — Configuration system (XDG standard).
//!
//! Reads from `~/.config/sysvibe/config.toml` via the `dirs` crate.
//! Falls back to compiled-in defaults on any error (missing file,
//! malformed TOML, invalid values).

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

// ── Config struct ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// UI tick rate in milliseconds (keyboard responsiveness).
    pub ui_tick_rate: u64,
    /// Data refresh rate in milliseconds (sysinfo polling).
    pub data_refresh_rate: u64,
    /// Whether to render braille sparkline graphs.
    pub show_braille_graphs: bool,
    /// Whether to show the Disk I/O panel.
    pub show_disk_io: bool,
    /// Maximum number of processes shown in the table.
    pub max_processes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui_tick_rate: 250,
            data_refresh_rate: 1000,
            show_braille_graphs: true,
            show_disk_io: true,
            max_processes: 10,
        }
    }
}

impl Config {
    /// Load configuration from `~/.config/sysvibe/config.toml`.
    /// Returns defaults on any error.
    pub fn load() -> Self {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    /// Returns the XDG config path for SysVibe.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("sysvibe")
            .join("config.toml")
    }

    fn load_from(path: &std::path::Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        toml::from_str(&content).unwrap_or_default()
    }

    /// Returns an example configuration as a static string.
    #[allow(dead_code)]
    pub fn example_toml() -> &'static str {
        r#"# SysVibe Configuration
# Place at ~/.config/sysvibe/config.toml

# UI tick rate in milliseconds (controls keyboard responsiveness)
ui_tick_rate = 250

# Data refresh rate in milliseconds (controls how often sysinfo is polled)
data_refresh_rate = 1000

# Show braille sparkline graphs for CPU, Network, Disk
show_braille_graphs = true

# Show Disk I/O panel
show_disk_io = true

# Maximum number of processes shown in the table
max_processes = 10
"#
    }
}
