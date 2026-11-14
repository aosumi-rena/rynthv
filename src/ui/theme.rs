use eframe::egui::*;
mod fonts;
pub use fonts::install as fonts;
pub const BACKGROUND: Color32 = Color32::from_rgb(27, 27, 29);
pub const PANEL: Color32 = Color32::from_rgb(45, 43, 46);
pub const CANVAS: Color32 = Color32::from_rgb(35, 35, 37);
pub const ROW: Color32 = Color32::from_rgb(39, 39, 41);
pub const BLACK_ROW: Color32 = Color32::from_rgb(31, 31, 33);
pub const GRID: Color32 = Color32::from_rgb(58, 57, 60);
pub const TEXT: Color32 = Color32::from_rgb(201, 199, 204);
pub const MUTED: Color32 = Color32::from_rgb(135, 133, 139);
pub const GREEN: Color32 = Color32::from_rgb(125, 178, 53);
pub const EDGE: Color32 = Color32::from_rgb(21, 21, 23);
pub const BORDER: Color32 = Color32::from_rgb(57, 55, 58);
pub const BUTTON: Color32 = Color32::from_rgb(68, 66, 71);
pub const BUTTON_HOVER: Color32 = Color32::from_rgb(78, 76, 81);
pub const BUTTON_ACTIVE: Color32 = Color32::from_rgb(83, 81, 87);
pub const ICON_TOOL: Color32 = Color32::from_rgb(181, 179, 186);

pub fn section_title_font() -> FontId {
    FontId::new(12.0, FontFamily::Name("SectionTitle".into()))
}

pub fn install(ctx: &Context) {
    let mut style = Style {
        visuals: Visuals::dark(),
        ..Default::default()
    };
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BACKGROUND;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgb(72, 91, 48);
    style.visuals.selection.stroke = Stroke::new(1.0, GREEN);
    style.visuals.widgets.inactive.bg_fill = BUTTON;
    style.visuals.widgets.inactive.weak_bg_fill = BUTTON;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(0.5, BORDER);
    style.visuals.widgets.hovered.bg_fill = BUTTON_HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = BUTTON_HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(0.5, BORDER);
    style.visuals.widgets.active.bg_fill = BUTTON_ACTIVE;
    style.visuals.widgets.active.weak_bg_fill = BUTTON_ACTIVE;
    style.visuals.widgets.active.bg_stroke = Stroke::new(0.5, BORDER);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(2);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(2);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(2);
    style.spacing.item_spacing = vec2(6.0, 6.0);
    style.spacing.button_padding = vec2(7.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(12.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(12.0));
    ctx.set_theme(Theme::Dark);
    ctx.set_style_of(Theme::Dark, style);
}
