//! Vitalis — Pluggable color theme system.
//!
//! Defines a `Theme` struct containing all color tokens used by the UI.
//! Ships with 7 built-in themes and supports TOML-based custom themes.
//! The `palette` module re-exports theme colors as functions for easy access.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

/// Complete color theme for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    // Foreground
    pub rosewater: ColorDef,
    pub flamingo: ColorDef,
    pub pink: ColorDef,
    pub mauve: ColorDef,
    pub red: ColorDef,
    pub maroon: ColorDef,
    pub peach: ColorDef,
    pub yellow: ColorDef,
    pub green: ColorDef,
    pub teal: ColorDef,
    pub sky: ColorDef,
    pub sapphire: ColorDef,
    pub blue: ColorDef,
    pub lavender: ColorDef,
    // Text
    pub text: ColorDef,
    pub subtext: ColorDef,
    pub overlay: ColorDef,
    // Surfaces
    pub surface0: ColorDef,
    pub surface1: ColorDef,
    pub surface2: ColorDef,
    // Backgrounds
    pub base: ColorDef,
    pub mantle: ColorDef,
    pub crust: ColorDef,
    // Focus
    pub focus_border: ColorDef,
    pub focus_tab: ColorDef,
    // Blur-friendly overrides (optional): brighter overlay/subtext used when
    // palette::blur_active() is true. Built-ins always set these; custom TOML
    // themes may omit them (palette falls back to derive_blur).
    pub overlay_blur: Option<ColorDef>,
    pub subtext_blur: Option<ColorDef>,
}

