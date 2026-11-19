//! Native entry point for the ATK HUB Rust GUI.
//!
//! `--self-check` renders every page offscreen and exits, so the GUI binary
//! can be verified without opening a window.

use atk_gui::{HubApp, Tab};
use egui::{Pos2, RawInput, Rect, Vec2};

fn main() -> eframe::Result<()> {
    if std::env::args().any(|argument| argument == "--self-check") {
        let mut app = HubApp::new();
        let context = egui::Context::default();
        for tab in Tab::ALL {
            app.set_tab(tab);
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 860.0))),
                ..Default::default()
            };
            let _output = context.run_ui(input, |ui| app.render(ui));
        }
        println!(
            "GUI self-check passed: {} pages rendered offscreen",
            Tab::ALL.len()
        );
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_title("ATK HUB — Rust port"),
        ..Default::default()
    };

    eframe::run_native(
        "ATK HUB (Rust port)",
        options,
        Box::new(|_creation_context| Ok(Box::new(HubApp::new()))),
    )
}
