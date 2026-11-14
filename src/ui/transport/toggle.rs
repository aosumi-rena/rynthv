use crate::ui::{
    icons::Icon,
    theme::{MUTED, PANEL},
};
use eframe::egui::*;

pub(super) fn show(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    width: f32,
    icon_size: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), selected, label)
    });
    ui.painter().rect_filled(
        rect,
        CornerRadius {
            nw: 0,
            ne: 3,
            sw: 0,
            se: 3,
        },
        PANEL,
    );
    let tint = if selected {
        Color32::from_rgb(235, 232, 240)
    } else {
        MUTED
    };
    icon.paint(
        ui,
        Rect::from_center_size(rect.center(), Vec2::splat(icon_size)),
        tint,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            2,
            Stroke::new(1.0, MUTED),
            StrokeKind::Inside,
        );
    }
    response
        .on_hover_cursor(CursorIcon::Default)
        .on_hover_text(label)
}
