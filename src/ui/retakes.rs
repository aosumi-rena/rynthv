use crate::{
    app::App,
    editor::RetakeSelection,
    model::{GroupRef, RetakeComponent},
    synthesis::{runtime::VoiceState, voice::VoiceFamily},
};
use eframe::egui::*;

#[derive(Clone, Copy)]
pub enum Target<'a> {
    Selection,
    Note { reference: &'a GroupRef, id: u64 },
}

pub fn label(id: u32) -> String {
    if id == 0 {
        crate::i18n::t("默认采样").into()
    } else {
        crate::lformat!("重录采样 {id}")
    }
}

pub fn unavailable(app: &App, component: RetakeComponent) -> Option<&'static str> {
    unavailable_at(app, component, Target::Selection)
}

fn unavailable_at(
    app: &App,
    component: RetakeComponent,
    target: Target<'_>,
) -> Option<&'static str> {
    if matches!(target, Target::Selection)
        && (app.editor.selected.is_empty() || app.editor.active_ref().is_none())
    {
        return Some(crate::i18n::t("在钢琴卷帘中选择要重录的音符"));
    }
    if app.editor.is_editing() {
        return Some(crate::i18n::t("请先结束当前编辑"));
    }
    let reason = match voice_family(app, target) {
        Some(VoiceFamily::R1Std | VoiceFamily::R2Std) => Some(crate::i18n::t("当前歌声不支持 AI 重录")),
        Some(VoiceFamily::Sv2) => None,
        Some(VoiceFamily::Sv2Compatible | VoiceFamily::Sv1)
            if component == RetakeComponent::Duration =>
        {
            Some(crate::i18n::t("一代歌声支持音高和音色重录"))
        }
        Some(VoiceFamily::Sv2Compatible | VoiceFamily::Sv1) => None,
        None => Some(crate::i18n::t("请先为当前音轨选择歌声")),
    };
    reason.or_else(|| {
        let key = voice_selection(app, target)?;
        match app.synthesis.voice_state(&key) {
            VoiceState::Loaded(_) => None,
            VoiceState::Loading => Some(crate::i18n::t("歌声正在加载，请稍候")),
            VoiceState::Failed(_) => Some(crate::i18n::t("歌声加载失败，请在歌声面板重试")),
            VoiceState::Unloaded => Some(crate::i18n::t("请先加载当前音轨的歌声")),
        }
    })
}

fn voice_selection(
    app: &App,
    target: Target<'_>,
) -> Option<crate::synthesis::settings::VoiceSelection> {
    match target {
        Target::Selection => app.voice_selection(),
        Target::Note { reference, .. } => app
            .synthesis
            .resolve_voice(app.editor.current_track(), reference)
            .ok()
            .flatten(),
    }
}

fn voice_family(app: &App, target: Target<'_>) -> Option<VoiceFamily> {
    app.synthesis
        .library
        .entry(&voice_selection(app, target)?)?
        .family
}

pub fn create(app: &mut App, component: RetakeComponent) {
    create_at(app, component, Target::Selection);
}

fn create_at(app: &mut App, component: RetakeComponent, target: Target<'_>) {
    if let Some(reason) = unavailable_at(app, component, target) {
        app.status = reason.into();
        return;
    }
    let component = if component == RetakeComponent::All
        && matches!(
            voice_family(app, target),
            Some(VoiceFamily::Sv2Compatible | VoiceFamily::Sv1)
        ) {
        RetakeComponent::PitchAndTimbre
    } else {
        component
    };
    app.commit_lyric();
    let result = match target {
        Target::Selection => app.editor.create_retake(component),
        Target::Note { reference, id } => {
            app.editor
                .create_note_retake(&reference.group_id, id, component)
        }
    };
    report(app, result);
}

pub fn prune(app: &mut App) {
    prune_at(app, Target::Selection);
}

