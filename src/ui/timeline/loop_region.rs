use super::{Viewport, beat_at, x_at};
use crate::{
    editor::Transport,
    model::{beats, ticks},
};
use eframe::egui::*;

pub struct RulerOptions {
    pub marker_step: Option<f64>,
    pub seek_step: Option<f64>,
    pub enabled: bool,
}

#[derive(Clone, Copy)]
enum Part {
    Start,
    End,
    Body,
}

#[derive(Clone, Copy)]
struct Drag {
    part: Part,
    pointer_beat: f64,
    start: i64,
    end: i64,
}

fn position(beat: f64, step: Option<f64>) -> i64 {
    ticks(step.map_or(beat, |step| (beat / step).round() * step)).max(0)
}

fn marker_rect(ruler: Rect, view: &Viewport, transport: &Transport) -> Rect {
    Rect::from_min_max(
        pos2(
            x_at(ruler, view, beats(transport.loop_region.start())),
            ruler.bottom() - 32.0,
        ),
        pos2(
            x_at(ruler, view, beats(transport.loop_region.end())),
            ruler.bottom() - 16.0,
        ),
    )
}

fn hit(marker: Rect, ruler: Rect, pointer: Pos2) -> Option<Part> {
    if !ruler.contains(pointer) || !marker.contains(pointer) {
        return None;
    }
    let edge = 4.0_f32.min(marker.width() / 3.0);
    if pointer.x - marker.left() <= edge {
        Some(Part::Start)
    } else if marker.right() - pointer.x <= edge {
        Some(Part::End)
    } else {
        Some(Part::Body)
    }
}

pub fn show(
    ui: &Ui,
    id: Id,
    ruler: Rect,
    view: &Viewport,
    transport: &mut Transport,
    options: RulerOptions,
) -> Option<i64> {
    let response = ui.interact(ruler, id, Sense::click_and_drag());
    let drag_id = id.with("loop-drag");
    let menu_id = id.with("loop-menu-position");
    let (primary_pressed, primary_down, focused, pointer, alt) = ui.input(|input| {
        (
            input.pointer.button_pressed(PointerButton::Primary),
            input.pointer.button_down(PointerButton::Primary),
            input.focused,
            input.pointer.interact_pos(),
            input.modifiers.alt,
        )
    });
    let mut drag = ui.ctx().data(|data| data.get_temp::<Drag>(drag_id));
    let enabled = options.enabled && focused && ui.is_enabled();
    if !enabled || !transport.looping {
        drag = None;
    } else if primary_pressed {

        drag = pointer
            .filter(|_| response.contains_pointer())
            .and_then(|pointer| {
                hit(marker_rect(ruler, view, transport), ruler, pointer).map(|part| Drag {
                    part,
                    pointer_beat: beat_at(ruler, view, pointer.x),
                    start: transport.loop_region.start(),
                    end: transport.loop_region.end(),
                })
            });
    }

    if enabled {
        if let Some(drag) = drag {
            if (response.dragged_by(PointerButton::Primary)
                || response.drag_stopped_by(PointerButton::Primary))
                && let Some(pointer) = pointer
            {
                let delta = beat_at(ruler, view, pointer.x) - drag.pointer_beat;
                let step = if alt { None } else { options.marker_step };
                match drag.part {
                    Part::Start => transport
                        .loop_region
                        .set_start(position(beats(drag.start) + delta, step)),
                    Part::End => transport
                        .loop_region
                        .set_end(position(beats(drag.end) + delta, step)),
                    Part::Body => transport
                        .loop_region
                        .move_to(position(beats(drag.start) + delta, step)),
                }
            }
        } else if (response.clicked() || response.dragged_by(PointerButton::Primary))
            && let Some(pointer) = response.interact_pointer_pos()
        {
            transport.position = position(beat_at(ruler, view, pointer.x), options.seek_step);
        }

        if response.secondary_clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let at = position(
                beat_at(ruler, view, pointer.x),
                if alt { None } else { options.marker_step },
            );
            ui.ctx().data_mut(|data| data.insert_temp(menu_id, at));
        }
        response.context_menu(|ui| {
            let Some(at) = ui.ctx().data(|data| data.get_temp::<i64>(menu_id)) else {
                return;
            };
            if ui.button(crate::i18n::t("将循环开始标记移至此处")).clicked() {
                transport.loop_region.place_start(at);
                transport.looping = true;
                ui.close();
            }
            if ui.button(crate::i18n::t("将循环结束标记移至此处")).clicked() {
                transport.loop_region.place_end(at);
                transport.looping = true;
                ui.close();
            }
        });
    }

    if transport.looping {
        let marker = marker_rect(ruler, view, transport);
        let part = enabled
            .then(|| {
                drag.map(|drag| drag.part).or_else(|| {
                    ui.input(|input| input.pointer.hover_pos())
                        .filter(|_| response.hovered())
                        .and_then(|pointer| hit(marker, ruler, pointer))
                })
            })
            .flatten();
        if let Some(part) = part {
            ui.ctx().set_cursor_icon(match part {
                Part::Start | Part::End => CursorIcon::ResizeHorizontal,
                Part::Body if primary_down => CursorIcon::Grabbing,
                Part::Body => CursorIcon::Grab,
            });
        }
        let painter = ui.painter().with_clip_rect(ruler.intersect(ui.clip_rect()));
        painter.rect_filled(marker, 0, Color32::from_rgb(67, 65, 70));
        painter.rect_stroke(
            marker,
            0,
            Stroke::new(1.0, Color32::from_rgb(85, 82, 89)),
            StrokeKind::Inside,
        );
    }
    ui.ctx().data_mut(|data| {
        if let Some(drag) = drag.filter(|_| primary_down && enabled) {
            data.insert_temp(drag_id, drag);
        } else {
            data.remove::<Drag>(drag_id);
        }
    });

    if enabled && !ui.input(|i| i.pointer.any_down()) && !Popup::is_any_open(ui.ctx()) && response.contains_pointer() {
        ui.input(|i|i.pointer.hover_pos()).filter(|p| {
            !transport.looping || hit(marker_rect(ruler,view,transport),ruler,*p).is_none()
        }).map(|p|position(beat_at(ruler,view,p.x),options.seek_step))
    } else {None}
}
