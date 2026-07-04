// Dev-only module (preview feature). Compiled into the main `vitalis` bin
// via `mod ui`, which never calls these functions — silence that expected
// dead code so `cargo build --features preview` stays warning-free.
#![allow(dead_code)]

//! Vitalis — Preview rendering: converts a ratatui Buffer to SVG.
//!
//! Gated behind the `preview` feature; used by the `svshot` dev tool.

use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier},
};
use unicode_width::UnicodeWidthStr;

/// SVG font stack. CSS font names are single-quoted so they never clash with
/// the surrounding double-quoted `style="..."` XML attribute. A Nerd Font is
/// listed first so Nerd Font glyphs render in-browser when one is installed;
/// `monospace` is the fallback.
const FONT_FAMILY: &str =
    "'Symbols Nerd Font Mono','Hack Nerd Font','JetBrainsMono Nerd Font',monospace";

const CELL_W: u32 = 10;
const CELL_H: u32 = 20;

/// Convert a rendered ratatui `Buffer` to a standalone SVG string.
///
/// Each cell becomes a background `<rect>` and (when it has a visible glyph)
/// a `<text>`. Wide glyphs advance past their continuation cells exactly as
/// ratatui's own buffer-view helper does (width-based skipping).
#[allow(clippy::collapsible_if)]
pub fn buffer_to_svg(buffer: &Buffer) -> String {
    let cols = buffer.area.width;
    let rows = buffer.area.height;
    let view_w = cols as u32 * CELL_W;
    let view_h = rows as u32 * CELL_H;

    let mut svg = String::with_capacity((cols as usize) * (rows as usize) * 48);
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">",
        w = view_w,
        h = view_h
    ));

    let mut skip: usize = 0;
    for y in 0..rows {
        for x in 0..cols {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let cell = &buffer[(x, y)];
            let px = x as u32 * CELL_W;
            let py = y as u32 * CELL_H;

            // Effective colors: REVERSED swaps fg/bg.
            let (efg, ebg) = if cell.modifier.contains(Modifier::REVERSED) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };

            if let Some(hex) = color_hex(ebg) {
                svg.push_str(&format!(
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{c}\"/>",
                    x = px,
                    y = py,
                    w = CELL_W,
                    h = CELL_H,
                    c = hex
                ));
            }

            let sym = cell.symbol();
            if !sym.is_empty() && sym != " " {
                if let Some(hex) = color_hex(efg) {
                    let mut style = format!(
                        "font-family:{fam};font-size:{fs}px;",
                        fam = FONT_FAMILY,
                        fs = CELL_H - 4
                    );
                    let m = cell.modifier;
                    if m.contains(Modifier::BOLD) {
                        style.push_str("font-weight:bold;");
                    }
                    if m.contains(Modifier::ITALIC) {
                        style.push_str("font-style:italic;");
                    }
                    if m.contains(Modifier::UNDERLINED) {
                        style.push_str("text-decoration:underline;");
                    }
                    let opacity = if m.contains(Modifier::DIM) {
                        "0.5"
                    } else {
                        "1.0"
                    };
                    svg.push_str(&format!(
                        "<text x=\"{tx}\" y=\"{ty}\" fill=\"{c}\" opacity=\"{o}\" style=\"{s}\">{g}</text>",
                        tx = px + 1,
                        ty = py + CELL_H - 4,
                        c = hex,
                        o = opacity,
                        s = style,
                        g = xml_escape(sym)
                    ));
                }
            }

            // Advance past wide-glyph continuation cells.
            skip = sym.width().saturating_sub(1);
        }
    }

    svg.push_str("</svg>");
    svg
}

use crate::app::App;

/// Render the live Vitalis UI for `app` at `width`×`height` to an SVG string,
/// using a `TestBackend` (no terminal required). Uses the exact `ui::draw`
/// code path — so the output is pixel-faithful to the real app.
pub fn render_app_to_svg(app: &mut App, width: u16, height: u16) -> String {
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test backend must construct");
    terminal
        .draw(|f| crate::ui::draw(f, app))
        .expect("draw must succeed");
    buffer_to_svg(terminal.backend().buffer())
}

