//! SysVibe — Configuration module (expanded).
//!
//! Config loading from XDG-compliant TOML file with validation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ui_tick_rate: u64,
    pub data_refresh_rate: u64,
    pub show_braille_graphs: bool,
    pub show_disk_io: bool,
    pub temperature_unit: String,
    pub max_processes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui_tick_rate: 250,
            data_refresh_rate: 1000,
            show_braille_graphs: true,
            show_disk_io: true,
            temperature_unit: "celsius".to_string(),
            max_processes: 50,
        }
    }
}

impl Config {
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
        let lower = self.temperature_unit.to_lowercase();
        if lower != "celsius" && lower != "fahrenheit" {
            self.temperature_unit = "celsius".to_string();
        }
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sysvibe")
            .join("config.toml")
    }
}
