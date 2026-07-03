//! SysVibe — Application helper utilities.
//!
//! Common utility functions shared across the app module,
//! primarily for managing fixed-length history buffers.

use super::state::HISTORY_LEN;
use std::collections::VecDeque;

/// Push a value into a fixed-length history buffer, evicting the oldest entry.
pub fn push_history(buf: &mut VecDeque<u64>, val: u64) {
    buf.push_back(val);
    if buf.len() > HISTORY_LEN {
        buf.pop_front();
    }
}

/// "Nice number" ceiling: round `v` up to the next 1 / 2 / 5 × 10ⁿ. Produces
/// readable, stable scale boundaries for arbitrary-magnitude data like network
/// speeds: 1234 → 2000, 876 → 1000, 4500 → 5000, 70 → 100, 0 → 1. Combined
/// with a sticky decay on the caller side, the scale only crosses a boundary
/// on meaningful peaks, killing vertical "breathing" jitter.
pub fn nice_number_ceiling(v: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let mag = 10_f64.powi(v.log10().floor() as i32);
    let norm = v / mag; // 1.0..10.0
    let nice_norm = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice_norm * mag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_history_basic() {
        let mut buf = VecDeque::new();
        push_history(&mut buf, 10);
        push_history(&mut buf, 20);
        push_history(&mut buf, 30);
        assert_eq!(buf.len(), 3);
        assert_eq!(*buf.front().unwrap(), 10);
        assert_eq!(*buf.back().unwrap(), 30);
    }

    #[test]
    fn test_push_history_evicts_oldest() {
        let mut buf = VecDeque::new();
        // Fill to capacity + 1
        for i in 0..=(HISTORY_LEN) {
            push_history(&mut buf, i as u64);
        }
        assert_eq!(buf.len(), HISTORY_LEN);
        // Oldest (0) should have been evicted
        assert_eq!(*buf.front().unwrap(), 1);
        assert_eq!(*buf.back().unwrap(), HISTORY_LEN as u64);
    }

    #[test]
    fn test_push_history_exact_capacity() {
        let mut buf = VecDeque::new();
        for i in 0..HISTORY_LEN {
            push_history(&mut buf, i as u64);
        }
        assert_eq!(buf.len(), HISTORY_LEN);
        assert_eq!(*buf.front().unwrap(), 0);
        assert_eq!(*buf.back().unwrap(), (HISTORY_LEN - 1) as u64);
    }

    #[test]
    fn nice_number_rounds_up_to_1_2_5_step() {
        // boundary: 1 / 2 / 5 / 10 × 10ⁿ
        assert_eq!(nice_number_ceiling(1.0), 1.0);
        assert_eq!(nice_number_ceiling(1.5), 2.0);
        assert_eq!(nice_number_ceiling(2.0), 2.0);
        assert_eq!(nice_number_ceiling(3.0), 5.0);
        assert_eq!(nice_number_ceiling(5.0), 5.0);
        assert_eq!(nice_number_ceiling(7.0), 10.0);
        assert_eq!(nice_number_ceiling(10.0), 10.0);
        // magnitude
        assert_eq!(nice_number_ceiling(70.0), 100.0);
        assert_eq!(nice_number_ceiling(876.0), 1000.0);
        assert_eq!(nice_number_ceiling(1234.0), 2000.0);
        assert_eq!(nice_number_ceiling(4500.0), 5000.0);
        // edges
        assert_eq!(nice_number_ceiling(0.0), 1.0);
        assert_eq!(nice_number_ceiling(-5.0), 1.0);
    }
}
