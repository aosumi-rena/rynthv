use crate::{
    app::App,
    model::Track,
    ui::{icons::Icon, theme::*},
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui, rect: Rect, text_rect: Rect, track: &Track) {
    let painter = ui
        .painter()
        .with_clip_rect(ui.clip_rect().intersect(text_rect));
    if track.is_instrumental() {
        Icon::Waveform.paint(
            ui,
            Rect::from_center_size(rect.min + vec2(24.0, 39.0), Vec2::splat(12.0)),
            MUTED,
        );
        let text = track.audio_clip().map_or(crate::i18n::t("导入伴奏…"), |clip| clip.name());
        painter.text(
            rect.min + vec2(35.0, 39.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(12.0),
            MUTED,
        );
        let click = Rect::from_min_max(
            rect.min + vec2(16.0, 29.0),
            pos2(text_rect.right(), rect.bottom()),
        );
        if ui
            .interact(
                click,
                Id::new(("track-audio-file", track.id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::Default)
            .on_hover_text(crate::i18n::t("导入 / 替换伴奏音频"))
            .clicked()
        {
            app.import_audio(Some(track.id), ui.ctx());
        }
        return;
    }
    if track.instrument.is_some() {
        Icon::Instrument.paint(ui,Rect::from_center_size(rect.min+vec2(24.0,39.0),Vec2::splat(14.0)),MUTED);
        let entry=app.synthesis.instruments.for_track(track);
        let name=entry.map(|e|crate::i18n::t(&e.name)).unwrap_or(crate::i18n::t("Select Instrument"));
        painter.text(rect.min+vec2(35.0,39.0),Align2::LEFT_CENTER,name,FontId::proportional(12.0),MUTED);
        let response=ui.interact(Rect::from_min_max(rect.min+vec2(16.0,29.0),pos2(text_rect.right(),rect.bottom())),Id::new(("track-instrument",track.id)),Sense::click());
        if response.clicked(){app.commit_lyric();app.editor.select_track(track.id);}
        crate::ui::instruments::popup(app,&response);return;
    }
    let missing=app.synthesis.voice_not_installed(track,&track.main_ref);
    (if missing {Icon::Warning}else{Icon::Microphone}).paint(
        ui,
        Rect::from_center_size(rect.min + vec2(24.0, 39.0), vec2(12.0, 12.0)),
        MUTED,
    );
    let voice = track
        .main_ref
        .database
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(crate::i18n::t("未指定歌声"));
    painter.text(
        rect.min + vec2(35.0, 39.0),
        Align2::LEFT_CENTER,
        voice,
        FontId::proportional(12.0),
        MUTED,
    );
    let voice_rect = Rect::from_min_max(
        rect.min + vec2(16.0, 29.0),
        pos2(text_rect.right(), rect.bottom()),
    );
    let voice_response = ui
        .interact(
            voice_rect,
            Id::new(("track_voice", track.id)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::Default)
        .on_hover_text(if missing {crate::i18n::t("未安装")}else{crate::i18n::t("选择歌声")});
    if voice_response.clicked() {
        app.commit_lyric();
        app.editor.select_track(track.id);
    }
    crate::ui::voice_library::popup(app, &voice_response);
}
