use crate::{app::App, model::Track, ui};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui, track: &Track) {
    if !app.editor.selected_tracks.contains(&track.id) {
        app.editor.select_track_header(track.id, false, false);
    }
    app.editor.track_focus = true;
    for (label, unlinked) in [("Duplicate Track", false), ("Duplicate Track (Unlinked)", true)] {
        if ui.button(crate::i18n::t(label)).clicked() {
            app.commit_lyric();
            app.editor.duplicate_tracks(unlinked);
            ui.close();
        }
    }
    ui.separator();
    if ui.button(crate::i18n::t("重命名音轨…")).clicked() {
        app.rename = Some(ui::rename::Rename::track(track.id, &track.name));
        ui.close();
    }
    ui::instrumental::create_menu(app, ui);
    ui::rocoflex::track_menu(app, ui, track);
    let Some(index) = app.editor.project.tracks.iter().position(|t| t.id == track.id) else {
        return;
    };
    for (enabled, direction, label) in [
        (index > 0, -1, "上移音轨"),
        (index + 1 < app.editor.project.tracks.len(), 1, "下移音轨"),
    ] {
        if ui.add_enabled(enabled, Button::new(crate::i18n::t(label))).clicked() {
            app.editor.reorder_track(track.id, direction);
            ui.close();
        }
    }
    ui.separator();
    if ui.add_enabled(!app.editor.selected_tracks.is_empty(), Button::new(crate::i18n::t("删除音轨"))).clicked() {
        app.commit_lyric();
        app.editor.delete_tracks();
        ui.close();
    }
    ui.separator();
    if let Some(color) = ui::track_color::picker(ui, track.disp_color) {
        app.commit_lyric();
        app.editor.set_track_color(track.id, color);
        ui.close();
    }
}
