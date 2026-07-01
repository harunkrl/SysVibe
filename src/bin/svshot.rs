//! svshot — render SysVibe tabs to SVG via the real `ui::draw` path and emit
//! an HTML gallery for side-by-side UI/UX review.
//!
//! Build & run: `cargo run --bin svshot --features preview`
//! Options: `--tab <dashboard|system|hardware|processes|logs|gpu>`
//!          `--size <WxH>`   (e.g. 120x40)
//!          `--theme <name>` (default catppuccin-macchiato)

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
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

    let mut gallery: Vec<(String, String)> = Vec::new(); // (title, filename)
    let mut failures: Vec<(String, String)> = Vec::new(); // (title, reason)

    for (tab, tab_name) in TABS {
        if let Some(ref want) = want_tab {
            if want != tab_name {
                continue;
            }
        }
        for (w, h, size_name) in SIZES {
            if let Some(ref want) = want_size {
                if want != &format!("{}x{}", w, h) {
                    continue;
                }
            }
            let title = format!("{} · {} ({}×{})", tab_name, size_name, w, h);

            // Fresh app per render so a panic in one size can never corrupt a
            // later render. catch_unwind isolates per-size render panics
            // (e.g. a widget that overflows its rect at very narrow widths) so
            // the whole gallery is still produced and the failure is reported.
            let mut app = App::new_sample(config.clone());
            app.tab = tab;
            let result = catch_unwind(AssertUnwindSafe(|| {
                render_app_to_svg(&mut app, w, h)
            }));

            match result {
                Ok(svg) => {
                    let filename = format!("{}_{}.svg", tab_name, size_name);
                    fs::write(out_dir.join(&filename), svg).expect("write svg");
                    gallery.push((title, filename));
                }
                Err(panic) => {
                    let reason = panic_message(&panic);
                    eprintln!("✗ FAILED to render {} — {}", title, reason);
                    failures.push((title, reason));
                }
            }
        }
    }

    fs::write(out_dir.join("index.html"), render_index_html(&gallery, &failures))
        .expect("write index.html");

    println!(
        "✓ Wrote {} SVG(s) + index.html to {}",
        gallery.len(),
        out_dir.display()
    );
    if !failures.is_empty() {
        println!(
            "⚠ {} render(s) failed (see above); likely narrow-width widget bugs.",
            failures.len()
        );
    }
    println!(
        "  Open {} in a browser.",
        out_dir.join("index.html").display()
    );
}

/// Best-effort extraction of a message from a panic payload.
fn panic_message(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = p.downcast_ref::<String>() {
        return s.clone();
    }
    "<non-string panic payload>".to_string()
}

fn render_index_html(gallery: &[(String, String)], failures: &[(String, String)]) -> String {
    let mut html = String::from(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>SysVibe — Preview Gallery</title>\n<style>\n\
         body{background:#1e1e2e;color:#cdd6f4;font-family:system-ui,sans-serif;margin:24px}\n\
         h1{color:#cba6f7}\n\
         figure{margin:18px 0}\n\
         figcaption{color:#89b4fa;font-weight:600;margin-bottom:6px}\n\
         img{background:#1e1e2e;border:1px solid #45475a;display:block;max-width:100%}\n\
         .fails{background:#313244;border:1px solid #f38ba8;padding:12px 16px;margin-bottom:20px}\n\
         .fails h2{color:#f38ba8;margin:0 0 8px}\n\
         .fails li{color:#f9e2af;font-family:monospace;margin:4px 0}\n\
         .fail-reason{color:#a6adc8}\n\
         </style>\n</head>\n<body>\n<h1>SysVibe — Preview Gallery</h1>\n",
    );
    if !failures.is_empty() {
        html.push_str("<div class=\"fails\"><h2>⚠ Render failures (panics)</h2><ul>\n");
        for (title, reason) in failures {
            html.push_str(&format!(
                "<li>{} <span class=\"fail-reason\">— {}</span></li>\n",
                title, reason
            ));
        }
        html.push_str("</ul></div>\n");
    }
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
