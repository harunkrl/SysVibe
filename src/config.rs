//! SysVibe — Configuration module.
//!
//! Config loading from XDG-compliant TOML file with validation.
//! Supports pluggable themes and auto-generation of default config.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration loaded from `~/.config/sysvibe/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ui_tick_rate: u64,
    pub data_refresh_rate: u64,
    pub show_braille_graphs: bool,
    pub show_disk_io: bool,
    pub temperature_unit: String,
    pub max_processes: usize,
    #[serde(default = "default_process_refresh")]
    pub process_refresh_rate: u64,
    #[serde(default = "default_sensor_refresh")]
    pub sensor_refresh_rate: u64,
    #[serde(default = "default_log_source")]
    pub log_source: String,
    #[serde(default = "default_log_max_lines")]
    pub log_max_lines: usize,
    #[serde(default = "default_true")]
    pub show_gpu: bool,
    #[serde(default = "default_tab")]
    pub default_tab: String,
    #[serde(default = "default_true")]
    pub nerd_fonts: bool,
    /// Theme name: "catppuccin-macchiato", "catppuccin-mocha", "dracula", "nord", "gruvbox", "tokyo-night", "one-dark"
    #[serde(default = "default_theme")]
    pub theme: String,

    // ── Widget visibility toggles ─────────────────────────────────
    /// Show CPU history graph on dashboard.
    #[serde(default = "default_true")]
    pub show_cpu_graph: bool,
    /// Show per-core CPU usage.
    #[serde(default = "default_true")]
    pub show_per_core: bool,
    /// Show memory panel.
    #[serde(default = "default_true")]
    pub show_memory: bool,
    /// Show network panel.
    #[serde(default = "default_true")]
    pub show_network: bool,
    /// Show processes panel.
    #[serde(default = "default_true")]
    pub show_processes: bool,
    /// Show temperature sensors panel.
    #[serde(default = "default_true")]
    pub show_temperatures: bool,
    /// Show battery panel.
    #[serde(default = "default_true")]
    pub show_battery: bool,
    /// Resolve the public IP via an outbound HTTPS request (opt-in; off by default).
    #[serde(default)]
    pub resolve_public_ip: bool,
    /// Show logs tab.
    #[serde(default = "default_true")]
    pub show_logs: bool,

    // ── Alert thresholds ──────────────────────────────────────────
    /// CPU usage alert threshold (0–100). None = disabled.
    #[serde(default)]
    pub cpu_alert_threshold: Option<f32>,
    /// Memory usage alert threshold (0–100). None = disabled.
    #[serde(default)]
    pub memory_alert_threshold: Option<f32>,
    /// Temperature alert threshold in °C. None = disabled.
    #[serde(default)]
    pub temperature_alert_threshold: Option<f32>,
    /// Disk usage alert threshold (0–100). None = disabled.
    #[serde(default)]
    pub disk_alert_threshold: Option<f32>,

    // ── Per-widget refresh rate overrides ─────────────────────────
    /// CPU refresh interval in ms. None = use data_refresh_rate.
    #[serde(default)]
    pub cpu_refresh_ms: Option<u64>,
    /// Network refresh interval in ms. None = use data_refresh_rate.
    #[serde(default)]
    pub network_refresh_ms: Option<u64>,
    /// Disk refresh interval in ms. None = use data_refresh_rate.
    #[serde(default)]
    pub disk_refresh_ms: Option<u64>,
    /// Process refresh interval in ms. None = use process_refresh_rate.
    #[serde(default)]
    pub process_refresh_ms: Option<u64>,
    /// Sensor refresh interval in ms. None = use sensor_refresh_rate.
    #[serde(default)]
    pub sensor_refresh_ms: Option<u64>,
    /// GPU refresh interval in ms. None = use sensor_refresh_rate.
    #[serde(default)]
    pub gpu_refresh_ms: Option<u64>,
}

fn default_process_refresh() -> u64 { 2000 }
fn default_sensor_refresh() -> u64 { 5000 }
fn default_log_source() -> String { "auto".to_string() }
fn default_log_max_lines() -> usize { 500 }
fn default_true() -> bool { true }
fn default_tab() -> String { "dashboard".to_string() }
fn default_theme() -> String { "catppuccin-macchiato".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            ui_tick_rate: 250,
            data_refresh_rate: 1000,
            show_braille_graphs: true,
            show_disk_io: true,
            temperature_unit: "celsius".to_string(),
            max_processes: 50,
            process_refresh_rate: default_process_refresh(),
            sensor_refresh_rate: default_sensor_refresh(),
            log_source: default_log_source(),
            log_max_lines: default_log_max_lines(),
            show_gpu: default_true(),
            default_tab: default_tab(),
            nerd_fonts: default_true(),
            theme: default_theme(),
            // Widget visibility
            show_cpu_graph: default_true(),
            show_per_core: default_true(),
            show_memory: default_true(),
            show_network: default_true(),
            show_processes: default_true(),
            show_temperatures: default_true(),
            show_battery: default_true(),
            resolve_public_ip: false,
            show_logs: default_true(),
            // Alert thresholds
            cpu_alert_threshold: None,
            memory_alert_threshold: None,
            temperature_alert_threshold: None,
            disk_alert_threshold: None,
            // Refresh rate overrides
            cpu_refresh_ms: None,
            network_refresh_ms: None,
            disk_refresh_ms: None,
            process_refresh_ms: None,
            sensor_refresh_ms: None,
            gpu_refresh_ms: None,
        }
    }
}

