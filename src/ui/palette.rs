//! SysVibe — Color palette (re-exported from theme system).
//!
//! For backwards compatibility, this module provides static color constants
//! matching the Catppuccin Macchiato theme (the default).
//!
//! For runtime theme support, use `crate::ui::theme::Theme` instead.
//! The `apply_theme()` function updates all palette constants via a
//! thread-local theme cache.

use ratatui::style::Color;
use std::cell::RefCell;

use super::theme::{ColorDef, Theme};

// Thread-local theme storage
thread_local! {
    static CURRENT_THEME: RefCell<Theme> = RefCell::new(Theme::catppuccin_macchiato());
}

/// Apply a theme as the current active palette.
/// Must be called before any rendering happens.
pub fn apply_theme(theme: Theme) {
    CURRENT_THEME.with(|t| *t.borrow_mut() = theme);
}

/// Load and apply a theme by name.
pub fn load_and_apply(name: &str) {
    let theme = Theme::load(name);
    apply_theme(theme);
}

// ═══════════════════════════════════════════════════════════════════════
// Runtime color accessors — read from the current theme
// ═══════════════════════════════════════════════════════════════════════

macro_rules! theme_color {
    ($name:ident, $field:ident) => {
        pub fn $name() -> Color {
            CURRENT_THEME.with(|t| t.borrow().$field.to_color())
        }
    };
}

theme_color!(rosewater, rosewater);
theme_color!(flamingo, flamingo);
theme_color!(pink, pink);
theme_color!(mauve, mauve);
theme_color!(red, red);
theme_color!(maroon, maroon);
theme_color!(peach, peach);
theme_color!(yellow, yellow);
theme_color!(green, green);
theme_color!(teal, teal);
theme_color!(sky, sky);
theme_color!(sapphire, sapphire);
theme_color!(blue, blue);
theme_color!(lavender, lavender);
theme_color!(text, text);
theme_color!(subtext, subtext);
theme_color!(overlay, overlay);
theme_color!(surface0, surface0);
theme_color!(surface1, surface1);
theme_color!(surface2, surface2);
theme_color!(base, base);
theme_color!(mantle, mantle);
theme_color!(crust, crust);
theme_color!(focus_border, focus_border);
theme_color!(focus_tab, focus_tab);

// ═══════════════════════════════════════════════════════════════════════
// Static constants for backwards compatibility (Catppuccin Macchiato)
// ═══════════════════════════════════════════════════════════════════════
// These are used at compile time. Runtime code should prefer the
// function accessors above for theme-aware colors.

pub const ROSEWATER: Color = Color::Rgb(244, 194, 219);
#[allow(dead_code)]
pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);
#[allow(dead_code)]
pub const PINK: Color = Color::Rgb(245, 189, 230);
pub const MAUVE: Color = Color::Rgb(198, 160, 246);
pub const RED: Color = Color::Rgb(237, 135, 150);
pub const MAROON: Color = Color::Rgb(235, 111, 146);
pub const PEACH: Color = Color::Rgb(245, 164, 136);
pub const YELLOW: Color = Color::Rgb(238, 212, 159);
pub const GREEN: Color = Color::Rgb(166, 227, 149);
pub const TEAL: Color = Color::Rgb(139, 213, 202);
#[allow(dead_code)]
pub const SKY: Color = Color::Rgb(137, 220, 235);
#[allow(dead_code)]
pub const SAPPHIRE: Color = Color::Rgb(125, 196, 228);
pub const BLUE: Color = Color::Rgb(138, 173, 244);
pub const LAVENDER: Color = Color::Rgb(183, 223, 249);
pub const TEXT: Color = Color::Rgb(202, 211, 245);
pub const SUBTEXT: Color = Color::Rgb(165, 173, 203);
pub const OVERLAY: Color = Color::Rgb(128, 135, 162);
pub const SURFACE0: Color = Color::Rgb(54, 58, 79);
pub const SURFACE1: Color = Color::Rgb(73, 77, 100);
pub const SURFACE2: Color = Color::Rgb(91, 96, 120);
pub const BASE: Color = Color::Rgb(30, 30, 46);
pub const MANTLE: Color = Color::Rgb(24, 24, 37);
pub const CRUST: Color = Color::Rgb(17, 17, 27);
pub const FOCUS_BORDER: Color = LAVENDER;
pub const FOCUS_BORDER_ALT: Color = MAUVE;
pub const FOCUS_TAB: Color = MAUVE;
