use super::DIVIDER;
use crate::ui::{child, theme::*};
use eframe::egui::*;

pub(super) fn row(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui)) {
    let stacked = ui.available_width() < 360.0;
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), if stacked { 70.0 } else { 46.0 }),
        Sense::hover(),
    );
    let control_width = (rect.width() * 0.44).clamp(160.0, 250.0).min(rect.width());
    let controls = if stacked {
        Rect::from_min_max(rect.min + vec2(0.0, 30.0), rect.max - vec2(0.0, 8.0))
    } else {
        Rect::from_min_max(
            pos2(rect.right() - control_width, rect.top() + 7.0),
            rect.max - vec2(0.0, 7.0),
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
            Label::new(RichText::new(label).color(TEXT))
                .selectable(false)
                .wrap(),
        );
    });
    let mut control = child(ui, controls, ui.id().with((label, "control")));
    control.with_layout(Layout::right_to_left(Align::Center), content);
}

pub(super) fn toggle(ui: &mut Ui, label: &str, value: &mut bool) {
    row(ui, label, |ui| {
        let response = ui.checkbox(value, "");
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *value, label)
        });
    });
}

pub(super) fn fixed(ui: &mut Ui, label: &str, value: &str, reason: &str) {
    row(ui, label, |ui| {
        crate::ui::panels::controls::disabled_field(ui, value, true, reason);
    });
}

pub(super) fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(section_title_font()));
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, DIVIDER));
    });
    ui.add_space(5.0);
}

pub(super) fn speaker(ui: &Ui, rect: Rect) {
    let r = Rect::from_center_size(rect.center(), vec2(10.0, 16.0));
    ui.painter()
        .rect_stroke(r, 2, Stroke::new(1.0, MUTED), StrokeKind::Inside);
    for (offset, radius) in [(-4.0, 1.5), (3.0, 2.5)] {
        ui.painter().circle_stroke(
            r.center() + vec2(0.0, offset),
            radius,
            Stroke::new(1.0, MUTED),
        );
    }
}

pub(super) fn keyboard(ui: &Ui, rect: Rect) {
    let r = Rect::from_center_size(rect.center(), vec2(18.0, 11.0));
    ui.painter()
        .rect_stroke(r, 1, Stroke::new(1.0, MUTED), StrokeKind::Inside);
    for y in 0..2 {
        for x in 0..5 {
            ui.painter().circle_filled(
                r.min + vec2(3.0 + x as f32 * 3.0, 3.0 + y as f32 * 2.5),
                0.6,
                MUTED,
            );
        }
    }
    ui.painter().line_segment(
        [r.center() + vec2(-3.0, 3.0), r.center() + vec2(3.0, 3.0)],
        Stroke::new(1.0, MUTED),
    );
}
