use crate::{
    model::RetakeComponent,
    ui::icons::Icon,
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
    let opacity = ui.opacity();
    ui.set_opacity(1.0);
    ui.painter().rect(
        rect, 3,
        Color32::from_white_alpha(if hovered {31} else {18}),
        Stroke::new(1.0,Color32::from_white_alpha(18)),
        StrokeKind::Inside,
    );
    let ink = Color32::from_rgba_unmultiplied(163,157,166,if enabled {255}else{128});
    let icon = match component {
        RetakeComponent::All | RetakeComponent::PitchAndTimbre => Icon::AllRetakes,
        RetakeComponent::Duration => Icon::TimingRetake,
        RetakeComponent::Pitch => Icon::PitchRetake,
        RetakeComponent::Timbre => Icon::TimbreRetake,
    };
    icon.paint_control(ui, rect, ink);
    ui.painter().text(
        pos2(rect.center().x, rect.bottom() - 18.0),
        Align2::CENTER_CENTER,
        label,
        crate::ui::theme::native_font(18.0),
        ink,
    );
    ui.set_opacity(opacity);
    response.on_hover_text(crate::lformat!("重录{label}采样"))
}

