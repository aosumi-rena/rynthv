use super::{
    theme::*,
    time_edit::{self, Target},
    timeline::*,
};
use crate::{app::App, model::*};
use eframe::egui::*;

pub const ROW_HEIGHT: f32 = 28.0;

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    for row_index in 0..2 {
        let row = Rect::from_min_size(
            rect.min + vec2(0.0, row_index as f32 * ROW_HEIGHT),
            vec2(rect.width(), ROW_HEIGHT),
        )
        .intersect(rect)
        .intersect(ui.clip_rect());
        if !row.is_positive() {
            continue;
        }
        let response = ui.interact(row, Id::new(("time-track", row_index)), Sense::click());
        let painter = ui.painter().with_clip_rect(row);
        let time = &app.editor.project.time;
        let mut markers = Vec::new();
        if row_index == 0 {
            for meter in &time.meter {
                let x = x_at(
                    rect,
                    &app.view.arrangement,
                    beats(time.bar_position(meter.index)),
                );
                if let Some(hit) = marker(
                    &painter,
                    pos2(x + 5.0, rect.top() + ROW_HEIGHT / 2.0),
                    &format!("{}/{}", meter.numerator, meter.denominator),
                    true,
                ) {
                    markers.push((hit, Target::Meter(meter.index)));
                }
            }
        } else {
            for tempo in &time.tempo {
                let x = x_at(rect, &app.view.arrangement, beats(tempo.position));
                if let Some(hit) = marker(
                    &painter,
                    pos2(x + 5.0, rect.top() + ROW_HEIGHT * 1.5),
                    &format!("{:.2}", tempo.bpm),
                    false,
                ) {
                    markers.push((hit, Target::Tempo(tempo.position)));
                }
            }
        }
        if response.clicked()
            && !app.view_pan.is_active()
            && let Some(pointer) = response.interact_pointer_pos()
        {

            let target = markers
                .iter()
                .rev()
                .find(|(hit, _)| hit.contains(pointer))
                .map(|(_, target)| *target)
                .unwrap_or_else(|| {
                    let beat = beat_at(rect, &app.view.arrangement, pointer.x).max(0.0);
                    if row_index == 0 {
                        Target::Meter(time.bar_at(ticks(beat)).0)
                    } else {
                        Target::Tempo(snap_arrangement_position(&app.view.arrangement, time, beat))
                    }
                });
            time_edit::open(app, target);
        }
        for (hit, target) in markers {
            let response = ui.interact(hit, Id::new(("time-event", target)), Sense::click());
            if response.clicked() && !app.view_pan.is_active() {
                time_edit::open(app, target);
            }
            response.context_menu(|ui| event_menu(app, ui, target));
        }
    }
}

fn marker(painter: &Painter, pos: Pos2, text: &str, flag: bool) -> Option<Rect> {
    let text = painter.layout_no_wrap(text.into(), FontId::proportional(12.0), TEXT);
    let rect = Rect::from_min_size(pos - vec2(5.0, 9.0), vec2(text.size().x + 9.0, 19.0));
    let bounds = Rect::from_min_max(rect.min, rect.max + vec2(if flag { 7.0 } else { 0.0 }, 0.0));
    let hit = bounds.intersect(painter.clip_rect());
    if !hit.is_positive() {
        return None;
    }
    if flag {
        painter.add(Shape::convex_polygon(
            vec![
                rect.left_top(),
                rect.right_top(),
                rect.right_center() + vec2(7.0, 0.0),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            BUTTON,
            Stroke::NONE,
        ));
    } else {
        painter.rect_filled(rect, 1, BUTTON);
    }
    painter.galley(pos2(pos.x, pos.y - text.size().y / 2.0), text, TEXT);
    Some(hit)
}

fn event_menu(app: &mut App, ui: &mut Ui, target: Target) {
    let (edit_label, delete_label, can_delete) = match target {
        Target::Meter(index) => (crate::i18n::t("编辑拍号…"), crate::i18n::t("删除拍号标记"), index > 0),
        Target::Tempo(position) => (crate::i18n::t("编辑曲速…"), crate::i18n::t("删除曲速标记"), position > 0),
    };
    if ui.button(edit_label).clicked() {
        time_edit::open(app, target);
        ui.close();
    }
    ui.separator();
    if ui
        .add_enabled(can_delete, Button::new(delete_label))
        .on_disabled_hover_text(crate::i18n::t("初始拍号和曲速必须保留，可通过编辑修改其数值"))
        .clicked()
    {
        super::tempo_input::finish(app);
        app.commit_lyric();
        match target {
            Target::Meter(index) => app.editor.delete_meter(index),
            Target::Tempo(position) => app.editor.delete_tempo(position),
        }
        ui.close();
    }
}
