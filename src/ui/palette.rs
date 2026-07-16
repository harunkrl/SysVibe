//! Vitalis — Color palette (re-exported from theme system).
//!
//! For backwards compatibility, this module provides static color constants
//! matching the Catppuccin Macchiato theme (the default).
//!
//! For runtime theme support, use `crate::ui::theme::Theme` instead.
//! The `apply_theme()` function updates all palette constants via a
//! thread-local theme cache.

use ratatui::style::Color;
use std::cell::{Cell, RefCell};

use super::theme::{ColorDef, Theme};

// Thread-local theme storage
thread_local! {
    static CURRENT_THEME: RefCell<Theme> = RefCell::new(Theme::catppuccin_macchiato());
}

// Thread-local blur-friendly flag. When true, overlay()/subtext() return each
// theme's brighter *_blur variant (or a derived fallback) so dim text stays
// readable under terminal compositor blur. Default false → current behaviour.
thread_local! {
    // `const {}` is already used; clippy 1.96's missing_const_for_thread_local
    // still flags it under some targets (a known FP), so allow it on the item.
    #[allow(clippy::missing_const_for_thread_local)]
    static BLUR_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Enable/disable blur-friendly palette globally.
pub fn set_blur_active(on: bool) {
    BLUR_ACTIVE.with(|c| c.set(on));
}

/// Whether the blur-friendly palette is active.
pub fn blur_active() -> bool {
    BLUR_ACTIVE.with(|c| c.get())
}

/// Best-effort blur fallback for themes without explicit `*_blur` fields
/// (custom TOML themes): blend the original colour toward the theme's `text`
/// by 0.70. Built-in themes never hit this path.
fn derive_blur(orig: ColorDef, text: ColorDef) -> Color {
    let f = 0.70;
    let lerp = |a: u8, b: u8| {
        (a as f64 + (b as f64 - a as f64) * f)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::Rgb(
        lerp(orig.r, text.r),
        lerp(orig.g, text.g),
        lerp(orig.b, text.b),
    )
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

/// subtext accessor — returns the blur-friendly variant when blur is active.
pub fn subtext() -> Color {
    CURRENT_THEME.with(|t| {
        let theme = t.borrow();
        if blur_active() {
            theme
                .subtext_blur
                .map(|c| c.to_color())
                .unwrap_or_else(|| derive_blur(theme.subtext, theme.text))
        } else {
            theme.subtext.to_color()
        }
    })
}

/// overlay accessor — returns the blur-friendly variant when blur is active.
pub fn overlay() -> Color {
    CURRENT_THEME.with(|t| {
        let theme = t.borrow();
        if blur_active() {
            theme
                .overlay_blur
                .map(|c| c.to_color())
                .unwrap_or_else(|| derive_blur(theme.overlay, theme.text))
        } else {
            theme.overlay.to_color()
        }
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Guard: restore both the blur flag AND the default theme on drop so tests
    /// don't leak thread-local state into each other (cargo test reuses threads
    /// from its pool, and thread-locals persist per-thread).
    struct BlurGuard;
    impl Drop for BlurGuard {
        fn drop(&mut self) {
            set_blur_active(false);
            apply_theme(Theme::catppuccin_macchiato());
        }
    }

    #[test]
    fn overlay_off_returns_normal_overlay() {
        let _g = BlurGuard;
        set_blur_active(false);
        // CURRENT_THEME defaults to catppuccin_macchiato.
        let normal = Theme::catppuccin_macchiato().overlay.to_color();
        assert_eq!(overlay(), normal);
    }

    #[test]
    fn overlay_on_returns_blur_variant() {
        let _g = BlurGuard;
        set_blur_active(true);
        let blur = Theme::catppuccin_macchiato()
            .overlay_blur
            .expect("macchiato has overlay_blur")
            .to_color();
        assert_eq!(overlay(), blur);
        // And it must differ from the normal value.
        assert_ne!(blur, Theme::catppuccin_macchiato().overlay.to_color());
    }

    #[test]
    fn derive_blur_blends_toward_text_70pct() {
        let orig = ColorDef::rgb(0, 0, 0);
        let text = ColorDef::rgb(100, 100, 100);
        // 0.70 of the way from 0 to 100 = 70.
        assert_eq!(derive_blur(orig, text), Color::Rgb(70, 70, 70));
    }

    #[test]
    fn derive_blur_path_used_when_field_none() {
        // A hand-built theme with no blur fields exercises the fallback via
        // the accessor by applying it as the current theme.
        let _g = BlurGuard;
        let mut theme = Theme::catppuccin_macchiato();
        theme.overlay_blur = None;
        let expected = derive_blur(theme.overlay, theme.text);
        apply_theme(theme);
        set_blur_active(true);
        assert_eq!(overlay(), expected);
    }
}
