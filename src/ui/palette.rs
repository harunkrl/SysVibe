//! Vitalis — Color palette (re-exported from theme system).
//!
//! For backwards compatibility, this module provides static color constants
//! matching the Catppuccin Macchiato theme (the default).
//!
//! For runtime theme support, use `crate::ui::theme::Theme` instead.
//! The `apply_theme()` function updates all palette constants via a
//! thread-local theme cache.

use ratatui::style::Color;
use std::cell::RefCell;

use super::theme::Theme;

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
        #[allow(dead_code)]
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
// NOTE: Static color constants have been removed. All color access
// now goes through the runtime function accessors above, which
// respect the current theme. This ensures theme switching works
// correctly across all UI elements.
// ═══════════════════════════════════════════════════════════════════════
