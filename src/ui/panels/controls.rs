use crate::ui::theme::*;
use eframe::egui::*;

pub(super) fn padded(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    Frame::new().inner_margin(Margin{left:20,right:20,top:20,bottom:24}).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 12.0;
        content(ui);
    });
}

pub(super) fn section(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
    native::section(ui,("property-section",title),title,|_|{},content);
}

pub(super) fn field(ui: &mut Ui, text: &mut String) -> Response {
    ui.add_sized(
        vec2(ui.available_width(), native::control_height()),
        TextEdit::singleline(text)
            .vertical_align(Align::Center)
            .margin(vec2(7.0, 5.0))
            .background_color(border_color()),
    )
}

pub(super) fn pair(ui: &mut Ui, content: impl FnOnce(&mut Ui, f32)) {
    let width = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(1.0);
    ui.horizontal(|ui| content(ui, width));
}

pub(super) fn error(ui: &mut Ui, message: &str) {
    ui.colored_label(Color32::from_rgb(218, 142, 126), crate::i18n::message(message));
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
    crate::ui::theme::native::combo(id)
        .width(ui.available_width())
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (v, label) in options {
                ui.selectable_value(value, v.clone(), *label);
            }
        });
}
