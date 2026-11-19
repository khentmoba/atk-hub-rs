//! Visual system matching the light ATK GEAR interface of ATK HUB 3.2.x.
//!
//! Colours are taken from the running app's renderer: a light gray canvas,
//! white cards, dark text, black pill buttons and the ATK GEAR orange accent.

use egui::{
    Color32, CornerRadius, Frame, Id, Label, Layout, Margin, Rect, Response, RichText, Sense,
    Stroke, TextStyle, Ui, Vec2,
};

/// Colours sampled from the shipped ATK HUB renderer theme.
pub mod color {
    use super::Color32;

    /// Application canvas behind the cards.
    pub const BG: Color32 = Color32::from_rgb(245, 245, 247);
    /// Header bar.
    pub const CHROME: Color32 = Color32::from_rgb(255, 255, 255);
    /// Card surface.
    pub const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
    /// Card footer, secondary surface and hover.
    pub const RAISED: Color32 = Color32::from_rgb(232, 232, 236);
    /// Subtle card border.
    pub const BORDER: Color32 = Color32::from_rgb(224, 224, 228);
    /// Stronger border for selected controls.
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(198, 198, 204);
    /// Primary text.
    pub const TEXT: Color32 = Color32::from_rgb(26, 26, 28);
    /// Secondary text.
    pub const MUTED: Color32 = Color32::from_rgb(105, 105, 110);
    /// Hints and captions.
    pub const HINT: Color32 = Color32::from_rgb(150, 150, 156);
    /// ATK GEAR orange.
    pub const ACCENT: Color32 = Color32::from_rgb(255, 105, 0);
    /// Lighter orange used for accent text.
    pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(226, 92, 0);
    /// Success green.
    pub const SUCCESS: Color32 = Color32::from_rgb(47, 179, 90);
    /// Error red.
    pub const DANGER: Color32 = Color32::from_rgb(230, 62, 62);
    /// Black pill buttons used by the hub.
    pub const PILL: Color32 = Color32::from_rgb(26, 26, 28);
}

/// Apply the ATK HUB light theme to a context. Idempotent and cheap.
pub fn apply(ctx: &egui::Context) {
    let id = Id::new("atk-hub-theme-applied");
    if ctx.data(|data| data.get_temp::<bool>(id).unwrap_or(false)) {
        return;
    }

    let mut visuals = egui::Visuals::light();
    visuals.dark_mode = false;
    visuals.window_fill = color::CHROME;
    visuals.panel_fill = color::BG;
    visuals.faint_bg_color = color::RAISED;
    visuals.extreme_bg_color = color::SURFACE;
    visuals.text_edit_bg_color = Some(color::SURFACE);
    visuals.code_bg_color = color::RAISED;
    visuals.warn_fg_color = color::ACCENT_LIGHT;
    visuals.error_fg_color = color::DANGER;
    visuals.hyperlink_color = color::ACCENT_LIGHT;
    visuals.weak_text_color = Some(color::MUTED);
    visuals.button_frame = true;
    visuals.collapsing_header_frame = true;
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 105, 0, 55);
    visuals.selection.stroke = Stroke::new(1.0, color::ACCENT);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, color::BORDER);

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = color::SURFACE;
    w.noninteractive.weak_bg_fill = color::SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, color::BORDER);
    w.noninteractive.fg_stroke.color = color::TEXT;
    w.noninteractive.corner_radius = CornerRadius::same(8);

    w.inactive.bg_fill = color::RAISED;
    w.inactive.weak_bg_fill = color::RAISED;
    w.inactive.bg_stroke = Stroke::new(1.0, color::BORDER);
    w.inactive.fg_stroke.color = color::TEXT;
    w.inactive.corner_radius = CornerRadius::same(8);

    w.hovered.bg_fill = Color32::from_rgb(222, 222, 227);
    w.hovered.weak_bg_fill = color::RAISED;
    w.hovered.bg_stroke = Stroke::new(1.0, color::BORDER_STRONG);
    w.hovered.fg_stroke.color = color::TEXT;
    w.hovered.corner_radius = CornerRadius::same(8);

    w.active.bg_fill = Color32::from_rgb(212, 212, 218);
    w.active.weak_bg_fill = color::RAISED;
    w.active.bg_stroke = Stroke::new(1.0, color::BORDER_STRONG);
    w.active.fg_stroke.color = color::TEXT;
    w.active.corner_radius = CornerRadius::same(8);

    w.open.bg_fill = color::RAISED;
    w.open.fg_stroke.color = color::TEXT;

    ctx.set_theme(egui::Theme::Light);
    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(13.0, 7.0);
        style.text_styles = [
            (
                TextStyle::Heading,
                egui::FontId::new(20.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                egui::FontId::new(13.0, egui::FontFamily::Monospace),
            ),
            (
                TextStyle::Small,
                egui::FontId::new(12.0, egui::FontFamily::Proportional),
            ),
        ]
        .into();
    });
    ctx.data_mut(|data| data.insert_temp(id, true));
}

