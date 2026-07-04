//! Tests for the theme system.

use vitalis::ui::theme::Theme;

#[test]
fn load_builtin_macchiato() {
    let theme = Theme::built_in("catppuccin-macchiato").expect("macchiato should exist");
    assert_eq!(theme.name, "Catppuccin Macchiato");
    // Check some key colors
    assert_eq!(theme.red.r, 237);
    assert_eq!(theme.red.g, 135);
    assert_eq!(theme.red.b, 150);
}

#[test]
fn load_builtin_mocha() {
    let theme = Theme::built_in("mocha").expect("mocha alias should work");
    assert_eq!(theme.name, "Catppuccin Mocha");
}

#[test]
fn load_builtin_dracula() {
    let theme = Theme::built_in("dracula").expect("dracula should exist");
    assert_eq!(theme.name, "Dracula");
    // Dracula's green
    assert_eq!(theme.green.r, 80);
}

#[test]
fn load_builtin_nord() {
    let theme = Theme::built_in("nord").expect("nord should exist");
    assert_eq!(theme.name, "Nord");
}

#[test]
fn load_builtin_gruvbox() {
    let theme = Theme::built_in("gruvbox").expect("gruvbox should exist");
    assert_eq!(theme.name, "Gruvbox Dark");
}

#[test]
fn load_builtin_tokyo_night() {
    let theme = Theme::built_in("tokyo-night").expect("tokyo-night should exist");
    assert_eq!(theme.name, "Tokyo Night");
}

#[test]
fn load_builtin_one_dark() {
    let theme = Theme::built_in("one-dark").expect("one-dark should exist");
    assert_eq!(theme.name, "One Dark");
}

#[test]
fn unknown_theme_returns_none() {
    assert!(Theme::built_in("nonexistent-theme").is_none());
}

#[test]
fn load_falls_back_to_macchiato() {
    let theme = Theme::load("totally-fake-theme");
    assert_eq!(theme.name, "Catppuccin Macchiato");
}

#[test]
fn all_themes_have_unique_base_colors() {
    let themes: Vec<(&str, Theme)> = [
        "catppuccin-macchiato",
        "catppuccin-mocha",
        "dracula",
        "nord",
        "gruvbox",
        "tokyo-night",
        "one-dark",
    ]
    .iter()
    .map(|name| (*name, Theme::built_in(name).unwrap()))
    .collect();

    for i in 0..themes.len() {
        for j in (i + 1)..themes.len() {
            let (name_a, a) = &themes[i];
            let (name_b, b) = &themes[j];
            assert!(
                a.base != b.base || a.text != b.text,
                "{} and {} have identical base+text colors",
                name_a,
                name_b,
            );
        }
    }
}

#[test]
fn color_def_to_color() {
    use ratatui::style::Color;
    use vitalis::ui::theme::ColorDef;

    let cd = ColorDef::rgb(100, 150, 200);
    assert_eq!(cd.to_color(), Color::Rgb(100, 150, 200));
}

#[test]
fn theme_serialization_roundtrip() {
    let original = Theme::catppuccin_macchiato();
    let toml_str = toml::to_string_pretty(&original).expect("serialize");
    let deserialized: Theme = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(original.name, deserialized.name);
    assert_eq!(original.red.r, deserialized.red.r);
    assert_eq!(original.base.g, deserialized.base.g);
}
