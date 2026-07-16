//! Vitalis — App::gpu_view — GPU stats + per-GPU usage history + scroll,
//! extracted from `App`. Owns the live per-GPU snapshot, the per-id usage
//! trend map (with a zero-length fallback), and the multi-GPU scroll offset.
//!
//! The history field is named `history` here (not `gpu_usage_history`) to
//! resolve the old field-vs-method name collision; `App::gpu_usage_history(id)`
//! survives as a delegator for UI/test compatibility.

use std::collections::{HashMap, VecDeque};

use super::*;

/// GPU view state: live stats, per-GPU usage history, and scroll offset.
pub struct GpuView {
    pub(crate) stats: Vec<GpuStats>,
    pub(crate) history: HashMap<String, VecDeque<u64>>,
    pub(crate) history_empty: VecDeque<u64>,
    pub(crate) scroll: usize,
}

impl GpuView {
    pub fn new() -> Self {
        Self {
            stats: Vec::new(),
            history: HashMap::new(),
            history_empty: VecDeque::new(),
            scroll: 0,
        }
    }

    /// Apply a new GPU stats snapshot. NVIDIA/Unknown have no cheap per-tick
    /// usage source, so their per-GPU trend advances here at the 5 s sensor
    /// cadence (AMD/Intel advance via [`GpuView::push_samples`] at ~1 Hz).
    pub fn set_stats(&mut self, stats: Vec<GpuStats>) {
        for g in &stats {
            if matches!(g.vendor, GpuVendor::Nvidia | GpuVendor::Unknown) {
                let h = self
                    .history
                    .entry(g.id.clone())
                    .or_insert_with(|| VecDeque::with_capacity(HISTORY_LEN));
                helpers::push_history(h, g.usage_pct.round() as u64);
            }
        }
        self.stats = stats;
    }

    /// Push fast-tier (AMD/Intel) GPU usage samples into the per-GPU history.
    pub fn push_samples(&mut self, samples: Vec<(String, u64)>) {
        for (id, usage) in samples {
            let h = self
                .history
                .entry(id)
                .or_insert_with(|| VecDeque::with_capacity(HISTORY_LEN));
            helpers::push_history(h, usage);
        }
    }

    /// Primary-GPU usage history (Dashboard trend): the focused/primary GPU's
    /// buffer, falling back to the empty buffer when no GPU is present or the
    /// primary hasn't been sampled yet.
    pub fn primary_history(&self) -> &VecDeque<u64> {
        match self.stats.first() {
            Some(g) => self.history.get(&g.id).unwrap_or(&self.history_empty),
            None => &self.history_empty,
        }
    }

    /// Per-GPU usage history for the GPU tab's per-card braille trend.
    pub fn history_for(&self, id: &str) -> &VecDeque<u64> {
        self.history.get(id).unwrap_or(&self.history_empty)
    }

    /// Clear all per-GPU history (preview-test helper).
    #[cfg(all(test, feature = "preview"))]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn stats(&self) -> &[GpuStats] {
        &self.stats
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn scroll_down(&mut self) {
        let max = self.stats.len().saturating_sub(1);
        if self.scroll < max {
            self.scroll += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }
}