/// Header bar frame.
pub fn top_frame() -> Frame {
    Frame {
        inner_margin: Margin::symmetric(24, 10),
        fill: color::CHROME,
        stroke: Stroke::new(1.0, color::BORDER),
        corner_radius: CornerRadius::ZERO,
        ..Default::default()
    }
}

/// Main content frame.
pub fn content_frame() -> Frame {
    Frame {
        inner_margin: Margin {
            left: 28,
            right: 28,
            top: 22,
            bottom: 24,
        },
        fill: color::BG,
        corner_radius: CornerRadius::ZERO,
        ..Default::default()
    }
}

/// Rounded surface used to group related controls.
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    Frame {
        inner_margin: Margin::same(18),
        fill: color::SURFACE,
        stroke: Stroke::new(1.0, color::BORDER),
        corner_radius: CornerRadius::same(14),
        ..Default::default()
    }
    .show(ui, add_contents)
}

/// Page title with an accent underline.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add(Label::new(
        RichText::new(title).size(24.0).strong().color(color::TEXT),
    ));
    ui.add(Label::new(
        RichText::new(subtitle).size(13.0).color(color::MUTED),
    ));
    let bar = ui.allocate_space(Vec2::new(ui.available_width(), 2.0)).1;
    ui.painter().rect_filled(
        Rect::from_min_size(bar.min, Vec2::new(46.0, 3.0)),
        CornerRadius::same(2),
        color::ACCENT,
    );
    ui.add_space(14.0);
}

/// Section heading with the brand-colored leading bar.
pub fn h2(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        let bar = ui.allocate_space(Vec2::new(3.0, 17.0)).1;
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), color::ACCENT);
        ui.add(Label::new(
            RichText::new(text).size(16.0).strong().color(color::TEXT),
        ));
    });
    ui.add_space(2.0);
}

/// Black pill call-to-action button, as used by the hub.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(color::PILL)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(18))
            .min_size(Vec2::new(108.0, 32.0)),
    )
}

/// ATK GEAR wordmark with the orange lightning bolt, as in the app header.
pub fn brand(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(126.0, 30.0), Sense::hover());
    let painter = ui.painter();
    let bolt = [
        rect.min + Vec2::new(17.0, 2.0),
        rect.min + Vec2::new(6.0, 16.0),
        rect.min + Vec2::new(14.0, 16.0),
        rect.min + Vec2::new(9.0, 28.0),
        rect.min + Vec2::new(24.0, 11.0),
        rect.min + Vec2::new(16.0, 11.0),
    ];
    painter.add(egui::Shape::convex_polygon(
        bolt.to_vec(),
        color::ACCENT,
        Stroke::NONE,
    ));
    painter.text(
        rect.min + Vec2::new(30.0, 0.0),
        egui::Align2::LEFT_TOP,
        "ATK GEAR",
        egui::FontId::new(16.0, egui::FontFamily::Proportional),
        color::TEXT,
    );
}

/// Small navigation button in the header. Returns whether it was clicked.
pub fn nav_button(ui: &mut Ui, badge: &str, label: &str, selected: bool) -> bool {
    let response = ui.add(
        egui::Button::new(RichText::new(badge).size(12.0).strong())
            .fill(if selected {
                color::ACCENT
            } else {
                color::SURFACE
            })
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(26.0, 26.0)),
    );
    response.on_hover_text(label).clicked()
}

/// Right-aligned window controls. Returns clicked actions as (minimize, maximize, close).
fn control_button(ui: &mut Ui, kind: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), Sense::click());
    let bg = if response.hovered() {
        color::RAISED
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    let center = rect.center();
    let stroke = Stroke::new(1.4, color::MUTED);
    match kind {
        "min" => {
            ui.painter().line_segment(
                [center - Vec2::new(5.0, 0.0), center + Vec2::new(5.0, 0.0)],
                stroke,
            );
        }
        "max" => {
            ui.painter().rect_stroke(
                Rect::from_center_size(center, Vec2::new(11.0, 11.0)),
                CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        _ => {
            let d = 4.0;
            ui.painter().line_segment(
                [center + Vec2::new(-d, -d), center + Vec2::new(d, d)],
                stroke,
            );
            ui.painter().line_segment(
                [center + Vec2::new(d, -d), center + Vec2::new(-d, d)],
                stroke,
            );
        }
    }
    response.clicked()
}

/// Right-aligned window controls with the release pill.
pub fn window_controls(ui: &mut Ui) -> (bool, bool, bool) {
    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
        let close = control_button(ui, "close");
        let maximize = control_button(ui, "max");
        let minimize = control_button(ui, "min");
        ui.add_space(10.0);
        ui.add(Label::new(
            RichText::new("v3.2.31").size(11.0).color(color::HINT),
        ));
        (minimize, maximize, close)
    })
    .inner
}
