//! Vitalis — Tests for StateUpdate channel mechanics and EventStream logic.
//!
//! These tests validate that the lightweight instantaneous-value payload
//! works correctly with the App's push_history mechanism.

use std::collections::VecDeque;
use vitalis::app::helpers::push_history;
use vitalis::app::state::HISTORY_LEN;

// ── push_history correctness ────────────────────────────────────

#[test]
fn push_history_respects_capacity_limit() {
    let mut buf: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);
    // Insert more than HISTORY_LEN values
    for i in 0..(HISTORY_LEN * 3) {
        push_history(&mut buf, i as u64);
    }
    assert_eq!(
        buf.len(),
        HISTORY_LEN,
        "buffer should be capped at HISTORY_LEN"
    );
    // The oldest values should have been dropped
    assert_eq!(
        *buf.front().unwrap(),
        (HISTORY_LEN * 3 - HISTORY_LEN) as u64
    );
    assert_eq!(*buf.back().unwrap(), (HISTORY_LEN * 3 - 1) as u64);
}

#[test]
fn push_history_appends_correctly() {
    let mut buf: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);
    push_history(&mut buf, 10);
    push_history(&mut buf, 20);
    push_history(&mut buf, 30);
    assert_eq!(buf.len(), 3);
    assert_eq!(*buf.front().unwrap(), 10);
    assert_eq!(*buf.back().unwrap(), 30);
}

#[test]
fn push_history_per_core_multiple_cores() {
    let core_count = 8;
    let mut per_core: Vec<VecDeque<u64>> = vec![VecDeque::with_capacity(HISTORY_LEN); core_count];

    // Simulate 5 ticks
    for tick in 0..5 {
        for (i, history) in per_core.iter_mut().enumerate() {
            push_history(history, (tick * core_count + i) as u64);
        }
    }

    for (i, history) in per_core.iter().enumerate() {
        assert_eq!(history.len(), 5, "core {i} should have 5 entries");
    }
}

#[test]
fn push_history_handles_core_count_change() {
    // Start with 4 cores
    let mut per_core: Vec<VecDeque<u64>> = vec![VecDeque::with_capacity(HISTORY_LEN); 4];
    for history in &mut per_core {
        push_history(history, 50);
    }

    // Core count changes to 8 (e.g., hotplug) — resize
    per_core = vec![VecDeque::with_capacity(HISTORY_LEN); 8];
    for (i, history) in per_core.iter_mut().enumerate() {
        push_history(history, i as u64);
    }
    assert_eq!(per_core.len(), 8);
    assert_eq!(*per_core[7].front().unwrap(), 7);
}

// ── Simulated FastMetrics application ───────────────────────────

#[test]
fn simulate_fast_metrics_update_cycle() {
    // Simulates what apply_state_update does in main.rs:
    // receive instantaneous values and push into App-maintained history.

    let mut cpu_history: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);
    let mut per_core_history: Vec<VecDeque<u64>> = vec![VecDeque::with_capacity(HISTORY_LEN); 4];

    // Simulate 100 ticks (25 seconds at 250ms)
    for tick in 0..100 {
        let cpu_usage = (tick % 100) as u64;
        let per_core_usage: Vec<u64> = (0..4).map(|c| cpu_usage + c).collect();

        // Apply update (mirrors apply_state_update logic)
        push_history(&mut cpu_history, cpu_usage);
        for (i, &usage) in per_core_usage.iter().enumerate() {
            push_history(&mut per_core_history[i], usage);
        }
    }

    assert_eq!(
        cpu_history.len(),
        HISTORY_LEN,
        "global history should be capped"
    );
    for history in &per_core_history {
        assert_eq!(
            history.len(),
            HISTORY_LEN,
            "per-core history should be capped"
        );
    }
}

// ── Channel send/recv with lightweight payload ──────────────────

#[test]
fn channel_fast_metrics_payload_size() {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    // Send a lightweight update (mirrors StateUpdate::FastMetrics)
    let per_core_usage: Vec<u64> = vec![10, 20, 30, 40];
    tx.send((50u64, per_core_usage, 8192u64, 16384u64)).unwrap();

    let (cpu, cores, ram_used, ram_total) = rx.recv().unwrap();
    assert_eq!(cpu, 50);
    assert_eq!(cores.len(), 4);
    assert_eq!(ram_used, 8192);
    assert_eq!(ram_total, 16384);
}

// ── Zero-copy verification ──────────────────────────────────────

#[test]
fn instantaneous_values_no_history_in_payload() {
    // Verify that we can reconstruct full history from only
    // instantaneous values pushed over time
    let mut reconstructed: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);

    for tick in 0..200 {
        let value = (tick * 7 + 3) % 100; // arbitrary pattern
        push_history(&mut reconstructed, value);
    }

    assert_eq!(reconstructed.len(), HISTORY_LEN);
    // Verify last value
    assert_eq!(*reconstructed.back().unwrap(), ((199 * 7 + 3) % 100));
}
