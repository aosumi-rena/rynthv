use super::Range;
use crate::ui::theme::*;
use eframe::egui::{
    text::{CCursor, CCursorRange},
    *,
};

#[derive(Clone)]
struct Entry {
    text: String,
    source: u64,
    focus: bool,
    frame: u64,
}

pub(super) fn show(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    source: f64,
    value: f64,
    range: Range,
    format: &dyn Fn(f64) -> String,
    suffix: &str,
    visible: bool,
) -> Option<f64> {
    let response = ui.interact(rect, id.with("value"), Sense::click());
    let input = id.with("input");
    let frame = ui.ctx().cumulative_frame_nr();
    let mut entry = ui
        .ctx()
        .data(|d| d.get_temp::<Entry>(input))
        .filter(|e| ui.is_enabled() && e.source == source.to_bits() && e.frame + 1 >= frame);
    if ui.is_enabled() && response.double_clicked() {
        entry = Some(Entry {
            text: value.to_string(),
            source: source.to_bits(),
            focus: true,
            frame,
        });
    }
    let mut result = None;
    if let Some(mut entry) = entry {
        let mut child = crate::ui::child(ui, rect, id.with("entry-area"));
        let edit = child.add_sized(
            rect.size(),
            TextEdit::singleline(&mut entry.text)
                .id(input)
                .font(FontId::proportional(12.))
                .horizontal_align(Align::RIGHT)
                .vertical_align(Align::Center)
                .margin(vec2(2., 0.))
                .char_limit(24),
        );
        if entry.focus {
            edit.request_focus();
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), input) {
                state.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::new(0),
                    CCursor::new(entry.text.chars().count()),
                )));
                state.store(ui.ctx(), input);
            }
            entry.focus = false;
            ui.ctx().request_repaint();
        }
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        if escape || ui.input(|i| i.key_pressed(Key::Enter)) || edit.lost_focus() {
            if !escape
                && let Ok(number) = entry.text.trim().parse::<f64>()
                && number.is_finite()
            {
                result = Some(range.quantize(number));
            }
            edit.surrender_focus();
            ui.ctx().data_mut(|d| d.remove::<Entry>(input));
        } else {
            entry.frame = frame;
            ui.ctx().data_mut(|d| d.insert_temp(input, entry));
        }
    } else {
        ui.ctx().data_mut(|d| d.remove::<Entry>(input));
        if visible || response.hovered() {
            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                format!("{}{suffix}", format(value)),
                FontId::proportional(12.),
                MUTED,
            );
        }
    }
    result
}
