mod clip;
use crate::{app::App, model::*};
pub use clip::show;
use eframe::egui::*;

pub struct AudioDrag {
    track: EntityId,
    pointer_x: f32,
    beat: f64,
    beat_width: f32,
}

pub fn create_menu(app: &mut App, ui: &mut Ui) {
    if ui.button(crate::i18n::t("添加歌声音轨…")).clicked() {
        app.commit_lyric();
        app.editor.add_track();
        ui.close();
    }
    if ui.button(crate::i18n::t("Add Instrument Track...")).clicked() {
        app.commit_lyric();app.editor.add_instrument_track();ui.close();
    }
    if ui.button(crate::i18n::t("添加音频音轨…")).clicked() {
        app.commit_lyric();
        let id = app.editor.add_instrumental_track();
        ui.close();
        app.import_audio(Some(id), ui.ctx());
    }
}

pub fn update_drag(app: &mut App, ctx: &Context) {
    let Some(drag) = &app.audio_drag else { return };
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.editor.cancel();
        app.audio_drag = None;
        return;
    }
    if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
        let beat = app
            .view
            .snap_beat(drag.beat + f64::from((pos.x - drag.pointer_x) / drag.beat_width));
        let seconds = app.editor.project.time.seconds_at(ticks(beat));
        app.editor.move_audio_clip(drag.track, seconds);
    }
    if ctx.input(|i| i.pointer.button_released(PointerButton::Primary) || !i.pointer.primary_down())
    {
        app.editor.finish();
        app.audio_drag = None;
    }
}
