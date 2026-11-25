use crate::ui::{icons::Icon, theme::*};
use eframe::egui::*;

pub(crate) fn padded(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    Frame::new().inner_margin(16).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 12.0;
        content(ui);
    });
}

pub(crate) fn section(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.next_widget_position().y,
        Stroke::new(1.0, EDGE),
    );
    padded(ui, |ui| {
        CollapsingHeader::new(RichText::new(title).font(section_title_font()))
            .id_salt(("property-section", title))
            .default_open(true)
            .show_unindented(ui, |ui| {
                ui.set_width(ui.available_width());
                content(ui);
            });
    });
}

pub(crate) fn field(ui: &mut Ui, text: &mut String) -> Response {
    ui.add_sized(
        vec2(ui.available_width(), 28.0),
        TextEdit::singleline(text)
            .vertical_align(Align::Center)
            .margin(vec2(7.0, 5.0))
            .background_color(BORDER),
    )
}

pub(super) fn pair(ui: &mut Ui, content: impl FnOnce(&mut Ui, f32)) {
    let width = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(1.0);
    ui.horizontal(|ui| content(ui, width));
}

pub(super) fn error(ui: &mut Ui, message: &str) {
    ui.colored_label(
        Color32::from_rgb(218, 142, 126),
        crate::i18n::message(message),
    );
}

pub(super) fn combo<T: PartialEq + Clone>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    let label = options
        .iter()
        .find(|(v, _)| v == value)
        .map_or(crate::i18n::t("未支持的值"), |(_, label)| *label);
    ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (v, label) in options {
                ui.selectable_value(value, v.clone(), *label);
            }
        });
}

const CONTROL_FILL: Color32 = Color32::from_rgb(58, 56, 59);

pub(crate) fn disabled_field(ui: &mut Ui, value: &str, dropdown: bool, reason: &str) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
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
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, false, value));
    response
        .on_hover_cursor(CursorIcon::NotAllowed)
        .on_hover_text(crate::i18n::message(reason));
}
