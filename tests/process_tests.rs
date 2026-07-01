//! Tests for process management functions.

use sysinfo::System;
use sysvibe::app::processes::{build_process_list, kill_process, kill_process_force};
use sysvibe::app::state::SortBy;

fn create_test_system() -> System {
    let mut sys = System::new_all();
    sys.refresh_all();
    sys
}

#[test]
fn build_process_list_returns_entries() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 20, true);
    assert!(!procs.is_empty(), "Should find at least some processes");
}

#[test]
fn build_process_list_respects_max() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 5, true);
    assert!(procs.len() <= 5, "Should not exceed max_processes");
}

#[test]
fn build_process_list_normalized_cpu() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 20, true);
    for p in &procs {
        // Normalized: should be <= 100% per core (allow small float tolerance)
        assert!(
            p.cpu_pct <= 101.0,
            "Normalized CPU {} should be <= 100% (got {}%)",
            p.name,
            p.cpu_pct,
        );
    }
}

#[test]
fn build_process_list_per_core_cpu() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 20, false);
    // Per-core: can be > 100% on multi-core systems
    // Just verify entries exist
    assert!(!procs.is_empty());
}

#[test]
fn build_process_list_sorted_by_cpu() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 20, true);
    for window in procs.windows(2) {
        assert!(
            window[0].cpu_pct >= window[1].cpu_pct,
            "Processes should be sorted by CPU desc: {}% >= {}%",
            window[0].cpu_pct,
            window[1].cpu_pct,
        );
    }
}

#[test]
fn build_process_list_sorted_by_mem() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Mem, 20, true);
    for window in procs.windows(2) {
        assert!(
            window[0].mem_pct >= window[1].mem_pct,
            "Processes should be sorted by MEM desc: {}% >= {}%",
            window[0].mem_pct,
            window[1].mem_pct,
        );
    }
}

#[test]
fn build_process_list_sorted_by_pid() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Pid, 20, true);
    for window in procs.windows(2) {
        assert!(
            window[0].pid <= window[1].pid,
            "Processes should be sorted by PID asc: {} <= {}",
            window[0].pid,
            window[1].pid,
        );
    }
}

#[test]
fn build_process_list_sorted_by_name() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Name, 20, true);
    // Just verify the list is non-empty and has valid names
    for p in &procs {
        assert!(!p.name.is_empty(), "Process name should not be empty");
    }
}

#[test]
fn build_process_list_has_parent_pid() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 20, true);
    for p in &procs {
        // Parent PID should be a valid PID or 0 (for init/kernel processes)
        // It doesn't have to be in the list, just a valid u32
        assert!(
            p.parent_pid > 0 || p.pid <= 10, // PID 1 (init) has parent 0
            "Process {} (PID {}) should have parent_pid > 0 or be an early process",
            p.name,
            p.pid,
        );
    }
}

#[test]
fn build_process_list_mem_pct_non_negative() {
    let sys = create_test_system();
    let procs = build_process_list(&sys, &SortBy::Cpu, 20, true);
    for p in &procs {
        assert!(
            p.mem_pct >= 0.0,
            "Memory percentage should be non-negative, got {}% for {}",
            p.mem_pct,
            p.name,
        );
    }
}

#[test]
fn kill_nonexistent_process_fails() {
    // PID 99999999 should not exist
    let result = kill_process(99999999);
    assert!(result.is_err());
}

#[test]
fn kill_force_nonexistent_process_fails() {
    let result = kill_process_force(99999999);
    assert!(result.is_err());
}
