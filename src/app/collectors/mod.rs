//! SysVibe — Data collection modules.
//!
//! Each sub-module gathers a specific category of system telemetry
//! (CPU, network, disk, sensors, kernel logs).

pub mod cpu;
pub mod disk;
pub mod logs;
pub mod network;
pub mod hardware;
pub mod sensors;
pub mod gpu;
