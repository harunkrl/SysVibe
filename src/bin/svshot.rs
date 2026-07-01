//! svshot — render SysVibe tabs to SVG via the real `ui::draw` path and emit
//! an HTML gallery for side-by-side UI/UX review.
//!
//! Build & run: `cargo run --bin svshot --features preview`
//! Options: `--tab <dashboard|system|hardware|processes|logs|gpu>`
//!          `--size <WxH>`   (e.g. 120x40)
//!          `--theme <name>` (default catppuccin-macchiato)

use std::fs;
use std::path::PathBuf;

use sysvibe::app::state::AppTab;
use sysvibe::app::App;
use sysvibe::config::Config;
use sysvibe::ui::palette;
use sysvibe::ui::preview::render_app_to_svg;

const TABS: [(AppTab, &str); 6] = [
    (AppTab::Dashboard, "dashboard"),
    (AppTab::System, "system"),
    (AppTab::Hardware, "hardware"),
    (AppTab::Processes, "processes"),
    (AppTab::Logs, "logs"),
    (AppTab::Gpu, "gpu"),
];

const SIZES: [(u16, u16, &str); 3] = [(120, 40, "wide"), (80, 40, "narrow"), (60, 24, "compact")];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let theme = arg_value(&args, "--theme").unwrap_or_else(|| "catppuccin-macchiato".into());

    let mut config = Config::default();
    config.theme = theme;
    config.nerd_fonts = true; // match the real default look
    palette::load_and_apply(&config.theme);

    let want_tab = arg_value(&args, "--tab");
    let want_size = arg_value(&args, "--size");

    let out_dir = PathBuf::from("docs/preview");
    fs::create_dir_all(&out_dir).expect("create docs/preview");

    let mut app = App::new_sample(config);
    let mut gallery: Vec<(String, String)> = Vec::new(); // (title, filename)

    for (tab, tab_name) in TABS {
        if let Some(ref want) = want_tab {
            if want != tab_name {
                continue;
            }
        }
        app.tab = tab;
        for (w, h, size_name) in SIZES {
            if let Some(ref want) = want_size {
                if want != &format!("{}x{}", w, h) {
                    continue;
                }
            }
            let filename = format!("{}_{}.svg", tab_name, size_name);
            let svg = render_app_to_svg(&mut app, w, h);
            fs::write(out_dir.join(&filename), svg).expect("write svg");
            let title = format!("{} · {} ({}×{})", tab_name, size_name, w, h);
            gallery.push((title, filename));
        }
    }

    fs::write(out_dir.join("index.html"), render_index_html(&gallery)).expect("write index.html");

    println!(
        "✓ Wrote {} SVG(s) + index.html to {}",
        gallery.len(),
        out_dir.display()
    );
    println!(
        "  Open {} in a browser.",
        out_dir.join("index.html").display()
    );
}

fn render_index_html(gallery: &[(String, String)]) -> String {
    let mut html = String::from(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>SysVibe — Preview Gallery</title>\n<style>\n\
         body{background:#1e1e2e;color:#cdd6f4;font-family:system-ui,sans-serif;margin:24px}\n\
         h1{color:#cba6f7}\n\
         figure{margin:18px 0}\n\
         figcaption{color:#89b4fa;font-weight:600;margin-bottom:6px}\n\
         img{background:#1e1e2e;border:1px solid #45475a;display:block;max-width:100%}\n\
         </style>\n</head>\n<body>\n<h1>SysVibe — Preview Gallery</h1>\n",
    );
    for (title, filename) in gallery {
        html.push_str(&format!(
            "<figure>\n<figcaption>{}</figcaption>\n<img src=\"{}\" alt=\"{}\">\n</figure>\n",
            title, filename, title
        ));
    }
    html.push_str("</body>\n</html>\n");
    html
}

/// Read a `--flag value` (or `--flag=value`) argument from the CLI.
fn arg_value(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == flag {
            return iter.next().cloned();
        }
        if let Some(rest) = a.strip_prefix(&format!("{}=", flag)) {
            return Some(rest.to_string());
        }
    }
    None
}