impl Config {
    /// Load configuration from the XDG config directory, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let mut config: Config = toml::from_str(&content).unwrap_or_default();
            config.validate();
            return config;
        }
        Self::default()
    }

    /// Generate a default config file at the XDG config path.
    /// Returns the path where the file was written.
    pub fn generate_default_file() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = Self::config_path();

        // Create parent directory
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let default_config = Self::default();
        let toml_content = toml::to_string_pretty(&default_config)?;

        // Add comments
        let commented = format!(
            "# SysVibe Configuration\n\
             # Generated automatically — modify as needed.\n\
             # Place at ~/.config/sysvibe/config.toml\n\
             #\n\
             # Available themes: catppuccin-macchiato, catppuccin-mocha,\n\
             #   dracula, nord, gruvbox, tokyo-night, one-dark\n\
             # Available default_tab: dashboard, system, hardware, processes, logs\n\
             # Temperature unit: celsius or fahrenheit\n\
             #\n\
             # Widget visibility (show_*) — set to false to hide specific widgets\n\
             # Alert thresholds — set a value (0-100 for %, degrees for temperature)\n\
             #   to trigger footer warnings when exceeded. Leave unset/null to disable.\n\
             # Refresh overrides — per-widget refresh interval in ms. Leave unset/null\n\
             #   to use the default tier intervals.\n\n\
             {}",
            toml_content,
        );

        std::fs::write(&path, commented)?;

        Ok(path)
    }

    pub(crate) fn validate(&mut self) {
        self.ui_tick_rate = self.ui_tick_rate.clamp(50, 5000);
        self.data_refresh_rate = self.data_refresh_rate.clamp(250, 30_000);
        self.max_processes = self.max_processes.clamp(5, 500);
        self.process_refresh_rate = self.process_refresh_rate.clamp(500, 30_000);
        self.sensor_refresh_rate = self.sensor_refresh_rate.clamp(1000, 60_000);
        self.log_max_lines = self.log_max_lines.clamp(50, 5000);
        let lower = self.temperature_unit.to_lowercase();
        if lower != "celsius" && lower != "fahrenheit" {
            self.temperature_unit = "celsius".to_string();
        }
        let valid_tabs = ["dashboard", "system", "hardware", "processes", "logs", "gpu"];
        if !valid_tabs.contains(&self.default_tab.to_lowercase().as_str()) {
            self.default_tab = "dashboard".to_string();
        }
        let valid_themes = [
            "catppuccin-macchiato", "catppuccin-mocha", "dracula",
            "nord", "gruvbox", "tokyo-night", "one-dark",
        ];
        if !valid_themes.contains(&self.theme.to_lowercase().as_str()) {
            self.theme = "catppuccin-macchiato".to_string();
        }

        // Validate alert thresholds
        if let Some(t) = self.cpu_alert_threshold {
            self.cpu_alert_threshold = Some(t.clamp(0.0, 100.0));
        }
        if let Some(t) = self.memory_alert_threshold {
            self.memory_alert_threshold = Some(t.clamp(0.0, 100.0));
        }
        if let Some(t) = self.temperature_alert_threshold {
            self.temperature_alert_threshold = Some(t.clamp(0.0, 150.0));
        }
        if let Some(t) = self.disk_alert_threshold {
            self.disk_alert_threshold = Some(t.clamp(0.0, 100.0));
        }

        // Validate refresh overrides
        if let Some(ms) = self.cpu_refresh_ms {
            self.cpu_refresh_ms = Some(ms.clamp(250, 30_000));
        }
        if let Some(ms) = self.network_refresh_ms {
            self.network_refresh_ms = Some(ms.clamp(250, 30_000));
        }
        if let Some(ms) = self.disk_refresh_ms {
            self.disk_refresh_ms = Some(ms.clamp(250, 30_000));
        }
        if let Some(ms) = self.process_refresh_ms {
            self.process_refresh_ms = Some(ms.clamp(500, 30_000));
        }
        if let Some(ms) = self.sensor_refresh_ms {
            self.sensor_refresh_ms = Some(ms.clamp(1000, 60_000));
        }
        if let Some(ms) = self.gpu_refresh_ms {
            self.gpu_refresh_ms = Some(ms.clamp(1000, 60_000));
        }
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sysvibe")
            .join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let cfg = Config::default();
        assert_eq!(cfg.ui_tick_rate, 250);
        assert_eq!(cfg.data_refresh_rate, 1000);
        assert!(cfg.show_braille_graphs);
        assert!(cfg.show_disk_io);
        assert_eq!(cfg.temperature_unit, "celsius");
        assert_eq!(cfg.max_processes, 50);
        assert_eq!(cfg.process_refresh_rate, 2000);
        assert_eq!(cfg.sensor_refresh_rate, 5000);
        assert_eq!(cfg.log_source, "auto");
        assert_eq!(cfg.log_max_lines, 500);
        assert!(cfg.show_gpu);
        assert_eq!(cfg.default_tab, "dashboard");
        assert!(cfg.nerd_fonts);
        assert_eq!(cfg.theme, "catppuccin-macchiato");
    }

    #[test]
    fn test_config_validate_clamps() {
        let mut cfg = Config {
            ui_tick_rate: 1,
            data_refresh_rate: 10,
            max_processes: 0,
            process_refresh_rate: 100,
            sensor_refresh_rate: 500,
            log_max_lines: 1,
            ..Config::default()
        };
        cfg.validate();
        assert_eq!(cfg.ui_tick_rate, 50);          // clamped to min
        assert_eq!(cfg.data_refresh_rate, 250);     // clamped to min
        assert_eq!(cfg.max_processes, 5);           // clamped to min
        assert_eq!(cfg.process_refresh_rate, 500);  // clamped to min
        assert_eq!(cfg.sensor_refresh_rate, 1000);  // clamped to min
        assert_eq!(cfg.log_max_lines, 50);          // clamped to min
    }

    #[test]
    fn test_config_validate_clamps_upper() {
        let mut cfg = Config {
            ui_tick_rate: 99999,
            data_refresh_rate: 99999,
            max_processes: 99999,
            process_refresh_rate: 99999,
            sensor_refresh_rate: 99999,
            log_max_lines: 99999,
            ..Config::default()
        };
        cfg.validate();
        assert_eq!(cfg.ui_tick_rate, 5000);
        assert_eq!(cfg.data_refresh_rate, 30_000);
        assert_eq!(cfg.max_processes, 500);
        assert_eq!(cfg.process_refresh_rate, 30_000);
        assert_eq!(cfg.sensor_refresh_rate, 60_000);
        assert_eq!(cfg.log_max_lines, 5000);
    }

    #[test]
    fn test_config_invalid_temperature_unit() {
        let mut cfg = Config {
            temperature_unit: "kelvin".to_string(),
            ..Config::default()
        };
        cfg.validate();
        assert_eq!(cfg.temperature_unit, "celsius");
    }

    #[test]
    fn test_config_valid_temperature_units() {
        for unit in &["celsius", "fahrenheit", "Celsius", "Fahrenheit", "CELSIUS"] {
            let mut cfg = Config {
                temperature_unit: unit.to_string(),
                ..Config::default()
            };
            cfg.validate();
            assert_eq!(cfg.temperature_unit.to_lowercase(), unit.to_lowercase());
        }
    }

    #[test]
    fn test_config_invalid_tab() {
        let mut cfg = Config {
            default_tab: "nonexistent".to_string(),
            ..Config::default()
        };
        cfg.validate();
        assert_eq!(cfg.default_tab, "dashboard");
    }

    #[test]
    fn test_config_valid_tabs() {
        for tab in &["dashboard", "system", "hardware", "processes", "logs", "gpu"] {
            let mut cfg = Config {
                default_tab: tab.to_string(),
                ..Config::default()
            };
            cfg.validate();
            assert_eq!(cfg.default_tab, *tab);
        }
    }

    #[test]
    fn test_config_invalid_theme() {
        let mut cfg = Config {
            theme: "nonexistent-theme".to_string(),
            ..Config::default()
        };
        cfg.validate();
        assert_eq!(cfg.theme, "catppuccin-macchiato");
    }

    #[test]
    fn test_config_valid_themes() {
        for theme in &[
            "catppuccin-macchiato", "catppuccin-mocha", "dracula",
            "nord", "gruvbox", "tokyo-night", "one-dark",
        ] {
            let mut cfg = Config {
                theme: theme.to_string(),
                ..Config::default()
            };
            cfg.validate();
            assert_eq!(cfg.theme, *theme);
        }
    }
}
