use crate::ui::{icons::Icon, theme::*};
use eframe::egui::*;

const CONTROL_FILL: Color32 = Color32::from_rgb(58, 56, 59);

pub(super) fn field(ui: &mut Ui, value: &str, dropdown: bool, reason: &str) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0),Sense::hover());
    ui.painter().rect(
        rect,
        4,
        CONTROL_FILL,
        Stroke::new(0.6, BUTTON),
        StrokeKind::Inside,
    );
    let text_rect = Rect::from_min_max(
        rect.min + vec2(8.0, 0.0),
        rect.max - vec2(if dropdown { 24.0 } else { 8.0 }, 0.0),
    );
    ui.painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
        .text(
            text_rect.left_center(),
            Align2::LEFT_CENTER,
            value,
            FontId::proportional(12.0),
            TEXT,
        );
    if dropdown {
        Icon::ChevronDown.paint(
            ui,
            Rect::from_center_size(rect.right_center() - vec2(17.0, 0.0), Vec2::splat(12.0)),
            MUTED,
        );
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox,false,value));
    response
        .on_hover_cursor(CursorIcon::NotAllowed)
        .on_hover_text(crate::i18n::message(reason));
}

const POPUP_KNOB: Color32 = Color32::from_rgb(65, 63, 68);

pub(super) fn slider(ui: &mut Ui, label: &str) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 51.0), Sense::hover());
    let painter = ui.painter();
    painter.text(
        rect.min + vec2(0.0, 4.0),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(12.0),
        MUTED,
    );
    let y = rect.top() + 32.0;
    painter.hline(rect.x_range(), y, Stroke::new(2.0, EDGE));
    let center = pos2(rect.center().x, y);
    painter.circle(center, 9.5, EDGE, Stroke::NONE);
    painter.circle(center, 8.5, POPUP_KNOB, Stroke::new(0.7, BUTTON_ACTIVE));
    painter.circle_filled(center, 3.0, TEXT);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, false, label));
    response
        .on_hover_cursor(CursorIcon::NotAllowed)
        .on_hover_text(crate::lformat!("{label}调节尚未接入合成"));
}
