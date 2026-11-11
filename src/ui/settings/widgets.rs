use crate::ui::{child, icons::Icon, theme::*};
use eframe::egui::*;

pub(super) fn row(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui)) {
    let stacked = ui.available_width() < 360.0;
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), if stacked { 70.0 } else { 40.0 }),
        Sense::hover(),
    );
    let control_width = (rect.width() * 0.44).clamp(160.0, 250.0).min(rect.width());
    let controls = if stacked {
        Rect::from_min_max(rect.min + vec2(0.0, 30.0), rect.max - vec2(0.0, 8.0))
    } else {
        Rect::from_min_max(
            pos2(rect.right() - control_width, rect.top() + 4.0),
            rect.max - vec2(0.0, 4.0),
        )
    };
    let label_rect = if stacked {
        Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 26.0))
    } else {
        Rect::from_min_max(rect.min, pos2(controls.left() - 12.0, rect.bottom()))
    };
    let mut title = child(ui, label_rect, ui.id().with((label, "label")));
    title.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.add(
            Label::new(RichText::new(label).color(Color32::from_rgb(182,182,191)))
                .selectable(false)
                .wrap(),
        );
    });
    let mut control = child(ui, controls, ui.id().with((label, "control")));
    control.with_layout(Layout::right_to_left(Align::Center), content);
}

pub(super) fn toggle(ui: &mut Ui, label: &str, value: &mut bool) {
    row(ui, label, |ui| {
        let response=crate::ui::theme::native::settings_checkbox(ui,value,"");
        response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, *value, label));
    });
}

pub(super) fn fixed(ui: &mut Ui, label: &str, value: &str, reason: &str) {
    row(ui, label, |ui| {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
        ui.painter().rect(
            rect,
            4,
            palette().settings_control,
            Stroke::new(0.6, button_active_color()),
            StrokeKind::Inside,
        );
        ui.painter()
            .with_clip_rect(rect.shrink2(vec2(8.0, 0.0)).intersect(ui.clip_rect()))
            .text(
                rect.left_center() + vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                value,
                FontId::new(13.0, FontFamily::Name("NativeSettings".into())),
                muted_color(),
            );
        Icon::SettingsDropdown.paint(
            ui,
            Rect::from_center_size(rect.right_center() - vec2(17.0, 0.0), vec2(10.0,6.0)),
            muted_color(),
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, false, label));
        response.on_hover_text(reason);
    });
}

pub(super) fn section(ui: &mut Ui, title: &str) {
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(13.0).color(Color32::from_rgb(237,237,242)));
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, Color32::from_white_alpha(26)));
    });
    ui.add_space(16.0);
}

