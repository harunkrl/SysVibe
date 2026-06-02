//! SysVibe — Pluggable color theme system.
//!
//! Defines a `Theme` struct containing all color tokens used by the UI.
//! Ships with 7 built-in themes and supports TOML-based custom themes.
//! The `palette` module re-exports theme colors as functions for easy access.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

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

    #[allow(dead_code)]
    pub fn to_color(self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
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

    /// Load theme by name: check built-ins first, then try TOML file.
    pub fn load(name: &str) -> Self {
        if let Some(theme) = Self::built_in(name) {
            return theme;
        }

        // Try loading from ~/.config/sysvibe/themes/{name}.toml
        if let Some(theme) = Self::load_from_file(name) {
            return theme;
        }

        // Fallback
        Self::catppuccin_macchiato()
    }

    fn load_from_file(name: &str) -> Option<Self> {
        let path = dirs::config_dir()?
            .join("sysvibe")
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
        }
    }
}