fn prune_at(app: &mut App, target: Target<'_>) {
    app.commit_lyric();
    let result = match target {
        Target::Selection => app.editor.prune_retakes(),
        Target::Note { reference, id } => app.editor.prune_note_retakes(&reference.group_id, id),
    };
    report(app, result);
}

fn select_at(app: &mut App, id: u32, target: Target<'_>) {
    app.commit_lyric();
    let result = match target {
        Target::Selection => app.editor.select_retake(id),
        Target::Note {
            reference,
            id: note,
        } => app.editor.select_note_retake(&reference.group_id, note, id),
    };
    report(app, result);
}

fn report(app: &mut App, result: Result<usize, String>) {
    match result {
        Ok(count) if count > 0 => app.status.clear(),
        Ok(_) => {}
        Err(error) => app.error = Some(error),
    }
}

pub fn commands(app: &mut App, ui: &mut Ui) {
    let reason = unavailable(app, RetakeComponent::All);
    let response = ui.add_enabled(
        reason.is_none(),
        Button::new(crate::i18n::t("重录新的采样")).shortcut_text("Alt+T"),
    );
    if response.clicked() {
        create(app, RetakeComponent::All);
        ui.close();
    } else if let Some(reason) = reason {
        response.on_disabled_hover_text(crate::i18n::message(&reason));
    }
    let can_prune = app
        .editor
        .retake_selection()
        .ok()
        .flatten()
        .is_some_and(|s| s.can_prune);
    if ui
        .add_enabled(
            can_prune,
            Button::new(crate::i18n::t("裁剪保留使用采样")).shortcut_text("Shift+Alt+T"),
        )
        .clicked()
    {
        prune(app);
        ui.close();
    }
}

pub fn choices(app: &mut App, ui: &mut Ui, selection: &RetakeSelection) {
    choices_at(app, ui, selection, Target::Selection);
}

pub fn choices_at(app: &mut App, ui: &mut Ui, selection: &RetakeSelection, target: Target<'_>) {
    if matches!(target, Target::Note { .. }) {
        let reason = unavailable_at(app, RetakeComponent::All, target);
        let response = ui.add_enabled(reason.is_none(), Button::new(crate::i18n::t("重录新的采样")));
        if response.clicked() {
            create_at(app, RetakeComponent::All, target);
            ui.close();
        } else if let Some(reason) = reason {
            response.on_disabled_hover_text(crate::i18n::message(&reason));
        }
    }
    if ui
        .add_enabled(
            selection.can_prune,
            Button::new(crate::i18n::t("裁剪保留使用采样")).shortcut_text("Shift+Alt+T"),
        )
        .clicked()
    {
        prune_at(app, target);
        ui.close();
    }
    ui.separator();
    for take in &selection.takes {
        if ui
            .selectable_label(selection.active_take_id == Some(take.id), label(take.id))
            .clicked()
        {
            select_at(app, take.id, target);
            ui.close();
        }
    }
}

pub fn selector(app: &mut App, ui: &mut Ui, selection: &RetakeSelection) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        crate::ui::theme::native::combo("retake-choice")
            .width((ui.available_width() - 54.0).max(30.0))
            .selected_text(
                selection
                    .active_take_id
                    .map_or_else(|| crate::i18n::t("多个采样").into(), label),
            )
            .show_ui(ui, |ui| choices(app, ui, selection))
            .response
            .on_hover_text(crate::lformat!("切换所选 {} 个音符的采样", selection.note_count));
        for (direction, text, tip) in [(-1, "‹", crate::i18n::t("上一个采样")), (1, "›", crate::i18n::t("下一个采样"))]
        {
            let adjacent = selection.adjacent_id(direction);
            if ui
                .add_enabled(
                    adjacent.is_some(),
                    Button::new(text).min_size(vec2(22.0, 28.0)),
                )
                .on_hover_text(tip)
                .clicked()
            {
                app.commit_lyric();
                let result = app.editor.step_retake(direction);
                report(app, result);
            }
        }
    });
}
