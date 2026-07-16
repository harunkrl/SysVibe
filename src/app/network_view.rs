//! Vitalis — App::network_view — network stats + public IP + the sticky graph
//! scale, extracted from `App`. Owns the per-interface counters, the lazily-
//! resolved public IP, and the smoothed graph ceiling recomputed on each live
//! sample.

use std::sync::{Arc, Mutex};

use super::*;

/// Network view state: per-interface counters, the lazily-resolved public IP,
/// and the sticky (smoothed) graph ceiling.
pub struct NetworkView {
    pub(crate) stats: Vec<NetworkStats>,
    pub(crate) public_ip: Arc<Mutex<Option<String>>>,
    pub(crate) visible_scale: f64,
}

impl NetworkView {
    pub fn new() -> Self {
        Self {
            stats: Vec::new(),
            public_ip: Arc::new(Mutex::new(None)),
            // ~1 MB/s floor until the first live sample arrives and set_stats
            // recomputes the sticky ceiling.
            visible_scale: 1000.0,
        }
    }

    /// Apply a new network sample and recompute the sticky graph ceiling.
    ///
    /// target = nice-numbered raw peak (with a ~1 MB/s floor), then keep the
    /// max of target and a slow decay of the previous visible value. The scale
    /// rises instantly with real peaks but sinks gradually (~8%/tick), so the
    /// mirrored graph stops "breathing" as traffic wavers while still tracking
    /// it over the session. This is the live entry point for network data (the
    /// background fast collector calls here ~1 Hz).
    pub fn set_stats(&mut self, stats: Vec<NetworkStats>) {
        self.stats = stats;
        const NET_FLOOR_KIB: f64 = 1000.0;
        const DECAY: f64 = 0.92;
        let raw_peak = self
            .stats
            .iter()
            .flat_map(|s| s.rx_history.iter().chain(s.tx_history.iter()))
            .copied()
            .map(|v| v as f64)
            .fold(0.0_f64, f64::max);
        let target = helpers::nice_number_ceiling(raw_peak.max(NET_FLOOR_KIB));
        self.visible_scale = target.max(self.visible_scale * DECAY).max(1.0);
    }

    pub fn stats(&self) -> &[NetworkStats] {
        &self.stats
    }

    pub fn visible_scale(&self) -> f64 {
        self.visible_scale
    }

    pub fn public_ip(&self) -> Option<String> {
        self.public_ip.lock().ok().and_then(|g| g.clone())
    }

    /// Spawn a background thread to resolve the public IP (if not already
    /// resolved and the feature is enabled).
    pub fn spawn_ip_resolve(&self, resolve_enabled: bool) {
        if !resolve_enabled {
            return;
        }
        let already = self
            .public_ip
            .lock()
            .ok()
            .map(|g| g.is_some())
            .unwrap_or(false);
        if already {
            return;
        }
        let shared = Arc::clone(&self.public_ip);
        std::thread::spawn(move || {
            let ip = collectors::network::resolve_public_ip();
            if let Ok(mut guard) = shared.lock() {
                *guard = ip;
            }
        });
    }
}
