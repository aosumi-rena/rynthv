use crate::{
    model::RetakeComponent,
    ui::{icons::Icon, theme::*},
};
use eframe::egui::*;

pub(super) fn show(ui: &mut Ui, duration_unavailable: Option<&str>) -> Option<RetakeComponent> {
    let width = ((ui.available_width() - 4.0) * 0.5).max(1.0);
    let mut clicked = None;
    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
    for row in [
        [RetakeComponent::All, RetakeComponent::Duration],
        [RetakeComponent::Pitch, RetakeComponent::Timbre],
    ] {
        ui.horizontal(|ui| {
            for component in row {
                let reason = if component == RetakeComponent::Duration {
                    duration_unavailable
                } else {
                    None
                };
                let response = ui
                    .add_enabled_ui(reason.is_none(), |ui| button(ui, width, component))
                    .inner;
                if response.clicked() {
                    clicked = Some(component);
                } else if let Some(reason) = reason {
                    response.on_disabled_hover_text(crate::i18n::message(&reason));
                }
            }
        });
    }
    clicked
}

fn button(ui: &mut Ui, width: f32, component: RetakeComponent) -> Response {
    let enabled = ui.is_enabled();
    let label = match component {
        RetakeComponent::All | RetakeComponent::PitchAndTimbre => crate::i18n::t("全部"),
        RetakeComponent::Duration => crate::i18n::t("时值"),
        RetakeComponent::Pitch => crate::i18n::t("音高"),
        RetakeComponent::Timbre => crate::i18n::t("音色"),
    };
    let (rect, response) = ui.allocate_exact_size(
        vec2(width, 88.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    let hovered = response.hovered() && enabled;
    ui.painter().rect(
        rect,
        3,
        if response.is_pointer_button_down_on() {
            BUTTON_ACTIVE
        } else if hovered {
            BUTTON_HOVER
        } else {
            BORDER
        },
        Stroke::new(0.6, BUTTON),
        StrokeKind::Inside,
    );
    let ink = if enabled { TEXT } else { MUTED };
    let center = pos2(rect.center().x, rect.top() + 34.0);
    match component {
        RetakeComponent::Duration | RetakeComponent::Pitch => {
            let icon = if component == RetakeComponent::Duration {
                Icon::Phonemes
            } else {
                Icon::Notes
            };
            icon.paint(ui, Rect::from_center_size(center, Vec2::splat(25.0)), ink);
        }
        RetakeComponent::All | RetakeComponent::PitchAndTimbre => sparkles(ui, center, ink),
        RetakeComponent::Timbre => lips(ui, center, ink),
    }
    ui.painter().text(
        pos2(center.x, rect.bottom() - 12.0),
        Align2::CENTER_BOTTOM,
        label,
        FontId::proportional(12.0),
        ink,
    );
    response.on_hover_text(crate::lformat!("重录{label}采样"))
}

fn lips(ui: &Ui, center: Pos2, ink: Color32) {
    let contour = [
        (-11.0, -1.0),
        (-5.0, -6.0),
        (0.0, -4.0),
        (5.0, -6.0),
        (11.0, -1.0),
        (7.0, 5.0),
        (0.0, 8.0),
        (-7.0, 5.0),
        (-11.0, -1.0),
    ];
    ui.painter().add(Shape::line(
        contour
            .into_iter()
            .map(|(x, y)| center + vec2(x, y))
            .collect(),
        Stroke::new(1.3, ink),
    ));
    ui.painter().add(Shape::line(
        [
            (-10.0, -1.0),
            (-4.0, 1.0),
            (0.0, 0.0),
            (4.0, 1.0),
            (10.0, -1.0),
        ]
        .into_iter()
        .map(|(x, y)| center + vec2(x, y))
        .collect(),
        Stroke::new(1.2, ink),
    ));
}

fn sparkles(ui: &Ui, center: Pos2, ink: Color32) {
    for (offset, radius) in [
        (vec2(-4.0, 0.0), 9.0),
        (vec2(8.0, -8.0), 3.5),
        (vec2(9.0, 8.0), 3.0),
    ] {
        let center = center + offset;
        let outline = [
            (0.0, -1.0),
            (0.3, -0.3),
            (1.0, 0.0),
            (0.3, 0.3),
            (0.0, 1.0),
            (-0.3, 0.3),
            (-1.0, 0.0),
            (-0.3, -0.3),
            (0.0, -1.0),
        ];
        ui.painter().add(Shape::line(
            outline
                .into_iter()
                .map(|(x, y)| center + vec2(x, y) * radius)
                .collect(),
            Stroke::new(1.3, ink),
        ));
    }
}
