pub mod arrangement;
pub mod arrangement_tracks;
pub mod chrome;
pub mod fill_lyrics;
pub mod group_edit;
pub mod group_geometry;
pub mod icons;
pub mod instrumental;
mod lane_tabs;
pub mod lanes;
pub mod midi_import;
pub mod panels;
pub mod piano;
pub mod phonemes;
pub mod prompt;
pub mod rename;
mod rendering_hint;
pub mod retakes;
pub mod scale;
pub mod scrollbars;
pub mod settings;
pub mod state;
pub mod synthesis;
pub mod tempo_input;
pub mod theme;
pub mod time_edit;
pub mod time_tracks;
pub mod timeline;
pub mod tool_rail;
mod track_color;
pub mod track_header;
pub mod transport;
pub mod view_pan;
pub mod voice_library;
pub mod workspace;

use eframe::egui::*;
pub fn parameter_label(parameter: &crate::model::Parameter) -> std::borrow::Cow<'_,str> {
    if let crate::model::Parameter::Experimental{key,label}=parameter {
        if key.starts_with("ddsPhoneMix_") {return format!("{} {label}",crate::i18n::t("Mix phoneme")).into();}
        if let Some((component,name))=key.strip_prefix("vocalModeComponent_").and_then(|v|v.split_once(':')) {
            let lane=match component {"pitch"=>"音高","timbre"=>"音色",_=>"发音"};
            return format!("{name} · {}",crate::i18n::t(lane)).into();
        }
        if key.starts_with("voiceBlend_") {
            return format!("{} · {}",crate::i18n::t("歌声混合"),label.split_once(" · ").map_or(label.as_str(),|(_,tail)|tail)).into();
        }
    }
    if matches!(parameter,crate::model::Parameter::VocalMode(_)) {parameter.label().into()}
    else {crate::i18n::t(parameter.label()).into()}
}
pub fn child(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash + std::fmt::Debug) -> Ui {
    let mut child = ui.new_child(UiBuilder::new().id(Id::new(id)).max_rect(rect));
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child
}
pub fn label(ui: &Ui, pos: Pos2, text: impl ToString, color: Color32) {
    ui.painter().text(
        pos,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(12.0),
        color,
    );
}

pub fn section_title(ui: &mut Ui, text: &str, payload: state::DragPayload) -> Response {
    let response = ui.add(
        Label::new(
            RichText::new(text)
                .font(theme::section_title_font())
                .color(theme::TEXT),
        )
        .selectable(false)
        .sense(Sense::drag()),
    );
    response.dnd_set_drag_payload(payload);
    response.on_hover_cursor(CursorIcon::Default)
}
