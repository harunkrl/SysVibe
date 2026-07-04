//! Vitalis — Tests for the unified AppError type.

use std::error::Error;
use vitalis::app::error::{AppError, AppResult};

// ── Construction tests ──────────────────────────────────────────

#[test]
fn app_error_command_constructor() {
    let err = AppError::command("kill", "permission denied");
    let msg = err.to_string();
    assert!(msg.contains("kill"), "should mention command name");
    assert!(msg.contains("permission denied"), "should include reason");
}

#[test]
fn app_error_export_constructor() {
    let err = AppError::export("disk full");
    let msg = err.to_string();
    assert!(msg.contains("Export error"), "should have Export prefix");
    assert!(msg.contains("disk full"));
}

#[test]
fn app_error_config_constructor() {
    let err = AppError::config("missing field");
    assert!(err.to_string().contains("Config error"));
}

#[test]
fn app_error_data_constructor() {
    let err = AppError::data("malformed sysfs");
    assert!(err.to_string().contains("Data error"));
}

// ── From conversions ────────────────────────────────────────────

#[test]
fn app_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let app_err: AppError = io_err.into();
    assert!(app_err.to_string().contains("I/O error"));
    assert!(app_err.to_string().contains("file missing"));
}

#[test]
fn app_error_from_serde_json_error() {
    let json_str = "{invalid json";
    let result: Result<serde_json::Value, _> = serde_json::from_str(json_str);
    let app_err: AppError = result.unwrap_err().into();
    assert!(app_err.to_string().contains("JSON error"));
}

// ── AppResult usage ─────────────────────────────────────────────

#[test]
fn app_result_ok_branch() {
    fn succeeds() -> AppResult<u32> {
        Ok(42)
    }
    assert_eq!(succeeds().unwrap(), 42);
}

#[test]
fn app_result_err_branch() {
    fn fails() -> AppResult<u32> {
        Err(AppError::data("bad value"))
    }
    assert!(fails().is_err());
}

#[test]
fn app_error_chain_with_source() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
    let app_err = AppError::Io(io_err);
    assert!(
        app_err.source().is_some(),
        "Io variant should have a source"
    );
}

#[test]
fn app_error_display_all_variants() {
    // Ensure no panic in Display for any variant
    let errors: Vec<AppError> = vec![
        AppError::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe")),
        AppError::Command {
            command: "test",
            reason: "failed".into(),
        },
        AppError::Export("exp".into()),
        AppError::Config("cfg".into()),
        AppError::Data("dat".into()),
    ];
    for err in &errors {
        let _ = format!("{err}");
    }
}
