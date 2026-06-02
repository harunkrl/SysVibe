//! Tests for application state types and enums.

use sysvibe::app::state::{
    AppTab, AppMode, PanelFocus, SortBy, LogLevelFilter, LogLevel,
};

// ═══════════════════════════════════════════════════════════════════
// AppTab
// ═══════════════════════════════════════════════════════════════════

#[test]
fn default_tab_is_dashboard() {
    assert_eq!(AppTab::default(), AppTab::Dashboard);
}

#[test]
fn all_tabs_are_distinct() {
    let tabs = [AppTab::Dashboard, AppTab::System, AppTab::Hardware, AppTab::Processes, AppTab::Logs];
    for i in 0..tabs.len() {
        for j in (i + 1)..tabs.len() {
            assert_ne!(tabs[i], tabs[j], "All tabs should be distinct");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// PanelFocus
// ═══════════════════════════════════════════════════════════════════

#[test]
fn panel_focus_default() {
    assert_eq!(PanelFocus::default(), PanelFocus::Panel1);
}

#[test]
fn panel_focus_cycle_forward() {
    let p = PanelFocus::Panel1;
    assert_eq!(p.next(), PanelFocus::Panel2);
    assert_eq!(p.next().next(), PanelFocus::Panel3);
    assert_eq!(p.next().next().next(), PanelFocus::Panel4);
    assert_eq!(p.next().next().next().next(), PanelFocus::Panel5);
    assert_eq!(p.next().next().next().next().next(), PanelFocus::Panel6);
    // Wraps around
    assert_eq!(
        PanelFocus::Panel1.next().next().next().next().next().next(),
        PanelFocus::Panel1,
    );
}

#[test]
fn panel_focus_cycle_backward() {
    assert_eq!(PanelFocus::Panel1.prev(), PanelFocus::Panel6);
    assert_eq!(PanelFocus::Panel2.prev(), PanelFocus::Panel1);
    assert_eq!(PanelFocus::Panel6.prev(), PanelFocus::Panel5);
}

#[test]
fn panel_focus_roundtrip() {
    for _ in 0..100 {
        let mut p = PanelFocus::Panel1;
        p = p.next();
        p = p.prev();
        assert_eq!(p, PanelFocus::Panel1, "next then prev should return to start");
    }
}

#[test]
fn panel_focus_is_focused() {
    assert!(PanelFocus::Panel1.is_focused(PanelFocus::Panel1));
    assert!(!PanelFocus::Panel1.is_focused(PanelFocus::Panel2));
}

// ═══════════════════════════════════════════════════════════════════
// SortBy
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sort_by_default_is_cpu() {
    assert_eq!(SortBy::default(), SortBy::Cpu);
}

// ═══════════════════════════════════════════════════════════════════
// LogLevelFilter
// ═══════════════════════════════════════════════════════════════════

#[test]
fn log_level_filter_all_allows_everything() {
    let filter = LogLevelFilter::all();
    assert!(filter.allows(&LogLevel::Error));
    assert!(filter.allows(&LogLevel::Warning));
    assert!(filter.allows(&LogLevel::Info));
    assert!(filter.allows(&LogLevel::Debug));
    assert!(filter.allows(&LogLevel::Notice));
    assert!(filter.allows(&LogLevel::Unknown));
}

#[test]
fn log_level_filter_default_is_all() {
    let default = LogLevelFilter::default();
    let all = LogLevelFilter::all();
    assert!(default.allows(&LogLevel::Error) == all.allows(&LogLevel::Error));
    assert!(default.allows(&LogLevel::Warning) == all.allows(&LogLevel::Warning));
    assert!(default.allows(&LogLevel::Info) == all.allows(&LogLevel::Info));
}

#[test]
fn log_level_filter_toggles() {
    let mut filter = LogLevelFilter::all();
    assert!(filter.allows(&LogLevel::Error));
    filter.show_errors = false;
    assert!(!filter.allows(&LogLevel::Error));
    // Other levels should still pass
    assert!(filter.allows(&LogLevel::Warning));
    assert!(filter.allows(&LogLevel::Info));
}

#[test]
fn log_level_filter_all_disabled() {
    let filter = LogLevelFilter {
        show_errors: false,
        show_warnings: false,
        show_info: false,
        show_debug: false,
        show_notice: false,
        show_unknown: false,
    };
    assert!(!filter.allows(&LogLevel::Error));
    assert!(!filter.allows(&LogLevel::Warning));
    assert!(!filter.allows(&LogLevel::Info));
    assert!(!filter.allows(&LogLevel::Debug));
    assert!(!filter.allows(&LogLevel::Notice));
    assert!(!filter.allows(&LogLevel::Unknown));
}

// ═══════════════════════════════════════════════════════════════════
// AppMode
// ═══════════════════════════════════════════════════════════════════

#[test]
fn app_mode_default_is_normal() {
    assert_eq!(AppMode::default(), AppMode::Normal);
}
