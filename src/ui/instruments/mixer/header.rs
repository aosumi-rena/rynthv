use crate::{
    app::App,
    ui::icons::{Icon, IconButton},
};
use eframe::egui::*;

pub(super) struct Actions {
    pub close: bool,
    pub applied: bool,
    pub navigate: Option<u64>,
}

pub(super) fn show(app: &mut App, ui: &mut Ui, rect: Rect, color: Color32, fit: f32) -> Actions {
    let mut actions = Actions {
        close: false,
        applied: false,
        navigate: None,
    };
    let mut header = crate::ui::child(ui, rect.shrink2(vec2(4., 4.) * fit), "microphone-header");
    header.with_layout(Layout::left_to_right(Align::Center), |ui| {
        let tracks: Vec<_> = app
            .editor
            .project
            .tracks
            .iter()
            .filter(|t| t.is_instrument())
            .map(|t| t.id)
            .collect();
        let index = tracks
            .iter()
            .position(|id| *id == app.editor.track)
            .unwrap_or(0);
        ui.allocate_ui_with_layout(
            vec2(rect.width() * 0.38, 32. * fit),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(rect.width() * 0.38);
                for (enabled, direction, icon, label) in [
                    (index > 0, -1, Icon::ChevronLeft, "Previous Track"),
                    (
                        index + 1 < tracks.len(),
                        1,
                        Icon::ChevronRight,
                        "Next Track",
                    ),
                ] {
                    if ui
                        .add_enabled(
                            enabled,
                            IconButton::new(icon, crate::i18n::t(label)).size(vec2(24., 28.) * fit),
                        )
                        .clicked()
                    {
                        actions.navigate =
                            tracks.get((index as isize + direction) as usize).copied();
                    }
                }
                ui.add(
                    Label::new(RichText::new(&app.editor.current_track().name).color(color))
                        .truncate(),
                );
            },
        );
        for (direction, icon, label) in [
            (-1, Icon::ChevronLeft, "Previous Preset"),
            (1, Icon::ChevronRight, "Next Preset"),
        ] {
            if ui
                .add(
                    IconButton::new(icon, crate::i18n::t(label))
                        .framed()
                        .size(vec2(32., 32.) * fit),
                )
                .clicked()
            {
                actions.applied |= crate::ui::presets::cycle(
                    app,
                    crate::ui::presets::Kind::Microphones,
                    direction,
                );
            }
        }
        ui.allocate_ui_with_layout(vec2(172.5, 32.) * fit, Layout::top_down(Align::Min), |ui| {
            ui.set_width(172.5 * fit);
            actions.applied |=
                crate::ui::presets::show(app, ui, crate::ui::presets::Kind::Microphones);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            actions.close = ui
                .add(
                    IconButton::new(Icon::Close, crate::i18n::t("Close"))
                        .size(vec2(24., 24.) * fit),
                )
                .clicked();
        });
    });
    actions
}
