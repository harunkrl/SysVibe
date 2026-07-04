//! Vitalis — Unified error types.
//!
//! Replaces ad-hoc `Result<T, String>` and `Box<dyn Error>` with a single,
//! well-typed error enum. All public APIs that can fail should return
//! `Result<T, AppError>`.

/// Unified error type for all Vitalis operations.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// An I/O error (file not found, permission denied, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A system command (`kill`, `nvidia-smi`, etc.) failed.
    #[error("Command '{command}' failed: {reason}")]
    Command {
        command: &'static str,
        reason: String,
    },

    /// Export-related errors (serialization, directory creation, etc.).
    #[error("Export error: {0}")]
    Export(String),

    /// Configuration errors (invalid config file, missing fields, etc.).
    #[error("Config error: {0}")]
    Config(String),

    /// Data collection / parsing errors (e.g., malformed sysfs output).
    #[error("Data error: {0}")]
    Data(String),
}

impl AppError {
    /// Create a command error from a failed process.
    pub fn command(cmd: &'static str, reason: impl Into<String>) -> Self {
        Self::Command {
            command: cmd,
            reason: reason.into(),
        }
    }

    /// Create an export error.
    pub fn export(msg: impl Into<String>) -> Self {
        Self::Export(msg.into())
    }

    /// Create a config error.
    #[allow(dead_code)] // intentionally retained as public API (no call sites yet)
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a data parsing error.
    #[allow(dead_code)] // intentionally retained as public API (no call sites yet)
    pub fn data(msg: impl Into<String>) -> Self {
        Self::Data(msg.into())
    }
}

/// Convenience alias used across the codebase.
pub type AppResult<T> = Result<T, AppError>;

// ── Conversion helpers for external crate errors ────────────────

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Export(format!("JSON error: {e}"))
    }
}

impl From<toml::de::Error> for AppError {
    fn from(e: toml::de::Error) -> Self {
        Self::Config(format!("TOML parse error: {e}"))
    }
}
