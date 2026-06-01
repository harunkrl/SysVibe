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