/// Serializable color definition — supports RGB triplets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorDef {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorDef {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_color(self) -> Color {
        match color_capacity() {
            ColorCapacity::Truecolor => Color::Rgb(self.r, self.g, self.b),
            ColorCapacity::TwoFiftySix => Color::Indexed(quantize_to_256(self.r, self.g, self.b)),
            ColorCapacity::Sixteen => Color::Indexed(quantize_to_16(self.r, self.g, self.b)),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Terminal color-capacity detection + fallback quantization (audit O-5).
//
// Themes are authored in truecolor RGB. On limited terminals (older
// emulators, some Termux builds) the palette is quantized to the terminal's
// actual color space so it degrades gracefully instead of rendering as
// garbage. Detection runs once at startup; `ColorDef::to_color` — the single
// color chokepoint every palette accessor goes through — consults it on each
// call. The truecolor path is a pure identity, so dev, `cargo test`, and
// svshot rendering stay byte-identical when no limited terminal is detected.
// ═══════════════════════════════════════════════════════════════════════

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapacity {
    Truecolor = 0,
    TwoFiftySix = 1,
    Sixteen = 2,
}

static COLOR_CAPACITY: AtomicU8 = AtomicU8::new(ColorCapacity::Truecolor as u8);

fn color_capacity() -> ColorCapacity {
    match COLOR_CAPACITY.load(Ordering::Relaxed) {
        1 => ColorCapacity::TwoFiftySix,
        2 => ColorCapacity::Sixteen,
        _ => ColorCapacity::Truecolor,
    }
}

/// Install the terminal's color capacity. Call once at startup, before render.
pub fn set_color_capacity(cap: ColorCapacity) {
    COLOR_CAPACITY.store(cap as u8, Ordering::Relaxed);
}

/// Detect the terminal's color capacity from `COLORTERM` / `TERM`. Defaults to
/// Truecolor when no limiting signal is present (modern emulators, `cargo
/// test`, svshot) so output stays full-color unless a limited terminal is
/// actually detected.
pub fn detect_color_capacity() -> ColorCapacity {
    detect_from(
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

fn detect_from(colorterm: Option<&str>, term: Option<&str>) -> ColorCapacity {
    if let Some(ct) = colorterm {
        let ct = ct.to_lowercase();
        if ct.contains("truecolor") || ct.contains("24bit") {
            return ColorCapacity::Truecolor;
        }
    }
    if let Some(term) = term {
        if term.contains("256") {
            return ColorCapacity::TwoFiftySix;
        }
        let lower = term.to_lowercase();
        if lower == "dumb" || lower.starts_with("linux") || lower.starts_with("vt") {
            return ColorCapacity::Sixteen;
        }
    }
    ColorCapacity::Truecolor
}

/// Nearest of the xterm 216-color cube's 6 levels for one channel.
fn cube_level(channel: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let mut best = 0u8;
    let mut best_dist = u32::MAX;
    for (i, &level) in LEVELS.iter().enumerate() {
        let d = (channel as i32 - level as i32).unsigned_abs();
        if d < best_dist {
            best_dist = d;
            best = i as u8;
        }
    }
    best
}

/// Quantize RGB to the nearest xterm 256-color index (the 216-color cube,
/// indices 16..=231). Greyscales and the 16 system colors are intentionally
/// unused — the cube alone covers the themed palette well enough for a fallback.
fn quantize_to_256(r: u8, g: u8, b: u8) -> u8 {
    16 + 36 * cube_level(r) + 6 * cube_level(g) + cube_level(b)
}

/// Quantize RGB to the nearest of the 16 ANSI colors. Channels are binarized
/// at a threshold; the on/off pattern maps to the 8 base colors, promoted to
/// the bright range (8..=15) when a channel is intense. Coarse but robust for
/// the worst-case terminal.
fn quantize_to_16(r: u8, g: u8, b: u8) -> u8 {
    let base = match (r > 64, g > 64, b > 64) {
        (false, false, false) => 0, // black
        (true, false, false) => 1,  // red
        (false, true, false) => 2,  // green
        (true, true, false) => 3,   // yellow
        (false, false, true) => 4,  // blue
        (true, false, true) => 5,   // magenta
        (false, true, true) => 6,   // cyan
        (true, true, true) => 7,    // white
    };
    if r > 200 || g > 200 || b > 200 {
        base + 8
    } else {
        base
    }
}

#[cfg(test)]
mod color_capacity_tests {
    use super::*;

    struct CapacityGuard;
    impl Drop for CapacityGuard {
        fn drop(&mut self) {
            set_color_capacity(ColorCapacity::Truecolor);
        }
    }

    #[test]
    fn to_color_truecolor_is_rgb_identity() {
        let _g = CapacityGuard;
        set_color_capacity(ColorCapacity::Truecolor);
        assert_eq!(ColorDef::rgb(10, 20, 30).to_color(), Color::Rgb(10, 20, 30));
    }

    #[test]
    fn to_color_256_yields_indexed() {
        let _g = CapacityGuard;
        set_color_capacity(ColorCapacity::TwoFiftySix);
        assert_eq!(ColorDef::rgb(0, 0, 0).to_color(), Color::Indexed(16));
        assert_eq!(ColorDef::rgb(255, 255, 255).to_color(), Color::Indexed(231));
        assert_eq!(ColorDef::rgb(255, 0, 0).to_color(), Color::Indexed(196));
    }

    #[test]
    fn to_color_16_yields_basic_ansi() {
        let _g = CapacityGuard;
        set_color_capacity(ColorCapacity::Sixteen);
        assert_eq!(ColorDef::rgb(0, 0, 0).to_color(), Color::Indexed(0));
        assert_eq!(ColorDef::rgb(255, 255, 255).to_color(), Color::Indexed(15));
        assert_eq!(ColorDef::rgb(255, 0, 0).to_color(), Color::Indexed(9));
    }

    #[test]
    fn detect_pure_logic() {
        // COLORTERM wins over a 256-color TERM.
        assert_eq!(
            detect_from(Some("truecolor"), Some("xterm-256color")),
            ColorCapacity::Truecolor
        );
        assert_eq!(detect_from(Some("24bit"), None), ColorCapacity::Truecolor);
        // TERM signals 256-color.
        assert_eq!(
            detect_from(None, Some("xterm-256color")),
            ColorCapacity::TwoFiftySix
        );
        assert_eq!(
            detect_from(None, Some("screen-256color")),
            ColorCapacity::TwoFiftySix
        );
        // 16-color consoles.
        assert_eq!(detect_from(None, Some("linux")), ColorCapacity::Sixteen);
        assert_eq!(detect_from(None, Some("dumb")), ColorCapacity::Sixteen);
        assert_eq!(detect_from(None, Some("vt100")), ColorCapacity::Sixteen);
        // No limiting signal → Truecolor (safe default).
        assert_eq!(detect_from(None, None), ColorCapacity::Truecolor);
        assert_eq!(detect_from(None, Some("xterm")), ColorCapacity::Truecolor);
    }

    #[test]
    fn quantize_256_corners() {
        assert_eq!(quantize_to_256(0, 0, 0), 16);
        assert_eq!(quantize_to_256(255, 255, 255), 231);
        assert_eq!(quantize_to_256(255, 0, 0), 196); // 16 + 36*5
        assert_eq!(quantize_to_256(0, 255, 0), 46); // 16 + 6*5
        assert_eq!(quantize_to_256(0, 0, 255), 21); // 16 + 5
    }

    #[test]
    fn quantize_16_corners() {
        assert_eq!(quantize_to_16(0, 0, 0), 0);
        assert_eq!(quantize_to_16(255, 255, 255), 15);
        assert_eq!(quantize_to_16(255, 0, 0), 9);
        assert_eq!(quantize_to_16(0, 255, 0), 10);
        assert_eq!(quantize_to_16(0, 0, 255), 12);
    }
}

impl Theme {
    /// Get a built-in theme by name.
    pub fn built_in(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "catppuccin-macchiato" | "macchiato" | "default" => Some(Self::catppuccin_macchiato()),
            "catppuccin-mocha" | "mocha" => Some(Self::catppuccin_mocha()),
            "dracula" => Some(Self::dracula()),
            "nord" => Some(Self::nord()),
            "gruvbox" => Some(Self::gruvbox()),
            "tokyo-night" => Some(Self::tokyo_night()),
            "one-dark" => Some(Self::one_dark()),
            _ => None,
        }
    }

    /// All built-in themes as `(key, Theme)` pairs, in canonical cycle order.
    /// The key matches `Config::theme` (e.g. "catppuccin-macchiato").
    pub fn all_built_ins() -> Vec<(&'static str, Theme)> {
        vec![
            ("catppuccin-macchiato", Self::catppuccin_macchiato()),
            ("catppuccin-mocha", Self::catppuccin_mocha()),
            ("dracula", Self::dracula()),
            ("nord", Self::nord()),
            ("gruvbox", Self::gruvbox()),
            ("tokyo-night", Self::tokyo_night()),
            ("one-dark", Self::one_dark()),
        ]
    }

    /// Load theme by name: check built-ins first, then try TOML file.
    pub fn load(name: &str) -> Self {
        if let Some(theme) = Self::built_in(name) {
            return theme;
        }

        // Try loading from ~/.config/vitalis/themes/{name}.toml
        if let Some(theme) = Self::load_from_file(name) {
            return theme;
        }

        // Fallback
        Self::catppuccin_macchiato()
    }

    fn load_from_file(name: &str) -> Option<Self> {
        let path = dirs::config_dir()?
            .join("vitalis")
            .join("themes")
            .join(format!("{}.toml", name));

        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    }

    // ═══════════════════════════════════════════════════════════════════
    // Built-in themes
    // ═══════════════════════════════════════════════════════════════════

    pub fn catppuccin_macchiato() -> Self {
        Self {
            name: "Catppuccin Macchiato".into(),
            rosewater: ColorDef::rgb(244, 194, 219),
            flamingo: ColorDef::rgb(242, 205, 205),
            pink: ColorDef::rgb(245, 189, 230),
            mauve: ColorDef::rgb(198, 160, 246),
            red: ColorDef::rgb(237, 135, 150),
            maroon: ColorDef::rgb(235, 111, 146),
            peach: ColorDef::rgb(245, 164, 136),
            yellow: ColorDef::rgb(238, 212, 159),
            green: ColorDef::rgb(166, 227, 149),
            teal: ColorDef::rgb(139, 213, 202),
            sky: ColorDef::rgb(137, 220, 235),
            sapphire: ColorDef::rgb(125, 196, 228),
            blue: ColorDef::rgb(138, 173, 244),
            lavender: ColorDef::rgb(183, 223, 249),
            text: ColorDef::rgb(202, 211, 245),
            subtext: ColorDef::rgb(165, 173, 203),
            overlay: ColorDef::rgb(128, 135, 162),
            surface0: ColorDef::rgb(54, 58, 79),
            surface1: ColorDef::rgb(73, 77, 100),
            surface2: ColorDef::rgb(91, 96, 120),
            base: ColorDef::rgb(30, 30, 46),
            mantle: ColorDef::rgb(24, 24, 37),
            crust: ColorDef::rgb(17, 17, 27),
            focus_border: ColorDef::rgb(183, 223, 249), // lavender
            focus_tab: ColorDef::rgb(198, 160, 246),    // mauve
            overlay_blur: Some(ColorDef::rgb(184, 192, 224)),
            subtext_blur: Some(ColorDef::rgb(189, 198, 230)),
        }
    }

    fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".into(),
            rosewater: ColorDef::rgb(245, 224, 220),
            flamingo: ColorDef::rgb(242, 205, 205),
            pink: ColorDef::rgb(245, 194, 231),
            mauve: ColorDef::rgb(203, 166, 247),
            red: ColorDef::rgb(243, 139, 168),
            maroon: ColorDef::rgb(235, 160, 172),
            peach: ColorDef::rgb(250, 179, 135),
            yellow: ColorDef::rgb(249, 226, 175),
            green: ColorDef::rgb(166, 227, 161),
            teal: ColorDef::rgb(148, 226, 213),
            sky: ColorDef::rgb(137, 220, 235),
            sapphire: ColorDef::rgb(116, 199, 236),
            blue: ColorDef::rgb(137, 180, 250),
            lavender: ColorDef::rgb(180, 190, 254),
            text: ColorDef::rgb(205, 214, 244),
            subtext: ColorDef::rgb(166, 173, 200),
            overlay: ColorDef::rgb(147, 153, 178),
            surface0: ColorDef::rgb(49, 50, 68),
            surface1: ColorDef::rgb(69, 71, 90),
            surface2: ColorDef::rgb(88, 91, 112),
            base: ColorDef::rgb(30, 30, 46),
            mantle: ColorDef::rgb(24, 24, 37),
            crust: ColorDef::rgb(17, 17, 27),
            focus_border: ColorDef::rgb(180, 190, 254),
            focus_tab: ColorDef::rgb(203, 166, 247),
            overlay_blur: Some(ColorDef::rgb(190, 199, 228)),
            subtext_blur: Some(ColorDef::rgb(191, 200, 229)),
        }
    }

    fn dracula() -> Self {
        Self {
            name: "Dracula".into(),
            rosewater: ColorDef::rgb(255, 121, 198),
            flamingo: ColorDef::rgb(255, 121, 198),
            pink: ColorDef::rgb(255, 121, 198),
            mauve: ColorDef::rgb(189, 147, 249),
            red: ColorDef::rgb(255, 85, 85),
            maroon: ColorDef::rgb(255, 85, 85),
            peach: ColorDef::rgb(255, 184, 108),
            yellow: ColorDef::rgb(241, 250, 140),
            green: ColorDef::rgb(80, 250, 123),
            teal: ColorDef::rgb(139, 233, 253),
            sky: ColorDef::rgb(139, 233, 253),
            sapphire: ColorDef::rgb(139, 233, 253),
            blue: ColorDef::rgb(98, 114, 164),
            lavender: ColorDef::rgb(189, 147, 249),
            text: ColorDef::rgb(248, 248, 242),
            subtext: ColorDef::rgb(144, 144, 144),
            overlay: ColorDef::rgb(98, 114, 164),
            surface0: ColorDef::rgb(68, 71, 90),
            surface1: ColorDef::rgb(88, 91, 112),
            surface2: ColorDef::rgb(98, 114, 164),
            base: ColorDef::rgb(40, 42, 54),
            mantle: ColorDef::rgb(34, 36, 46),
            crust: ColorDef::rgb(28, 30, 38),
            focus_border: ColorDef::rgb(189, 147, 249),
            focus_tab: ColorDef::rgb(255, 121, 198),
            overlay_blur: Some(ColorDef::rgb(210, 214, 222)),
            subtext_blur: Some(ColorDef::rgb(212, 212, 208)),
        }
    }

    fn nord() -> Self {
        Self {
            name: "Nord".into(),
            rosewater: ColorDef::rgb(216, 222, 233),
            flamingo: ColorDef::rgb(216, 222, 233),
            pink: ColorDef::rgb(180, 142, 173),
            mauve: ColorDef::rgb(180, 142, 173),
            red: ColorDef::rgb(191, 97, 106),
            maroon: ColorDef::rgb(191, 97, 106),
            peach: ColorDef::rgb(208, 135, 112),
            yellow: ColorDef::rgb(235, 203, 139),
            green: ColorDef::rgb(163, 190, 140),
            teal: ColorDef::rgb(136, 192, 208),
            sky: ColorDef::rgb(136, 192, 208),
            sapphire: ColorDef::rgb(136, 192, 208),
            blue: ColorDef::rgb(129, 161, 193),
            lavender: ColorDef::rgb(180, 142, 173),
            text: ColorDef::rgb(216, 222, 233),
            subtext: ColorDef::rgb(229, 233, 240),
            overlay: ColorDef::rgb(76, 86, 106),
            surface0: ColorDef::rgb(59, 66, 82),
            surface1: ColorDef::rgb(67, 76, 94),
            surface2: ColorDef::rgb(76, 86, 106),
            base: ColorDef::rgb(46, 52, 64),
            mantle: ColorDef::rgb(41, 46, 57),
            crust: ColorDef::rgb(35, 40, 50),
            focus_border: ColorDef::rgb(136, 192, 208),
            focus_tab: ColorDef::rgb(180, 142, 173),
            overlay_blur: Some(ColorDef::rgb(201, 204, 210)),
            subtext_blur: Some(ColorDef::rgb(221, 226, 235)),
        }
    }

    fn gruvbox() -> Self {
        Self {
            name: "Gruvbox Dark".into(),
            rosewater: ColorDef::rgb(235, 219, 178),
            flamingo: ColorDef::rgb(235, 219, 178),
            pink: ColorDef::rgb(211, 134, 155),
            mauve: ColorDef::rgb(211, 134, 155),
            red: ColorDef::rgb(204, 36, 29),
            maroon: ColorDef::rgb(157, 0, 6),
            peach: ColorDef::rgb(214, 93, 14),
            yellow: ColorDef::rgb(215, 153, 33),
            green: ColorDef::rgb(152, 151, 26),
            teal: ColorDef::rgb(104, 157, 106),
            sky: ColorDef::rgb(69, 133, 136),
            sapphire: ColorDef::rgb(69, 133, 136),
            blue: ColorDef::rgb(69, 133, 136),
            lavender: ColorDef::rgb(177, 98, 134),
            text: ColorDef::rgb(235, 219, 178),
            subtext: ColorDef::rgb(168, 153, 132),
            overlay: ColorDef::rgb(146, 131, 116),
            surface0: ColorDef::rgb(60, 56, 54),
            surface1: ColorDef::rgb(80, 73, 69),
            surface2: ColorDef::rgb(102, 92, 84),
            base: ColorDef::rgb(40, 40, 40),
            mantle: ColorDef::rgb(29, 32, 33),
            crust: ColorDef::rgb(20, 20, 20),
            focus_border: ColorDef::rgb(215, 153, 33),
            focus_tab: ColorDef::rgb(177, 98, 134),
            overlay_blur: Some(ColorDef::rgb(213, 197, 162)),
            subtext_blur: Some(ColorDef::rgb(212, 196, 162)),
        }
    }

    fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night".into(),
            rosewater: ColorDef::rgb(198, 160, 246),
            flamingo: ColorDef::rgb(245, 224, 220),
            pink: ColorDef::rgb(247, 118, 142),
            mauve: ColorDef::rgb(198, 160, 246),
            red: ColorDef::rgb(247, 118, 142),
            maroon: ColorDef::rgb(219, 75, 101),
            peach: ColorDef::rgb(255, 158, 100),
            yellow: ColorDef::rgb(224, 175, 104),
            green: ColorDef::rgb(158, 206, 106),
            teal: ColorDef::rgb(125, 207, 255),
            sky: ColorDef::rgb(125, 207, 255),
            sapphire: ColorDef::rgb(125, 207, 255),
            blue: ColorDef::rgb(122, 162, 247),
            lavender: ColorDef::rgb(165, 130, 242),
            text: ColorDef::rgb(192, 202, 245),
            subtext: ColorDef::rgb(169, 177, 214),
            overlay: ColorDef::rgb(131, 137, 174),
            surface0: ColorDef::rgb(65, 70, 103),
            surface1: ColorDef::rgb(82, 87, 123),
            surface2: ColorDef::rgb(98, 104, 140),
            base: ColorDef::rgb(26, 27, 46),
            mantle: ColorDef::rgb(22, 23, 40),
            crust: ColorDef::rgb(18, 19, 34),
            focus_border: ColorDef::rgb(122, 162, 247),
            focus_tab: ColorDef::rgb(198, 160, 246),
            overlay_blur: Some(ColorDef::rgb(193, 196, 214)),
            subtext_blur: Some(ColorDef::rgb(184, 193, 234)),
        }
    }

    fn one_dark() -> Self {
        Self {
            name: "One Dark".into(),
            rosewater: ColorDef::rgb(198, 120, 221),
            flamingo: ColorDef::rgb(198, 120, 221),
            pink: ColorDef::rgb(198, 120, 221),
            mauve: ColorDef::rgb(198, 120, 221),
            red: ColorDef::rgb(224, 108, 117),
            maroon: ColorDef::rgb(190, 80, 70),
            peach: ColorDef::rgb(209, 154, 102),
            yellow: ColorDef::rgb(229, 192, 123),
            green: ColorDef::rgb(152, 195, 121),
            teal: ColorDef::rgb(86, 182, 194),
            sky: ColorDef::rgb(86, 182, 194),
            sapphire: ColorDef::rgb(86, 182, 194),
            blue: ColorDef::rgb(97, 175, 239),
            lavender: ColorDef::rgb(97, 175, 239),
            text: ColorDef::rgb(171, 178, 191),
            subtext: ColorDef::rgb(92, 99, 112),
            overlay: ColorDef::rgb(92, 99, 112),
            surface0: ColorDef::rgb(55, 61, 72),
            surface1: ColorDef::rgb(66, 72, 84),
            surface2: ColorDef::rgb(77, 83, 96),
            base: ColorDef::rgb(40, 44, 52),
            mantle: ColorDef::rgb(33, 37, 43),
            crust: ColorDef::rgb(27, 29, 35),
            focus_border: ColorDef::rgb(97, 175, 239),
            focus_tab: ColorDef::rgb(198, 120, 221),
            overlay_blur: Some(ColorDef::rgb(190, 193, 198)),
            subtext_blur: Some(ColorDef::rgb(190, 193, 198)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_built_ins_define_blur_palette() {
        // Every built-in must ship explicit blur overrides (no None) so blur
        // mode never falls back to derive_blur for shipped themes.
        for (name, theme) in Theme::all_built_ins() {
            assert!(theme.overlay_blur.is_some(), "{name} missing overlay_blur");
            assert!(theme.subtext_blur.is_some(), "{name} missing subtext_blur");
        }
    }

    #[test]
    fn dracula_blur_values_match_spec() {
        // Reach the configured default theme via the public lookup.
        let t = Theme::load("dracula");
        assert_eq!(t.overlay_blur, Some(ColorDef::rgb(210, 214, 222)));
        assert_eq!(t.subtext_blur, Some(ColorDef::rgb(212, 212, 208)));
    }
}
