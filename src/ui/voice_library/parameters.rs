use crate::{app::App, synthesis::voice::VoiceFamily, ui::theme::*};
use crate::synthesis::parameters::{ParameterSpec,VOICE_CONTROLS};
use eframe::egui::*;
use super::widgets;

pub(super) fn show(app: &mut App, ui: &mut Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    for spec in VOICE_CONTROLS {control(app,ui,spec);}
}
pub(super) fn control(app:&mut App,ui:&mut Ui,mut spec:ParameterSpec) {
    if crate::synthesis::parameters::VOICE_CONTROLS.iter().any(|s|s.key==spec.key) {
        spec.limit*=app.synthesis.library.settings.experiments.expression_factor();
    }
    let additional=VOICE_CONTROLS.iter().chain(crate::synthesis::parameters::TUNING_CONTROLS.iter()).any(|s|s.key==spec.key);
    let supported = app.voice_inventory().and_then(|v| v.configuration.as_ref())
        .is_some_and(|v| v.family == VoiceFamily::Sv2 || (additional && app.synthesis.library.settings.experiments.active(app.synthesis.library.settings.experiments.dds_controls)));
    match app.editor.voice_parameter(spec) {
        Ok((reference, value)) if supported => {
            let id = ui.make_persistent_id((spec.key, reference, app.editor.revision));
            let mut current = ui.ctx().data_mut(|d| d.get_temp::<f32>(id)).unwrap_or(value.clamp(-spec.limit,spec.limit));
            let response = slider(ui, &mut current,spec);
            if response.dragged() { ui.ctx().data_mut(|d| d.insert_temp(id, current)); }
            if response.drag_stopped() || response.clicked() || response.double_clicked() {
                ui.ctx().data_mut(|d| d.remove::<f32>(id));
                if let Err(error) = app.editor.set_voice_parameter(reference,spec,current) { app.error = Some(error); }
            }
        }
        _ => widgets::slider(ui,crate::i18n::t(spec.label)),
    }
}

pub(super) fn slider(ui: &mut Ui, value: &mut f32,spec:ParameterSpec) -> Response {
    let height=if spec.key.starts_with("consonant"){48.0}else{51.0};
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let y = rect.top() + 32.0;
    let rail = Rect::from_min_max(pos2(rect.left() + 9.5, y - 10.0), pos2(rect.right() - 9.5, y + 10.0));
    let response = ui.interact(rail.expand2(vec2(9.5, 0.0)), ui.id().with(spec.key), Sense::click_and_drag());
    if response.double_clicked() { *value = 0.0; }
    else if (response.dragged() || response.clicked()) && let Some(pointer) = response.interact_pointer_pos() {
        let raw=((pointer.x-rail.center().x)/(rail.width()/2.0)*spec.limit).clamp(-spec.limit,spec.limit);
        *value=if spec.step>0.0 {(raw/spec.step).round()*spec.step}else{raw};
    }
    let painter = ui.painter();
    painter.text(rect.min + vec2(0.0, 4.0), Align2::LEFT_TOP,crate::i18n::t(spec.label), FontId::proportional(12.0), MUTED);
    painter.hline(rail.x_range(), y, Stroke::new(2.0, EDGE));
    let center = pos2(rail.center().x + (*value/spec.limit).clamp(-1.0,1.0) * rail.width() / 2.0, y);
    painter.line_segment([pos2(rail.center().x, y), center], Stroke::new(2.0, TEXT));
    painter.circle(center, 9.5, EDGE, Stroke::NONE);
    painter.circle(center, 8.5, Color32::from_rgb(65,63,68), Stroke::new(0.7, BUTTON_ACTIVE));
    painter.circle_filled(center, 3.0, TEXT);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true,crate::i18n::t(spec.label)));
    response.on_hover_text(format!("{}: {:.3} {}",crate::i18n::t(spec.label),*value,crate::i18n::t(spec.unit)))
}
