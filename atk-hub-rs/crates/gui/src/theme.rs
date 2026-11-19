//! Visual system for the ATK HUB-style interface.
//!
//! The palette is taken from the installed ATK HUB 3.2.31 renderer bundle
//! (`resources/main/app.asar` → `static/bootstrap-*.css`, theme `:root`):
//! near-black `bg-100/bg-default` surfaces, translucent white borders, and
//! the brand orange `--color-brand-primary-2: #ff6900` with its coral
//! variant `rgba(244, 119, 91, *)`.

use egui::{
    Color32, CornerRadius, Frame, Id, Label, Layout, Margin, Rect, Response, RichText, Sense,
    Stroke, TextStyle, Ui, Vec2, WidgetText,
};

/// Colors sampled from the shipped ATK HUB renderer theme.
pub mod color {
    use super::Color32;

    /// `--bg-100` — deepest application background.
    pub const BG: Color32 = Color32::from_rgb(10, 10, 12);
    /// `--bg-default` — panel / chrome background.
    pub const CHROME: Color32 = Color32::from_rgb(15, 15, 17);
    /// `--bg-300` — card surface.
    pub const SURFACE: Color32 = Color32::from_rgb(19, 20, 25);
    /// `--bg-500` — raised surface (buttons, inputs, hovered rows).
    pub const RAISED: Color32 = Color32::from_rgb(26, 27, 32);
    /// Subtle border colour (a translucent white over the dark surfaces).
    pub const BORDER: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20);
    /// Stronger border for selected controls.
    pub const BORDER_STRONG: Color32 = Color32::from_rgba_premultiplied(42, 42, 42, 42);
    /// `--greys-100` — primary text.
    pub const TEXT: Color32 = Color32::from_rgb(238, 240, 241);
    /// `--greys-400` — secondary text.
    pub const MUTED: Color32 = Color32::from_rgb(154, 164, 173);
    /// `--greys-600` — hints and captions.
    pub const HINT: Color32 = Color32::from_rgb(100, 114, 124);
    /// `--color-brand-primary-2` — brand orange.
    pub const ACCENT: Color32 = Color32::from_rgb(255, 105, 0);
    /// Lighter orange used for accent text.
    pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(255, 149, 75);
    /// `--color-board-orange-65` — coral accent.
    pub const CORAL: Color32 = Color32::from_rgb(244, 119, 91);
    /// `--color-function-success`.
    pub const SUCCESS: Color32 = Color32::from_rgb(47, 203, 100);
    /// `--color-function-error`.
    pub const DANGER: Color32 = Color32::from_rgb(255, 77, 79);
}

/// Apply the ATK HUB dark/orange theme to a context. Idempotent and cheap.
pub fn apply(ctx: &egui::Context) {
    let id = Id::new("atk-hub-theme-applied");
    if ctx.data(|data| data.get_temp::<bool>(id).unwrap_or(false)) {
        return;
    }

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.window_fill = color::CHROME;
    visuals.panel_fill = color::BG;
    visuals.faint_bg_color = color::SURFACE;
    visuals.extreme_bg_color = color::BG;
    visuals.text_edit_bg_color = Some(color::BG);
    visuals.code_bg_color = color::RAISED;
    visuals.warn_fg_color = color::ACCENT_LIGHT;
    visuals.error_fg_color = color::DANGER;
    visuals.hyperlink_color = color::ACCENT_LIGHT;
    visuals.weak_text_color = Some(color::MUTED);
    visuals.button_frame = true;
    visuals.collapsing_header_frame = true;
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 105, 0, 70);
    visuals.selection.stroke = Stroke::new(1.0, color::ACCENT);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, color::BORDER);

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = color::SURFACE;
    w.noninteractive.weak_bg_fill = color::SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, color::BORDER);
    w.noninteractive.fg_stroke.color = color::TEXT;
    w.noninteractive.corner_radius = CornerRadius::same(10);

    w.inactive.bg_fill = color::RAISED;
    w.inactive.weak_bg_fill = color::RAISED;
    w.inactive.bg_stroke = Stroke::new(1.0, color::BORDER);
    w.inactive.fg_stroke.color = color::TEXT;
    w.inactive.corner_radius = CornerRadius::same(8);

    w.hovered.bg_fill = Color32::from_rgba_unmultiplied(255, 105, 0, 45);
    w.hovered.weak_bg_fill = color::RAISED;
    w.hovered.bg_stroke = Stroke::new(1.0, color::ACCENT);
    w.hovered.fg_stroke.color = Color32::WHITE;
    w.hovered.corner_radius = CornerRadius::same(8);

    w.active.bg_fill = Color32::from_rgba_unmultiplied(255, 105, 0, 80);
    w.active.weak_bg_fill = color::RAISED;
    w.active.bg_stroke = Stroke::new(1.0, color::ACCENT);
    w.active.fg_stroke.color = Color32::WHITE;
    w.active.corner_radius = CornerRadius::same(8);

    w.open.bg_fill = color::RAISED;
    w.open.fg_stroke.color = color::TEXT;

    ctx.set_theme(egui::Theme::Dark);
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

/// Title-bar frame.
pub fn top_frame() -> Frame {
    Frame {
        inner_margin: Margin::symmetric(20, 10),
        fill: color::CHROME,
        stroke: Stroke::new(1.0, color::BORDER),
        corner_radius: CornerRadius::ZERO,
        ..Default::default()
    }
}

