use super::{Gesture, NoteDrag, canvas::note_rect};
use crate::{app::App, model::*, ui::timeline::*};
use eframe::egui::*;
use std::collections::BTreeSet;
pub(super) fn pitch_at(app: &App, rect: Rect, y: f32) -> i32 {
    (app.view.top_pitch - (y - rect.top()) / app.view.row_height).ceil() as i32
}
pub(super) fn start_gesture(
    app: &mut App,
    ui: &Ui,
    rect: Rect,
    groups: &[super::scene::SceneGroup],
) {
    let Some(origin) = ui.input(|i| i.pointer.press_origin()) else {
        return;
    };
    let modifiers = ui.input(|i| i.modifiers);
    app.commit_lyric();
    app.editor.track_focus=false;
    if let Some((reference, note)) = super::scene::hit_note(app, groups, rect, origin) {
        app.editor.select_reference(app.editor.track, reference.id);
        let r = note_rect(
            &note,
            reference.blick_offset,
            reference.pitch_offset,
            rect,
            app,
        );
        if !app.editor.selected.contains(&note.id) {
            app.editor.selected = BTreeSet::from([note.id]);
        }
        let kind = if origin.x > r.right() - 6.0 {
            NoteDrag::Right
        } else if origin.x < r.left() + 6.0 {
            NoteDrag::Left
        } else {
            NoteDrag::Move
        };
        app.editor.begin(crate::i18n::t("拖动音符"));
        let mut selected: Vec<_> = app
            .editor
            .notes()
            .iter()
            .filter(|n| app.editor.selected.contains(&n.id))
            .cloned()
            .collect();
        let movement=if app.preferences.sync_note_parameters && matches!(kind,NoteDrag::Move) {
            app.editor.group().map(|group|Box::new(crate::editor::note_movement::NoteMovement::capture(group,&selected,modifiers.alt&&!modifiers.ctrl)))
        }else{None};
        if modifiers.alt && !modifiers.ctrl && matches!(kind, NoteDrag::Move) {
            for n in &mut selected {
                n.renew_identity();
            }
            app.editor.selected = selected.iter().map(|n| n.id).collect();
            app.editor
                .group_mut()
                .unwrap()
                .notes
                .extend(selected.clone());
        }
        app.gesture = Some(Gesture::Notes {
            origin,
            notes: selected,
            kind,
            anchor: match kind {
                NoteDrag::Right => note.onset + note.duration,
                _ => note.onset,
            },
            movement,
        });
    } else if app.piano_tool == super::Tool::NoteDraw || modifiers.ctrl {
        app.editor.begin(crate::i18n::t("创建音符"));
        let beat = beat_at(rect, &app.view.piano, origin.x);
        let at = ticks(if app.view.snap && !(modifiers.ctrl && modifiers.alt) {
            app.view.snap_beat(beat)
        } else {
            beat
        })
        .max(0);
        if let Some(id) =
            app.editor
                .add_note(at, ticks(app.view.step()), pitch_at(app, rect, origin.y))
        {
            let onset = at - app.editor.active_ref().unwrap().blick_offset;
            app.gesture = Some(Gesture::Create { origin, id, onset });
        } else {
            app.editor.finish();
        }
    } else {
        let at = ticks(beat_at(rect, &app.view.piano, origin.x));
        if let Some(id) = app.editor.group_at(at) {
            app.editor.select_reference(app.editor.track, id);
        }
        let initial = if modifiers.shift {
            app.editor.selected.clone()
        } else {
            BTreeSet::new()
        };
        app.gesture = Some(Gesture::Select { origin, initial });
    }
}
pub(super) fn update_gesture(app: &mut App, ui: &Ui, rect: Rect) {
    let Some(p) = ui.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let Some(gesture) = app.gesture.take() else {
        return;
    };
    let modifiers = ui.input(|i| i.modifiers);
    let snapping = app.view.snap && !(modifiers.ctrl && modifiers.alt);
    let minimum = ticks(if snapping { app.view.step() } else { 0.01 });
    let Some(reference) = app.editor.active_ref().cloned() else {
        return;
    };
    match &gesture {
        Gesture::Notes {
            origin,
            notes,
            kind,
            anchor,
            movement,
        } => {
            let raw = ticks((p.x - origin.x) as f64 / app.view.piano.beat_width as f64);
            let mut delta = if snapping {
                ticks(
                    app.view
                        .snap_beat(beats(anchor + reference.blick_offset + raw)),
                ) - reference.blick_offset
                    - anchor
            } else {
                raw
            };
            let mut pitch = ((origin.y - p.y) / app.view.row_height).round() as i32;
            if modifiers.ctrl && !modifiers.alt {
                if modifiers.shift {
                    delta = 0;
                } else {
                    pitch = 0;
                }
            }
            let min_pitch = notes.iter().map(|n| n.pitch).min().unwrap_or(0);
            let max_pitch = notes.iter().map(|n| n.pitch).max().unwrap_or(127);
            pitch = pitch.clamp(-min_pitch, 127 - max_pitch);
            let min_onset = notes.iter().map(|n| n.onset).min().unwrap_or(0);

            let shortest = notes.iter().map(|n| n.duration).min().unwrap_or(minimum);
            let max_shrink = (shortest - minimum).max(0);
            delta = match kind {
                NoteDrag::Move => delta.max(-min_onset - reference.blick_offset),
                NoteDrag::Left => {
                    delta.clamp((-min_onset - reference.blick_offset).min(0), max_shrink)
                }
                NoteDrag::Right => delta.max(-max_shrink),
            };
            for n in &mut app.editor.group_mut().unwrap().notes {
                if let Some(old) = notes.iter().find(|old| old.id == n.id) {
                    match kind {
                        NoteDrag::Move => {
                            n.onset = old.onset + delta;
                            n.pitch = old.pitch + pitch;
                        }
                        NoteDrag::Left => {
                            n.onset = old.onset + delta;
                            n.duration = old.duration - delta;
                        }
                        NoteDrag::Right => n.duration = old.duration + delta,
                    }
                }
            }
            if let Some(movement)=movement {movement.apply(app.editor.group_mut().unwrap(),delta,pitch);}
        }
        Gesture::Create { origin, id, onset } => {
            let beat = beat_at(rect, &app.view.piano, p.x);
            let end = ticks(if snapping {
                app.view.snap_beat(beat)
            } else {
                beat
            })
            .clamp(reference.blick_absolute_begin, reference.visible_end())
                - reference.blick_offset;
            if let Some(n) = app
                .editor
                .group_mut()
                .unwrap()
                .notes
                .iter_mut()
                .find(|n| n.id == *id)
            {
                n.onset = (*onset).min(end);
                n.duration = (end - onset).abs().max(minimum);
                if (p.x - origin.x).abs() < 2.0 {
                    n.onset = *onset;
                }
            }
        }
        Gesture::Select { origin, initial } => {
            let r = Rect::from_two_pos(*origin, p);
            let mut selected = initial.clone();
            for n in app.editor.notes() {
                if crate::ui::group_geometry::visible_note_rect(
                    n,
                    &reference,
                    rect,
                    &app.view.piano,
                    app.view.top_pitch,
                    app.view.row_height,
                )
                .is_some_and(|note| note.intersects(r))
                {
                    selected.insert(n.id);
                }
            }
            app.editor.selected = selected;
        }
        Gesture::Parameter { .. } | Gesture::Phoneme(_) | Gesture::Curve(_) | Gesture::Pitch(_) | Gesture::Anchor(_) | Gesture::Rap(_) => {}
    }
    app.gesture = Some(gesture);
}
