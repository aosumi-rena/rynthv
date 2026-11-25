use crate::{app::App, model::EntityId};
use eframe::egui::*;
mod track;
pub use track::field as track_field;

pub enum Target {
    Track(EntityId),
    Group(EntityId),
}
pub struct Rename {
    target: Target,
    text: String,
    focus: bool,
    shown_frame: Option<u64>,
}
impl Rename {
    pub fn track(id: EntityId, text: &str) -> Self {
        Self {
            target: Target::Track(id),
            text: text.into(),
            focus: true,
            shown_frame: None,
        }
    }
    pub fn group(id: EntityId, text: &str) -> Self {
        Self {
            target: Target::Group(id),
            text: text.into(),
            focus: true,
            shown_frame: None,
        }
    }
}
pub fn show(app: &mut App, ctx: &Context) {
    if app.rename.as_ref().is_some_and(|r| matches!(r.target, Target::Track(_))) {
        track::end_frame(app, ctx);
        return;
    }
    let Some(mut rename) = app.rename.take() else {
        return;
    };
    let mut close = false;
    Modal::new(Id::new("rename")).show(ctx, |ui| {
        ui.heading(crate::i18n::t("重命名音符组"));
        let response = ui.add(TextEdit::singleline(&mut rename.text).desired_width(260.0));
        if rename.focus {
            response.request_focus();
            rename.focus = false;
        }
        ui.horizontal(|ui| {
            let submit = ui
                .add_enabled(!rename.text.trim().is_empty(), Button::new(crate::i18n::t("确定")))
                .clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
            if submit && !rename.text.trim().is_empty() {
                match rename.target {
                    Target::Track(_) => unreachable!("track names are edited in their headers"),
                    Target::Group(id) => app.editor.rename_group(id, rename.text.trim().into()),
                }
                close = true;
            }
            if ui.button(crate::i18n::t("取消")).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                close = true;
            }
        });
    });
    if !close {
        app.rename = Some(rename);
    }
}
