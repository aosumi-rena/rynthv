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
    let parameter=match spec.key {
        "paramTension"=>Some(crate::model::Parameter::Tension),
        "paramBreathiness"=>Some(crate::model::Parameter::Breathiness),
        "paramGender"=>Some(crate::model::Parameter::Gender),
        _=>None,
    };
    let bounded=parameter.as_ref().is_some_and(|p|crate::ui::lanes::standard_expression_limit(app,p).is_some());
    if bounded {
        let (_,high)=crate::ui::lanes::accepted_parameter_range(app,parameter.as_ref().unwrap());
        spec.limit=spec.limit.min(high as f32);
    }
    let additional=VOICE_CONTROLS.iter().chain(crate::synthesis::parameters::TUNING_CONTROLS.iter()).any(|s|s.key==spec.key);
    let supported = app.voice_inventory().and_then(|v| v.configuration.as_ref())
        .is_some_and(|v| v.family == VoiceFamily::Sv2 || (v.family.is_standard() && additional &&
            spec.key!="paramToneShift") ||
            (!v.family.is_standard() && additional && app.synthesis.library.settings.experiments.active(app.synthesis.library.settings.experiments.dds_controls)));
    match app.editor.voice_parameter(spec) {
        Ok((reference, value)) if supported => {
            let id = ui.make_persistent_id((spec.key, reference, app.editor.revision));
            let mut current = ui.ctx().data_mut(|d| d.get_temp::<f32>(id)).unwrap_or(if bounded{value}else{value.clamp(-spec.limit,spec.limit)});
            let response = slider(ui, &mut current,spec);
            if bounded && (response.dragged() || response.drag_stopped() || response.clicked() || response.double_clicked()) {
                current=current.clamp(-spec.limit,spec.limit);
            }
            if bounded && !{
                let (low,high)=crate::ui::lanes::accepted_parameter_range(app,parameter.as_ref().unwrap());
                (low..=high).contains(&f64::from(value))
            } {
                ui.colored_label(Color32::LIGHT_RED,crate::lformat!("Current value {value:.3} is outside this voice's supported range"));
            }
            if response.dragged() { ui.ctx().data_mut(|d| d.insert_temp(id, current)); }
            if response.drag_stopped() || response.clicked() || response.double_clicked() || response.changed() && !response.dragged() {
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

    let value_rect=Rect::from_min_size(pos2((rect.right()-75.0).max(rect.left()),rect.top()),vec2(75.0_f32.min(rect.width()),16.0));
    let value_response=ui.scope(|ui| {
        ui.spacing_mut().interact_size.y=16.0;
        ui.style_mut().text_styles.insert(TextStyle::Button,FontId::proportional(12.0));
        let classic=current() == InterfaceTheme::Classic;
        let fill=if classic {Color32::from_rgb(60,60,60)} else {Color32::from_white_alpha(18)};
        let ink=if classic {Color32::from_rgb(216,216,216)} else {Color32::from_rgb(239,232,242)};
        let outline=if classic {Stroke::NONE} else {Stroke::new(0.5,Color32::from_white_alpha(18))};
        let widgets=&mut ui.visuals_mut().widgets;
        for widget in [&mut widgets.inactive,&mut widgets.hovered,&mut widgets.active] {
            widget.bg_fill=fill;widget.weak_bg_fill=fill;widget.fg_stroke.color=ink;widget.bg_stroke=outline;
        }
        ui.put(value_rect,DragValue::new(value).range(-spec.limit..=spec.limit).clamp_existing_to_range(false)
            .speed(if spec.step>0.0 {f64::from(spec.step)} else {0.01})
            .suffix(crate::i18n::t(spec.unit)))
    }).inner;
    if value_response.changed() {
        if spec.step>0.0 {*value=(*value/spec.step).round()*spec.step;}
        *value=(*value).clamp(-spec.limit,spec.limit);
    }
    let painter = ui.painter();
    painter.text(rect.min + vec2(0.0, 4.0), Align2::LEFT_TOP,crate::i18n::t(spec.label), FontId::proportional(12.0), muted_color());
    painter.hline(rail.x_range(), y, Stroke::new(2.0, edge_color()));
    let center = pos2(rail.center().x + (*value/spec.limit).clamp(-1.0,1.0) * rail.width() / 2.0, y);
    painter.line_segment([pos2(rail.center().x, y), center], Stroke::new(2.0, text_color()));
    painter.circle(center, 9.5, edge_color(), Stroke::NONE);
    painter.circle(center, 8.5, Color32::from_rgb(65,63,68), Stroke::new(0.7, button_active_color()));
    painter.circle_filled(center, 3.0, text_color());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true,crate::i18n::t(spec.label)));
    response.union(value_response).on_hover_text(format!("{}: {:.3} {}",crate::i18n::t(spec.label),*value,crate::i18n::t(spec.unit)))
}