/// Map a ratatui `Color` to a CSS hex string, or `None` for `Reset`
/// (transparent / "use default"). Named ANSI colors use standard hex.
fn color_hex(c: Color) -> Option<String> {
    match c {
        Color::Reset => None,
        Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
        Color::Indexed(i) => Some(xterm_hex(i)),
        Color::Black => Some("#000000".into()),
        Color::Red => Some("#aa0000".into()),
        Color::Green => Some("#00aa00".into()),
        Color::Yellow => Some("#aa5500".into()),
        Color::Blue => Some("#0000aa".into()),
        Color::Magenta => Some("#aa00aa".into()),
        Color::Cyan => Some("#00aaaa".into()),
        Color::Gray => Some("#aaaaaa".into()),
        Color::DarkGray => Some("#555555".into()),
        Color::LightRed => Some("#ff5555".into()),
        Color::LightGreen => Some("#55ff55".into()),
        Color::LightYellow => Some("#ffff55".into()),
        Color::LightBlue => Some("#5555ff".into()),
        Color::LightMagenta => Some("#ff55ff".into()),
        Color::LightCyan => Some("#55ffff".into()),
        Color::White => Some("#ffffff".into()),
    }
}

/// Standard xterm 256-color palette lookup for `Color::Indexed`.
fn xterm_hex(i: u8) -> String {
    if i < 16 {
        // The first 16 mirror the named ANSI set above.
        return match i {
            0 => "#000000",
            1 => "#aa0000",
            2 => "#00aa00",
            3 => "#aa5500",
            4 => "#0000aa",
            5 => "#aa00aa",
            6 => "#00aaaa",
            7 => "#aaaaaa",
            8 => "#555555",
            9 => "#ff5555",
            10 => "#55ff55",
            11 => "#ffff55",
            12 => "#5555ff",
            13 => "#ff55ff",
            14 => "#55ffff",
            _ => "#ffffff",
        }
        .into();
    }
    if i >= 232 {
        // Grayscale ramp.
        let v = 8 + (i - 232) * 10;
        return format!("#{:02x}{:02x}{:02x}", v, v, v);
    }
    // 6x6x6 color cube.
    let i = i - 16;
    let r = i / 36;
    let g = (i / 6) % 6;
    let b = i % 6;
    let to_byte = |v: u8| -> u8 {
        if v == 0 {
            0
        } else {
            55 + v * 40
        }
    };
    format!("#{:02x}{:02x}{:02x}", to_byte(r), to_byte(g), to_byte(b))
}

/// Escape a string for safe inclusion as SVG text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use ratatui::{layout::Rect, style::Modifier};

    use super::*;

    #[test]
    fn buffer_to_svg_renders_rect_text_and_colors() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        buf[(0u16, 0u16)].fg = Color::Rgb(0xff, 0x00, 0x00);
        buf[(0u16, 0u16)].bg = Color::Rgb(0x00, 0x00, 0xff);
        buf[(0u16, 0u16)].modifier = Modifier::BOLD;
        buf[(0u16, 0u16)].set_symbol("A");

        let svg = buffer_to_svg(&buf);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<rect"));
        assert!(svg.contains("#0000ff"));
        assert!(svg.contains("#ff0000"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains("font-weight:bold"));
    }

    #[test]
    fn buffer_to_svg_emits_well_formed_attributes() {
        // Regression guard: CSS font names must be single-quoted so the
        // double-quoted `style="..."` attribute stays valid XML. A raw `"`
        // mid-attribute would break SVG parsers (rsvg-convert, browsers).
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        buf[(0u16, 0u16)].fg = Color::Rgb(0xc0, 0xd0, 0xf0);
        buf[(0u16, 0u16)].set_symbol("Z");
        let svg = buffer_to_svg(&buf);

        assert!(
            svg.contains("font-family:'Symbols Nerd Font Mono'"),
            "font names should be single-quoted"
        );
        // No text element should carry a raw double-quote inside its style attr.
        for text_el in svg.split("<text").skip(1) {
            let attr_section = text_el.split('>').next().unwrap_or("");
            let style_value = attr_section
                .split("style=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or("");
            assert!(
                !style_value.contains('"'),
                "style attribute must not contain a raw double-quote: {attr_section}"
            );
        }
    }

    #[test]
    fn color_hex_reset_is_none() {
        assert!(color_hex(Color::Reset).is_none());
        assert_eq!(color_hex(Color::Rgb(1, 2, 3)).as_deref(), Some("#010203"));
    }

    #[test]
    fn xml_escape_escamps_ampersand() {
        assert_eq!(xml_escape("a&b<c>"), "a&amp;b&lt;c&gt;");
    }

    #[test]
    fn render_app_to_svg_produces_a_closed_svg() {
        use crate::app::App;
        use crate::config::Config;

        let mut app = App::new_sample(Config::default());
        let svg = super::render_app_to_svg(&mut app, 80, 40);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<rect"), "a rendered dashboard has bg rect");
    }
}
