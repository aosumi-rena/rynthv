use crate::{app::App, editor::Editor, ui::theme::*};
use eframe::egui::{
    text::{CCursor, CCursorRange},
    *,
};

pub const WIDTH: f32 = 72.0;

pub struct BpmEdit {
    position: i64,
    text: String,
    initial_text: String,
    focus: bool,
    error: Option<String>,
}
impl BpmEdit {
    fn commit(&self, editor: &mut Editor) -> Result<(), String> {

        if self.text == self.initial_text {
            return Ok(());
        }
        if !editor
            .project
            .time
            .tempo
            .iter()
            .any(|t| t.position == self.position)
        {
            return Err(crate::i18n::t("所编辑的曲速事件已不存在").into());
        }
        let bpm = self
            .text
            .trim()
            .parse::<f64>()
            .map_err(|_| crate::i18n::t("请输入有效的 BPM 数值"))?;
        editor.set_tempo(self.position, bpm)
    }
}

pub fn open(app: &mut App, position: i64, ctx: &Context) {
    app.commit_lyric();
    let tempo = app
        .editor
        .project
        .time
        .tempo
        .iter()
        .rev()
        .find(|t| t.position <= position)
        .expect("time axis begins at zero");
    let text = format!("{:.2}", tempo.bpm);
    app.bpm_edit = Some(BpmEdit {
        position: tempo.position,
        initial_text: text.clone(),
        text,
        focus: true,
        error: None,
    });
    ctx.request_repaint();
}

pub fn finish(app: &mut App) {
    if let Some(edit) = app.bpm_edit.take()
        && let Err(error) = edit.commit(&mut app.editor)
    {
        app.status = error;
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let Some(mut edit) = app.bpm_edit.take() else {
        return;
    };
    let input = ui.add_sized(
        vec2(WIDTH, 28.0),
        TextEdit::singleline(&mut edit.text)
            .id(Id::new("transport-bpm-input"))
            .font(FontId::proportional(18.0))
            .text_color(TEXT)
            .background_color(PANEL)
            .margin(vec2(4.0, 0.0))
            .horizontal_align(Align::Center)
            .vertical_align(Align::Center),
    );
    if edit.focus {
        input.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), input.id) {
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(0),
                CCursor::new(edit.text.chars().count()),
            )));
            state.store(ui.ctx(), input.id);
        }
        edit.focus = false;
    }
    if input.changed() {
        edit.error = None;
    }
    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        input.surrender_focus();
        ui.ctx().request_repaint();
        return;
    }
    let enter = ui.input(|i| i.key_pressed(Key::Enter));
    if enter || input.lost_focus() {
        match edit.commit(&mut app.editor) {
            Ok(()) => {
                input.surrender_focus();
                ui.ctx().request_repaint();
                return;
            }
            Err(error) if !enter => {
                app.status = error;
                ui.ctx().request_repaint();
                return;
            }
            Err(error) => {
                edit.error = Some(error);
                input.request_focus();
            }
        }
    }
    if let Some(error) = &edit.error {
        ui.painter().rect_stroke(
            input.rect,
            3,
            Stroke::new(1.0, Color32::from_rgb(230, 139, 122)),
            StrokeKind::Inside,
        );
        input.on_hover_text(crate::i18n::message(error));
    }
    app.bpm_edit = Some(edit);
}
