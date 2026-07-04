//! Vitalis — Data collection modules.
//!
//! Each sub-module gathers a specific category of system telemetry
//! (CPU, network, disk, sensors, kernel logs).

pub mod cpu;
pub mod export;
pub mod network;
pub mod sensors;

#[cfg(not(target_os = "android"))]
pub mod linux;
#[cfg(not(target_os = "android"))]
pub use linux::*;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android::*;
