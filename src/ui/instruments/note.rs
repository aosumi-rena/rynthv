use crate::{
    app::App,
    model::RetakeComponent,
    ui::{retakes, theme::*},
};
use eframe::egui::*;

fn cards(ui: &mut Ui, labels: [(&str, &'static [u8]); 3]) -> Option<usize> {
    let w = (ui.available_width() - 8.0) / 3.0;
    let mut click = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, (label, svg)) in labels.into_iter().enumerate() {
            let (r, response) = ui.allocate_exact_size(vec2(w, 80.0), Sense::click());
            ui.painter().rect_filled(
                r,
                3,
                if response.hovered() && ui.is_enabled() {
                    BUTTON_HOVER
                } else {
                    BORDER
                },
            );
            Image::from_bytes(format!("bytes://rinstx-retake-{label}"), svg)
                .tint(crate::ui::theme::accent(ui).gamma_multiply(if ui.is_enabled() { 0.45 } else { 0.16 }))
                .fit_to_exact_size(r.size())
                .paint_at(ui, r);
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                crate::i18n::t(label),
                FontId::proportional(12.0),
                if ui.is_enabled() { TEXT } else { MUTED },
            );
            if response.clicked() {
                click = Some(i);
            }
        }
    });
    click
}
pub fn show(app: &mut App, ui: &mut Ui) {
    super::section(ui, crate::i18n::t("AI Retakes"), |ui| {
        let enabled = !app.editor.selected.is_empty()
            && app.editor.active_ref().is_some()
            && !app.editor.is_editing();
        let choice = ui
            .add_enabled_ui(enabled, |ui| {
                cards(
                    ui,
                    [
                        (
                            "PITCH",
                            include_bytes!("../../../assets/icons/rinstx/pitch.svg"),
                        ),
                        (
                            "TIMBRE",
                            include_bytes!("../../../assets/icons/rinstx/timbre.svg"),
                        ),
                        (
                            "BOTH",
                            include_bytes!("../../../assets/icons/rinstx/both.svg"),
                        ),
                    ],
                )
            })
            .inner;
        if let Some(i) = choice {
            retakes::create(
                app,
                [
                    RetakeComponent::Pitch,
                    RetakeComponent::Timbre,
                    RetakeComponent::PitchAndTimbre,
                ][i],
            );
        }
        if let Ok(Some(selection)) = app.editor.retake_selection() {
            retakes::selector(app, ui, &selection);
        } else {
            ui.add_enabled(
                false,
                Button::new(crate::i18n::t("Default")).min_size(vec2(ui.available_width(), 28.0)),
            );
        }
    });
    super::section(ui, crate::i18n::t("AI Rewrites"), |ui| {
        let choice=ui.add_enabled_ui(super::rewrite::enabled(app), |ui| {
            cards(
                ui,
                [
                    (
                        "ARTIC.",
                        include_bytes!("../../../assets/icons/rinstx/arrange.svg"),
                    ),
                    (
                        "DYNAMICS",
                        include_bytes!("../../../assets/icons/rinstx/dynamics.svg"),
                    ),
                    (
                        "BOTH",
                        include_bytes!("../../../assets/icons/rinstx/both.svg"),
                    ),
                ],
            )
        }).inner;
        if let Some(index)=choice {
            use crate::synthesis::instruments::rewrite::Kind;
            super::rewrite::start(app,ui.ctx(),[Kind::Articulation,Kind::Dynamics,Kind::Both][index]);
        }
    });
    super::section(ui, crate::i18n::t("Ensemble Spread"), |ui| {
        super::ensemble::spread(app,ui);
    });
    super::section(ui, crate::i18n::t("Vibrato"), |ui| {
        super::controls::vibrato(app, ui);
    });
    super::section(ui, crate::i18n::t("Articulation"), |ui| {
        super::articulation::panel(app, ui);
    });
}
