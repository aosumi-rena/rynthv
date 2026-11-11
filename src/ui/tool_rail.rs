use super::{
    child,
    icons::{Icon, IconButton},
    state::Column,
    theme::*,
};
use crate::app::App;
use eframe::egui::*;

pub fn width() -> f32 { metrics().rail_width }

pub fn show(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();

    ui.painter().rect_filled(rect.shrink(metrics().surface_inset), metrics().surface_radius, panel_color());
    ui.painter()
        .rect_stroke(rect.shrink(metrics().surface_inset), metrics().surface_radius, Stroke::new(1.0, border_color()), StrokeKind::Inside);

    if current() == InterfaceTheme::Classic {
        Icon::Brand.paint(ui, Rect::from_center_size(pos2(rect.center().x, rect.top() + 26.0), vec2(27.0, 27.0)), Color32::WHITE);
    }
    let layout = metrics();
    let content_rect = Rect::from_min_max(
        rect.min + vec2(layout.rail_horizontal_padding, layout.rail_top_padding),
        rect.max - vec2(layout.rail_horizontal_padding, 5.0),
    );
    let mut content = child(ui, content_rect, "rail-content");
    content.spacing_mut().item_spacing.y = layout.rail_button_spacing;
    for panel in Column::PANELS {
        if content
            .add(button(panel.icon(), panel.label()).selected(app.view.columns.contains(&panel)))
            .clicked()
        {
            app.view.toggle(panel);
        }
    }

    content.add_space(layout.rail_separator_before);
    let (separator, _) = content.allocate_exact_size(vec2(rect.width() - layout.rail_horizontal_padding * 2.0, 1.0), Sense::hover());
    content.painter().hline(
        separator.x_range(),
        separator.center().y,
        Stroke::new(1.0, edge_color()),
    );
    content.add_space(5.0);
    if content.add(button(Icon::Cloud, crate::i18n::t("管理歌声"))).clicked() {
        app.voice_manager = true;
    }
    if content.add(button(Icon::Settings, crate::i18n::t("设置"))).clicked() {
        app.settings = true;
    }
}

fn button(icon: Icon, label: &'static str) -> IconButton {
    IconButton::new(icon, label)
        .rail()
        .size(Vec2::splat(metrics().rail_button_size))
        .icon_size(20.0)
}
