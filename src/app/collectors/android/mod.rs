//! Vitalis — Android (Termux) platform-specific collectors.
//!
//! Each sub-module gathers data using Android-specific APIs and tools,
//! with a 3-layer fallback strategy:
//!   1. Normal (rootless) access
//!   2. `su -c <cmd>` (Magisk root)
//!   3. Graceful fallback (N/A, empty, or zeroed data)

pub mod battery;
pub mod disk;
pub mod gpu;
pub mod hardware;
pub mod logs;
