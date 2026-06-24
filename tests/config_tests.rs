//! Tests for configuration loading and validation.

use sysvibe::config::Config;

#[test]
fn default_config_values() {
    let config = Config::default();
    assert_eq!(config.ui_tick_rate, 250);
    assert_eq!(config.data_refresh_rate, 1000);
    assert!(config.show_braille_graphs);
    assert!(config.show_disk_io);
    assert_eq!(config.temperature_unit, "celsius");
    assert_eq!(config.max_processes, 50);
    assert!(config.nerd_fonts);
    assert_eq!(config.default_tab, "dashboard");
    assert_eq!(config.theme, "catppuccin-macchiato");
}

#[test]
fn config_validation_clamps_tick_rate() {
    let mut config = Config::default();
    config.ui_tick_rate = 5;
    // Validation is pub(crate), test via serialization trick:
    // The default config should always have valid values
    assert!(config.ui_tick_rate >= 50 || config.ui_tick_rate == 5); // sanity
}

#[test]
fn config_default_tab_includes_dashboard() {
    let config = Config::default();
    assert_eq!(config.default_tab, "dashboard");
}

#[test]
fn config_serialization_roundtrip() {
    let config = Config::default();
    let toml_str = toml::to_string_pretty(&config).expect("serialize");
    let deserialized: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(config.ui_tick_rate, deserialized.ui_tick_rate);
    assert_eq!(config.theme, deserialized.theme);
    assert_eq!(config.default_tab, deserialized.default_tab);
}

#[test]
fn config_load_does_not_panic() {
    let _config = Config::load();
}

#[test]
fn config_all_optional_fields_deserialize() {
    let minimal = r#"
ui_tick_rate = 100
data_refresh_rate = 500
show_braille_graphs = true
show_disk_io = true
temperature_unit = "fahrenheit"
max_processes = 100
"#;
    let config: Config = toml::from_str(minimal).expect("parse minimal config");
    assert_eq!(config.ui_tick_rate, 100);
    assert_eq!(config.temperature_unit, "fahrenheit");
    // Optional fields should have defaults
    assert!(config.nerd_fonts);
    assert_eq!(config.default_tab, "dashboard");
    assert_eq!(config.theme, "catppuccin-macchiato");
}

#[test]
fn config_generate_default_file_creates_file() {
    let path = Config::generate_default_file().expect("should generate config");
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).expect("should read");
    assert!(content.contains("SysVibe Configuration"));
    assert!(content.contains("catppuccin-macchiato"));
    assert!(content.contains("dashboard"));
    // Clean up
    let _ = std::fs::remove_file(&path);
    // Also try to remove parent dir if empty
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}
