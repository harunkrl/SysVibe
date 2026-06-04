//! SysVibe — Application helper utilities.
//!
//! Common utility functions shared across the app module,
//! primarily for managing fixed-length history buffers.

use std::collections::VecDeque;
use super::state::HISTORY_LEN;

/// Push a value into a fixed-length history buffer, evicting the oldest entry.
pub fn push_history(buf: &mut VecDeque<u64>, val: u64) {
    buf.push_back(val);
    if buf.len() > HISTORY_LEN {
        buf.pop_front();
    }
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
}
