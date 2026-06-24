//! SysVibe — Nerd Font icon constants with ASCII fallbacks.
//!
//! All icons are defined here so they can be swapped for ASCII/emoji
//! alternatives when a Nerd Font is not detected.

#![allow(dead_code)]

// ═══════════════════════════════════════════════════════════════════════
// Nerd Font icon constants (Powerline Extra Symbols / Codicon sets)
// ═══════════════════════════════════════════════════════════════════════

/// OS icons
pub const OS_LINUX: &str = "\u{F17C}";       // 
pub const OS_ARCH: &str = "\u{F303}";         // 
pub const OS_UBUNTU: &str = "\u{F31B}";       // 
pub const OS_DEBIAN: &str = "\u{F306}";       // 
pub const OS_FEDORA: &str = "\u{F30A}";       // 
pub const OS_WINDOWS: &str = "\u{F17A}";      // 
pub const OS_MACOS: &str = "\u{F179}";        // 
pub const OS_GENERIC: &str = "\u{F233}";      //  (server/computer)

/// Hardware icons
pub const CPU: &str = "\u{F4B9}";             // 
pub const RAM: &str = "\u{EFC6}";             // 
pub const TEMP: &str = "\u{F2C9}";            // 
pub const DISK: &str = "\u{F0A0}";              // 
pub const DISK_ICON: &str = "\u{F0A0}";       // 
pub const NETWORK: &str = "\u{F0380}";         // 󰈀
pub const BATTERY: &str = "\u{F240}";          // 
pub const GPU: &str = "\u{F878}";             // 
pub const FAN: &str = "\u{F9AD}";             // 
pub const CHIP: &str = "\u{F2DB}";            // 

/// Status icons
pub const ARROW_UP: &str = "\u{F062}";        // 
pub const ARROW_DOWN: &str = "\u{F063}";      // 
pub const CHECK: &str = "\u{F00C}";           // 
pub const CROSS: &str = "\u{F00D}";           // 
pub const WARNING: &str = "\u{F071}";         // 
pub const INFO: &str = "\u{F05A}";            // 
pub const SEARCH: &str = "\u{F002}";          // 
pub const GEAR: &str = "\u{F013}";            // 
pub const SORT: &str = "\u{F0DC}";            // 
pub const SORT_UP: &str = "\u{F0DE}";         // 
pub const SORT_DOWN: &str = "\u{F0DD}";       // 

/// UI chrome
pub const INDICATOR: &str = "\u{258C}";       // ▌ block
pub const SEPARATOR: &str = "\u{E0B1}";       // 
pub const SEPARATOR_BOLD: &str = "\u{E0B0}";  // 
pub const ELLIPSIS: &str = "\u{F141}";        // 
pub const BULLET: &str = "\u{F111}";          // 
pub const DIAMOND: &str = "\u{F0C6}";        // 

/// Tab icons
pub const TAB_SYSTEM: &str = "\u{F233}";      //  (server)
pub const TAB_HARDWARE: &str = "\u{F2DB}";    //  (chip)
pub const TAB_PROCESSES: &str = "\u{F085}";   //  (gears)
pub const TAB_LOGS: &str = "\u{F15C}";        //  (file-text)
pub const TAB_DASHBOARD: &str = "\u{F108}";   //  (dashboard)

/// Process icons
pub const PROCESS: &str = "\u{F120}";         //  (terminal)
pub const PROCESS_RUNNING: &str = "\u{F04B}"; //  (play)
pub const PROCESS_SLEEPING: &str = "\u{F04C}";//  (pause)
pub const PROCESS_ZOMBIE: &str = "\u{F7E4}";  //  (skull)

/// Log level icons
pub const LOG_ERROR: &str = "\u{F057}";       // 
pub const LOG_WARN: &str = "\u{F071}";        // 
pub const LOG_INFO: &str = "\u{F05A}";        // 
pub const LOG_DEBUG: &str = "\u{F188}";       // 
pub const LOG_TRACE: &str = "\u{F0AC}";       // 

/// Disk/network detail icons
pub const DISK_IO_READ: &str = "\u{F0AB}";    // 
pub const DISK_IO_WRITE: &str = "\u{F0AA}";   // 
pub const NET_UPLOAD: &str = "\u{F062}";       // 
pub const NET_DOWNLOAD: &str = "\u{F063}";     // 

// ═══════════════════════════════════════════════════════════════════════
// ASCII fallbacks (used when Nerd Font is not available)
// ═══════════════════════════════════════════════════════════════════════

pub mod fallback {
    pub const OS_LINUX: &str = "▦";
    pub const OS_GENERIC: &str = "▦";
    pub const CPU: &str = "⬡";
    pub const RAM: &str = "▧";
    pub const TEMP: &str = "⬢";
    pub const DISK: &str = "◉";
    pub const NETWORK: &str = "⇅";
    pub const BATTERY: &str = "⚡";
    pub const GPU: &str = "◈";
    pub const FAN: &str = "⚙";
    pub const SEARCH: &str = "◎";
    pub const ARROW_UP: &str = "▲";
    pub const ARROW_DOWN: &str = "▼";
    pub const CHECK: &str = "✓";
    pub const CROSS: &str = "✗";
    pub const WARNING: &str = "⚠";
    pub const TAB_SYSTEM: &str = "[Sys]";
    pub const TAB_HARDWARE: &str = "[Hw]";
    pub const TAB_PROCESSES: &str = "[Proc]";
    pub const TAB_LOGS: &str = "[Log]";
    pub const TAB_DASHBOARD: &str = "[Dash]";
    pub const PROCESS_RUNNING: &str = "▶";
    pub const PROCESS_SLEEPING: &str = "⏸";
    pub const LOG_ERROR: &str = "✖";
    pub const LOG_WARN: &str = "⚠";
    pub const LOG_INFO: &str = "ℹ";
}

// ═══════════════════════════════════════════════════════════════════════
// Convenience: icon selection based on Nerd Font availability
// ═══════════════════════════════════════════════════════════════════════

use crate::app::App;

/// Resolve an OS icon for the current system.
/// Uses Nerd Font if enabled in app config, else fallback.
pub fn os_icon(app: &App) -> &'static str {
    if app.config().nerd_fonts {
        OS_LINUX
    } else {
        fallback::OS_LINUX
    }
}

/// Create an icon prefix string: " {icon} " or " " when NF disabled.
pub fn icon(app: &App, nf_icon: &str, fb_icon: &str) -> String {
    if app.config().nerd_fonts {
        format!(" {} ", nf_icon)
    } else {
        format!(" {} ", fb_icon)
    }
}

/// Create a titled panel icon: " {icon} {title} " with NF support.
pub fn titled(app: &App, nf_icon: &str, _fb_icon: &str, title: &str) -> String {
    if app.config().nerd_fonts {
        format!(" {} {} ", nf_icon, title)
    } else {
        format!(" {} ", title)
    }
}
