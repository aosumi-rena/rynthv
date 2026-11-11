mod interaction;
mod rap;
mod widgets;
pub(crate) mod spread;

pub(crate) use interaction::end_frame;

use crate::{
    app::App,
    editor::{PerformanceParameter, PerformanceSelection},
    model::{NoteMode, NotePerformance},
    synthesis::voice::VoiceFamily,
    ui::theme::*,
};
use eframe::egui::*;

pub(super) fn mode(app: &mut App, ui: &mut Ui) {
    let selection = app.editor.performance_selection();
    let selected = selection.as_ref().ok().and_then(Option::as_ref);
    let enabled = editable(app, ui) && selected.is_some();
    let native_voice = uses_native_voice(app);
    let mode = selected.map_or(Some(NoteMode::Singing), |selection| selection.mode);
    let clicked = ui
        .add_enabled_ui(enabled, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
            ui.painter().rect_filled(rect, 3, canvas_color());
            let mut clicked = None;
            for (index, value, label) in
                [(0, NoteMode::Singing, crate::i18n::t("歌唱")), (1, NoteMode::Rap, crate::i18n::t("说唱"))]
            {
                let half = Rect::from_min_max(
                    pos2(rect.left() + rect.width() * index as f32 * 0.5, rect.top()),
                    pos2(
                        rect.left() + rect.width() * (index + 1) as f32 * 0.5,
                        rect.bottom(),
                    ),
                )
                .shrink(3.0);
                let supported = value != NoteMode::Rap || !native_voice;
                let mut button_ui = ui.new_child(UiBuilder::new().max_rect(half));
                if !supported {
                    button_ui.disable();
                }
                let response =
                    button_ui.interact(half, ui.id().with(("mode", index)), Sense::click());
                if mode == Some(value) || response.hovered() {
                    ui.painter().rect_filled(
                        half,
                        3,
                        if response.hovered() {
                            button_hover_color()
                        } else {
                            button_color()
                        },
                    );
                }
                ui.painter().text(
                    half.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(12.0),
                    if !supported {
                        muted_color().gamma_multiply(0.5)
                    } else if mode == Some(value) {
                        text_color()
                    } else {
                        muted_color()
                    },
                );
                if response.clicked() {
                    clicked = Some(value);
                }
                if !supported {
                    response.on_disabled_hover_text(crate::i18n::t("此声库暂不支持说唱模式"));
                }
            }
            clicked
        })
        .inner;
    if let Some(mode) = clicked {
        app.commit_lyric();
        app.editor.set_note_mode(mode);
    }
    if selected.is_some_and(|selection| selection.mode.is_none()) {
        ui.weak(crate::i18n::t("多个模式"));
    }
}

pub(super) fn expression(app: &mut App, ui: &mut Ui) {
    let selection = app.editor.performance_selection();
    let selected = selection.as_ref().ok().and_then(Option::as_ref);
    let enabled = editable(app, ui) && selected.is_some();
    let defaults = NotePerformance::default();
    let mut value = [
        normalize_expression(
            selected
                .and_then(|s| s.expression_x)
                .unwrap_or(defaults.expression_x),
        ),
        normalize_expression(
            selected
                .and_then(|s| s.expression_y)
                .unwrap_or(defaults.expression_y),
        ),
    ];
    let mixed = selected.is_some_and(|s| s.expression_x.is_none() || s.expression_y.is_none());
    let native_voice = uses_native_voice(app);
    let response = ui
        .add_enabled_ui(enabled && !native_voice, |ui| {
            widgets::expression(ui, &mut value, mixed)
        })
        .inner;
    interaction::apply(
        app,
        ui.ctx(),
        &response,
        PerformanceParameter::Expression,
        |editor| {
            editor.update_performance_expression(
                denormalize_expression(value[0]),
                denormalize_expression(value[1]),
            );
        },
    );
    if native_voice {
        ui.weak(crate::i18n::t("此声库暂不支持演绎坐标"));
    }
    ui.add_space(8.0);
    if selected.is_none_or(|selection| selection.mode != Some(NoteMode::Rap)) {
        vibrato(app, ui, selected, enabled && !crate::ui::phoneme_candidates::standard_voice(app));
    }
    if let Some(selection) = selected.filter(|selection| selection.mode != Some(NoteMode::Singing))
    {
        rap::show(app, ui, selection, enabled && !native_voice);
        if native_voice {
            ui.weak(crate::i18n::t("此声库暂不支持说唱模式"));
        }
    }
    if let Err(error) = &selection {
        ui.weak(crate::i18n::message(&error));
    } else if selected.is_none() {
        ui.weak(crate::i18n::t("在钢琴卷帘中选择音符"));
    }
}

fn vibrato(app: &mut App, ui: &mut Ui, selection: Option<&PerformanceSelection>, enabled: bool) {
    let defaults = NotePerformance::default();
    let current = selection.and_then(|selection| selection.vibrato);
    let mixed = selection.is_some() && current.is_none();
    let minimum = NotePerformance::VIBRATO_MIN;
    let span = NotePerformance::VIBRATO_MAX - minimum;
    let mut value = (current.unwrap_or(defaults.vibrato) - minimum) / span;
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            widgets::labeled_slider(
                ui,
                crate::i18n::t("颤音调制"),
                if mixed {
                    crate::i18n::t("多个值").into()
                } else {
                    format!("{:.0}%", current.unwrap_or(defaults.vibrato) * 100.0)
                },
                &mut value,
                mixed,
                (defaults.vibrato - minimum) / span,
            )
        })
        .inner;
    interaction::apply(
        app,
        ui.ctx(),
        &response,
        PerformanceParameter::Vibrato,
        |editor| {
            editor.update_performance_vibrato(minimum + value * span);
        },
    );
}

fn editable(app: &App, ui: &Ui) -> bool {
    !interaction::blocked(app)
        && app.gesture.is_none()
        && (!app.editor.is_editing() || app.editor.has_performance_edit())
        && ui.input(|input| input.focused)
}

fn uses_native_voice(app: &App) -> bool {
    app.voice_selection()
        .and_then(|voice| app.synthesis.library.entry(&voice))
        .is_some_and(|entry| matches!(entry.family,Some(VoiceFamily::Sv1|VoiceFamily::R1Std|VoiceFamily::R2Std)))
}

fn normalize_expression(value: f64) -> f64 {
    (value - NotePerformance::EXPRESSION_MIN)
        / (NotePerformance::EXPRESSION_MAX - NotePerformance::EXPRESSION_MIN)
}

fn denormalize_expression(value: f64) -> f64 {
    NotePerformance::EXPRESSION_MIN
        + value * (NotePerformance::EXPRESSION_MAX - NotePerformance::EXPRESSION_MIN)
}
