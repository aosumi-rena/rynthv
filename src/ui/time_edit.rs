use crate::{app::App, editor::Editor, model::TimeAxis, ui::theme::*};
use eframe::egui::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Meter(i32),
    Tempo(i64),
}

pub struct TimeEdit {
    target: Target,
    value: String,
    denominator: u32,
    focus: bool,
    error: Option<String>,
}
impl TimeEdit {
    pub fn new(target: Target, time: &TimeAxis) -> Self {
        let (value, denominator) = match target {
            Target::Meter(index) => {
                let meter = time.meter_at(index);
                (meter.numerator.to_string(), meter.denominator)
            }
            Target::Tempo(position) => (time.bpm_at(position).to_string(), 4),
        };
        Self {
            target,
            value,
            denominator,
            focus: true,
            error: None,
        }
    }

    fn commit(&self, editor: &mut Editor) -> Result<(), String> {
        match self.target {
            Target::Tempo(position) => {
                let bpm = self
                    .value
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| crate::i18n::t("请输入有效的 BPM 数值"))?;
                editor.set_tempo(position, bpm)
            }
            Target::Meter(index) => {
                let numerator = self
                    .value
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| crate::i18n::t("拍号分子须为 1–64 的整数"))?;
                editor.set_meter(index, numerator, self.denominator)
            }
        }
    }
}

pub fn open(app: &mut App, target: Target) {
    super::tempo_input::finish(app);
    app.commit_lyric();
    app.time_edit = Some(TimeEdit::new(target, &app.editor.project.time));
}

pub fn show(app: &mut App, ctx: &Context) {
    let Some(mut edit) = app.time_edit.take() else {
        return;
    };
    let mut submit = false;
    let mut cancel = false;
    const WIDTH: f32 = 312.0;
    let modal = Modal::new(Id::new("time-event-editor"))
        .frame(
            Frame::new()
                .fill(Color32::from_rgb(55, 55, 57))
                .stroke(Stroke::new(1.0, Color32::from_rgb(73, 72, 76)))
                .corner_radius(4)
                .inner_margin(24),
        )
        .backdrop_color(Color32::from_black_alpha(30))
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 8.0;
            let (title, label) = match edit.target {
                Target::Meter(_) => (crate::i18n::t("编辑拍号"), crate::i18n::t("分子")),
                Target::Tempo(_) => (crate::i18n::t("编辑曲速标记"), crate::i18n::t("每分钟节拍数 (BPM)")),
            };
            ui.label(
                RichText::new(title)
                    .font(FontId::new(16.0, FontFamily::Name("SectionTitle".into()))),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(label).color(MUTED));
            let input = ui.add_sized(
                vec2(WIDTH, 28.0),
                TextEdit::singleline(&mut edit.value)
                    .id(Id::new("time-event-value"))
                    .margin(vec2(6.0, 0.0))
                    .horizontal_align(Align::Center)
                    .vertical_align(Align::Center),
            );
            if edit.focus {
                input.request_focus();
                edit.focus = false;
            }
            if input.changed() {
                edit.error = None;
            }
            if input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                submit = true;
            }
            if matches!(edit.target, Target::Meter(_)) {
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::t("分母")).color(MUTED));
                ComboBox::from_id_salt("time-event-denominator")
                    .width(WIDTH)
                    .selected_text(edit.denominator.to_string())
                    .show_ui(ui, |ui| {
                        for value in [1, 2, 4, 8, 16, 32, 64] {
                            ui.selectable_value(&mut edit.denominator, value, value.to_string());
                        }
                    });
            }
            if let Some(error) = &edit.error {
                ui.label(RichText::new(crate::i18n::message(error)).color(Color32::from_rgb(230, 139, 122)));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let size = vec2((WIDTH - 8.0) / 2.0, 32.0);
                cancel = ui
                    .add_sized(size, Button::new(crate::i18n::t("取消")).fill(BUTTON_HOVER))
                    .clicked();
                submit |= ui
                    .add_sized(
                        size,
                        Button::new(RichText::new(crate::i18n::t("确定")).color(CANVAS)).fill(GREEN),
                    )
                    .clicked();
            });
        });
    cancel |= modal.is_top_modal
        && !modal.any_popup_open
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    if cancel {
        return;
    }
    if submit {
        match edit.commit(&mut app.editor) {
            Ok(()) => return,
            Err(error) => {
                edit.error = Some(error);
                edit.focus = true;
            }
        }
    }
    app.time_edit = Some(edit);
}
