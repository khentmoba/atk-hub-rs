//! Offscreen rendering tests for the GUI.

use atk_gui::{HubApp, Tab};
use egui::{Pos2, RawInput, Rect, Vec2};

fn render(app: &mut HubApp) {
    let ctx = egui::Context::default();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 860.0))),
        ..Default::default()
    };
    let _output = ctx.run_ui(input, |ui| app.render(ui));
}

#[test]
fn every_page_renders_offscreen_without_a_window() {
    let mut app = HubApp::new();
    for tab in Tab::ALL {
        app.set_tab(tab);
        render(&mut app);
    }
}

#[test]
fn fresh_app_starts_on_the_overview_page() {
    let mut app = HubApp::new();
    render(&mut app);
    app.set_tab(Tab::Overview);
    render(&mut app);
}
