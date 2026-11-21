pub mod dynamics;
pub mod lanes;
mod effects;
mod ensemble;
pub(crate) use effects::room::{apply_link as link_room_settings,linked as room_settings_linked};
mod controls;
mod smart;
pub mod articulation;
pub use articulation::toolbar as articulation_toolbar;
mod manager;
mod manager_card;
mod mixer;
mod note;
pub mod rewrite;
mod panel;
mod selector;
mod settings;
pub mod visuals;
use crate::{
    app::App,
    ui::{icons::Icon, state::Column},
};
pub use effects::show as effects_panel;
pub use manager::dialog;
pub use mixer::dialog as mixer_dialog;
pub use mixer::show as mixer_panel;
pub use note::show as note_panel;
pub use panel::show as track_panel;
pub use selector::{popup, waveform_picker};
pub use settings::show as settings;

pub fn label(column: Column) -> &'static str {
    crate::i18n::t(match column {
        Column::Voice => "Track",
        Column::Note => "Note",
        Column::Dictionary => "Mic Mixer",
        Column::Effects => "Effects",
        Column::Render => "Render",
        _ => return column.label(),
    })
}
pub fn icon(column: Column) -> Icon {
    match column {
        Column::Voice => Icon::Instrument,
        Column::Dictionary => Icon::MicMixer,
        Column::Effects => Icon::Effects,
        _ => column.icon(),
    }
}
fn current(app: &App) -> Option<&crate::synthesis::instruments::Entry> {
    app.synthesis
        .instruments
        .for_track(app.editor.current_track())
}
fn section(ui: &mut eframe::egui::Ui, title: &str, content: impl FnOnce(&mut eframe::egui::Ui)) {
    use eframe::egui::*;
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.next_widget_position().y,
        Stroke::new(1.0, crate::ui::theme::EDGE),
    );
    Frame::new().inner_margin(16).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 10.0;
        CollapsingHeader::new(RichText::new(title).font(crate::ui::theme::section_title_font()))
            .default_open(true)
            .show_unindented(ui, content);
    });
}
fn native_label(key: &str) -> &str {
    let value = crate::i18n::t(key);
    for prefix in ["Articulation_", "Mic_", "Reverb_", "Effects_"] {
        if let Some(value) = value.strip_prefix(prefix) {
            return value;
        }
    }
    value
}
pub fn toolbar(app: &mut App, ui: &mut eframe::egui::Ui) {
    use crate::ui::{
        icons::{Icon, IconButton},
        piano::Tool,
    };
    for (tool, icon, label) in [
        (Tool::NoteSelect, Icon::NoteSelect, "Select Notes"),
        (Tool::NoteDraw, Icon::NoteDraw, "Draw Notes"),
    ] {
        if ui
            .add(
                IconButton::new(icon, crate::i18n::t(label))
                    .framed()
                    .selected(app.piano_tool == tool),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_tool = tool;
        }
    }
    ui.add_enabled(
        false,
        IconButton::new(Icon::PitchDraw, crate::i18n::t("Pitch")),
    );
}
