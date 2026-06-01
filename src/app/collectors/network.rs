//! SysVibe — Network I/O data collection.
//!
//! Computes per-interface receive/transmit speeds, cumulative session
//! totals, local IP resolution, and maintains rolling history buffers
//! for sparkline rendering.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::UdpSocket;
use sysinfo::Networks;
use super::super::helpers::push_history;
use super::super::state::{NetworkStats, HISTORY_LEN};

/// Resolve the local IPv4 address by briefly opening a UDP socket.
pub fn resolve_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// Refresh network interface speeds and history.
///
/// For every non-loopback interface, computes the byte-rate delta since the
/// previous sample, converts to KB/s, and appends to the per-interface
/// history ring buffer. Also tracks cumulative session totals.
pub fn refresh_network(
    networks: &Networks,
    prev_bytes: &mut HashMap<String, (u64, u64)>,
    stats: &mut Vec<NetworkStats>,
    elapsed: f64,
    local_ip: &Option<String>,
) {
    let mut new_stats = Vec::with_capacity(stats.len());

    for (name, nd) in networks.list() {
        if name == "lo" {
            continue;
        }

        let cur_rx = nd.received();
        let cur_tx = nd.transmitted();
        let (prev_rx, prev_tx) = prev_bytes
            .get(name)
            .copied()
            .unwrap_or((cur_rx, cur_tx));

        let rx_speed_bps = cur_rx.saturating_sub(prev_rx) as f64 / elapsed;
        let tx_speed_bps = cur_tx.saturating_sub(prev_tx) as f64 / elapsed;
        let rx_kbs = (rx_speed_bps / 1024.0) as u64;
        let tx_kbs = (tx_speed_bps / 1024.0) as u64;

        // Cumulative session totals: delta since last tick
        let delta_rx = cur_rx.saturating_sub(prev_rx);
        let delta_tx = cur_tx.saturating_sub(prev_tx);

        // Remove matching entry from stats to update it in-place
        let idx = stats.iter().position(|s| s.interface == *name);
        if let Some(idx) = idx {
            let mut existing = stats.swap_remove(idx);
            existing.rx_speed_bps = rx_speed_bps;
            existing.tx_speed_bps = tx_speed_bps;
            push_history(&mut existing.rx_history, rx_kbs);
            push_history(&mut existing.tx_history, tx_kbs);
            existing.total_rx_bytes += delta_rx;
            existing.total_tx_bytes += delta_tx;
            existing.local_ip = local_ip.clone();
            new_stats.push(existing);
        } else {
            let mut rx_hist = VecDeque::with_capacity(HISTORY_LEN);
            let mut tx_hist = VecDeque::with_capacity(HISTORY_LEN);
            rx_hist.push_back(rx_kbs);
            tx_hist.push_back(tx_kbs);
            new_stats.push(NetworkStats {
                interface: name.clone(),
                rx_speed_bps,
                tx_speed_bps,
                rx_history: rx_hist,
                tx_history: tx_hist,
                total_rx_bytes: delta_rx,
                total_tx_bytes: delta_tx,
                local_ip: local_ip.clone(),
            });
        }

        prev_bytes.insert(name.clone(), (cur_rx, cur_tx));
    }

    *stats = new_stats;
}
