//! SysVibe — Configuration module (expanded for v0.3.0).
//!
//! Config loading from XDG-compliant TOML file with validation.

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
    // v0.3.0 additions
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
}

fn default_process_refresh() -> u64 { 2000 }
fn default_sensor_refresh() -> u64 { 5000 }
fn default_log_source() -> String { "auto".to_string() }
fn default_log_max_lines() -> usize { 500 }
fn default_true() -> bool { true }
fn default_tab() -> String { "system".to_string() }

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
        }
    }
}

impl Config {
    /// Load configuration from the XDG config directory, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut config: Config = toml::from_str(&content).unwrap_or_default();
                config.validate();
                return config;
            }
        }
        Self::default()
    }

    fn validate(&mut self) {
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
        let valid_tabs = ["system", "hardware", "processes", "logs"];
        if !valid_tabs.contains(&self.default_tab.to_lowercase().as_str()) {
            self.default_tab = "system".to_string();
        }
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sysvibe")
            .join("config.toml")
    }
}
