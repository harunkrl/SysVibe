//! Vitalis — App::tick — Lightweight per-tick update (UI animations, status expiry).
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
    pub fn on_tick(&mut self) {
        self.tick_count += 1;
        if let Some(ref msg) = self.status_message
            && Instant::now() >= msg.expires
        {
            self.status_message = None;
        }
        self.maybe_refresh_system_info();
        // Retry public IP resolution every ~20 ticks if still unresolved
        if self.tick_count.is_multiple_of(20) {
            self.spawn_public_ip_resolve();
        }
        // Check alert thresholds every ~4 ticks (~1s)
        if self.tick_count.is_multiple_of(4) {
            self.check_alerts();
        }
    }

    /// Check configured alert thresholds against current metric values.
    fn check_alerts(&mut self) {
        let mut alerts = Vec::new();

        // CPU alert
        if let Some(threshold) = self.config.cpu_alert_threshold {
            let cpu_pct = self.cpu_history.back().copied().unwrap_or(0) as f32;
            if cpu_pct >= threshold {
                alerts.push(format!("\u{26a0} CPU {:.0}% >= {:.0}%", cpu_pct, threshold));
            }
        }

        // Memory alert
        if let Some(threshold) = self.config.memory_alert_threshold {
            let ram_total = self.cached_ram_total as f64;
            if ram_total > 0.0 {
                let mem_pct = (self.cached_ram_used as f64 / ram_total * 100.0) as f32;
                if mem_pct >= threshold {
                    alerts.push(format!("\u{26a0} RAM {:.0}% >= {:.0}%", mem_pct, threshold));
                }
            }
        }

        // Temperature alert (max sensor)
        if let Some(threshold) = self.config.temperature_alert_threshold
            && let Some(max_temp) = self.temperatures.iter().map(|s| s.temp_c).reduce(f32::max)
            && max_temp >= threshold
        {
            alerts.push(format!(
                "\u{26a0} Temp {:.0}°C >= {:.0}°C",
                max_temp, threshold
            ));
        }

        // Disk usage alert (max partition usage)
        if let Some(threshold) = self.config.disk_alert_threshold
            && let Some(max_usage) = self
                .cached_partitions
                .iter()
                .map(|p| {
                    if p.total_bytes > 0 {
                        p.used_bytes as f32 / p.total_bytes as f32 * 100.0
                    } else {
                        0.0
                    }
                })
                .reduce(f32::max)
            && max_usage >= threshold
        {
            alerts.push(format!(
                "\u{26a0} Disk {:.0}% >= {:.0}%",
                max_usage, threshold
            ));
        }

        self.active_alerts = alerts;
    }

    /// Return the current list of active alert messages.
    pub fn active_alerts(&self) -> &[String] {
        &self.active_alerts
    }
}