/// Left navigation frame.
pub fn side_frame() -> Frame {
    Frame {
        inner_margin: Margin {
            left: 14,
            right: 14,
            top: 18,
            bottom: 16,
        },
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
            left: 26,
            right: 26,
            top: 22,
            bottom: 26,
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
        corner_radius: CornerRadius::same(12),
        ..Default::default()
    }
    .show(ui, add_contents)
}

/// Compact card with tighter padding, used for stat tiles.
pub fn tile<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    Frame {
        inner_margin: Margin::same(14),
        fill: color::RAISED,
        stroke: Stroke::new(1.0, color::BORDER),
        corner_radius: CornerRadius::same(10),
        ..Default::default()
    }
    .show(ui, add_contents)
}

/// Page title with an accent underline.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add(Label::new(
        RichText::new(title).size(25.0).strong().color(color::TEXT),
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

/// Secondary, quieter heading.
pub fn h3(ui: &mut Ui, text: impl Into<WidgetText>) {
    ui.add(Label::new(text.into().color(color::ACCENT_LIGHT).strong()));
}

/// Muted caption / status text.
pub fn muted(ui: &mut Ui, text: impl Into<WidgetText>) {
    ui.add(Label::new(text.into().color(color::MUTED)).wrap());
}

/// Prominent orange call-to-action button.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(color::ACCENT)
            .stroke(Stroke::new(1.0, color::ACCENT_LIGHT))
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(108.0, 32.0)),
    )
}

/// Small uppercase group caption for the sidebar.
pub fn nav_caption(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.add(Label::new(
        RichText::new(text).size(10.0).strong().color(color::HINT),
    ));
    ui.add_space(2.0);
}

/// Sidebar navigation entry. Returns whether it was clicked.
pub fn nav_button(ui: &mut Ui, badge: &str, label: &str, selected: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let bg = if selected {
        Color32::from_rgba_unmultiplied(255, 105, 0, 34)
    } else if response.hovered() {
        color::RAISED
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(
                rect.min + Vec2::new(0.0, 5.0),
                Vec2::new(3.0, rect.height() - 10.0),
            ),
            CornerRadius::same(2),
            color::ACCENT,
        );
    }
    let badge_rect = Rect::from_min_size(rect.min + Vec2::new(12.0, 9.0), Vec2::new(18.0, 18.0));
    let badge_fill = if selected {
        color::ACCENT
    } else {
        color::RAISED
    };
    let badge_text = if selected {
        Color32::WHITE
    } else {
        color::MUTED
    };
    ui.painter()
        .rect_filled(badge_rect, CornerRadius::same(5), badge_fill);
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        badge,
        egui::FontId::new(11.0, egui::FontFamily::Proportional),
        badge_text,
    );
    let text_color = if selected {
        Color32::WHITE
    } else {
        color::TEXT
    };
    ui.painter().text(
        rect.left_center() + Vec2::new(42.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::new(13.5, egui::FontFamily::Proportional),
        text_color,
    );

    response.clicked()
}

/// A small square color swatch.
pub fn swatch(ui: &mut Ui, color: Color32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect.shrink(2.0), CornerRadius::same(6), color);
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        CornerRadius::same(6),
        Stroke::new(1.0, color::BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
}

/// Draw the bundled ATK HUB mark at a fixed size. The PNG is decoded once and
/// uploaded as a texture; egui has no default image loader for `include_image`.
pub fn logo(ui: &mut Ui, size: Vec2) {
    let ctx = ui.ctx();
    let id = Id::new("atk-hub-logo-texture");
    let handle = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id));
    let handle = handle.unwrap_or_else(|| {
        let image = image::load_from_memory(include_bytes!("../../../../assets/atk-hub-logo.png"))
            .expect("bundled ATK HUB logo is a valid PNG")
            .to_rgba8();
        let width = image.width() as usize;
        let height = image.height() as usize;
        let pixels: Vec<Color32> = image
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let handle = ctx.load_texture(
            "atk-hub-logo",
            egui::epaint::ColorImage::new([width, height], pixels),
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|data| data.insert_temp(id, handle.clone()));
        handle
    });
    let [width, height] = handle.size();
    let sized = egui::load::SizedTexture::new(handle.id(), Vec2::new(width as f32, height as f32));
    ui.add(egui::Image::from_texture(sized).fit_to_exact_size(size));
}

/// Draw one window-control button from vector primitives (font independent).
fn control_button(ui: &mut Ui, kind: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::click());
    let bg = if response.hovered() {
        color::RAISED
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    let center = rect.center();
    let stroke = Stroke::new(
        1.4,
        if response.hovered() {
            color::TEXT
        } else {
            color::MUTED
        },
    );
    match kind {
        "min" => {
            ui.painter().line_segment(
                [center - Vec2::new(5.0, 0.0), center + Vec2::new(5.0, 0.0)],
                stroke,
            );
        }
        "max" => {
            let box_rect = Rect::from_center_size(center, Vec2::new(11.0, 11.0));
            ui.painter().rect_stroke(
                box_rect,
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

/// Right-aligned window controls. Returns clicked actions as (minimize, maximize, close).
pub fn window_controls(ui: &mut Ui) -> (bool, bool, bool) {
    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
        let close = control_button(ui, "close");
        let maximize = control_button(ui, "max");
        let minimize = control_button(ui, "min");
        ui.add_space(12.0);
        ui.add(Label::new(
            RichText::new("v3.2.31").size(11.0).color(color::HINT),
        ));
        (minimize, maximize, close)
    })
    .inner
}
