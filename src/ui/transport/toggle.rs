use crate::ui::{
    icons::Icon,
    theme::muted_color,
};
use eframe::egui::*;

pub(super) fn show(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    width: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), selected, label)
    });
    let tint=if selected {Color32::from_rgb(239,232,242)}else{Color32::from_rgb(138,133,140)};
    icon.paint_control(ui,rect,tint);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            2,
            Stroke::new(1.0, muted_color()),
            StrokeKind::Inside,
        );
    }
    response
        .on_hover_cursor(CursorIcon::Default)
        .on_hover_text(label)
}
