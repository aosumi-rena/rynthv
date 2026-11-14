use super::{
    child,
    state::{Column, DragPayload},
    theme::*,
};
use crate::app::App;
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui, seeked: bool) {
    let rect = ui.max_rect();
    let rail = Rect::from_min_max(
        pos2(rect.right() - super::tool_rail::WIDTH, rect.top()),
        rect.max,
    );
    super::tool_rail::show(app, &mut child(ui, rail, "tool-rail"));
    let main = Rect::from_min_max(rect.min, pos2(rail.left() - 5.0, rect.bottom()));
    let columns = app.view.columns.clone();
    let panels = columns.len() - 1;
    let panel_width = app
        .view
        .panel_width
        .min(((main.width() - 330.0) / panels.max(1) as f32 - 5.0).max(72.0));
    let editor_width = (main.width() - panels as f32 * (panel_width + 5.0)).max(200.0);
    let mut x = main.left();
    let mut drop = None;
    for (index, column) in columns.iter().enumerate() {
        let width = if *column == Column::Editor {
            editor_width
        } else {
            panel_width
        };
        let r = Rect::from_min_size(pos2(x, main.top()), vec2(width, main.height()));
        let mut column_ui = child(ui, r, ("column", column));
        if *column == Column::Editor {
            editor(app, &mut column_ui,seeked);
        } else {
            super::panels::show(app, &mut column_ui, *column);
        }
        if let Some(payload) = DragAndDrop::payload::<DragPayload>(ui.ctx())
            && let DragPayload::Column(source) = *payload
        {
            if ui.input(|i|i.pointer.hover_pos().is_some_and(|p|r.contains(p))) && source != *column {
                let before =
                    ui.input(|i| i.pointer.hover_pos().is_some_and(|p| p.x < r.center().x));
                let target = if before { r.left() } else { r.right() };
                ui.painter()
                    .vline(target, r.y_range(), Stroke::new(3.0, GREEN));
                if ui.input(|i|i.pointer.any_released()) {
                    DragAndDrop::take_payload::<DragPayload>(ui.ctx());
                    drop = Some((source, index + usize::from(!before)));
                }
            }
        }
        x += width + 5.0;
        if index + 1 < columns.len() {
            let divider = Rect::from_min_size(pos2(x - 5.0, r.top()), vec2(5.0, r.height()));
            let response = ui
                .interact(divider, Id::new(("column-divider", column)), Sense::drag())
                .on_hover_cursor(CursorIcon::ResizeHorizontal);
            if response.dragged() {
                let direction = if *column == Column::Editor { -1.0 } else { 1.0 };
                app.view.panel_width = (app.view.panel_width + direction * response.drag_delta().x)
                    .clamp(140.0, 420.0);
            }
        }
    }
    if let Some((source, mut index)) = drop
        && let Some(old) = app.view.columns.iter().position(|c| *c == source)
    {
        app.view.columns.remove(old);
        if old < index {
            index -= 1;
        }
        app.view.columns.insert(index, source);

        app.view.panel_order.retain(|c|*c!=source);
        let next=app.view.columns.get(index+1);
        let at=next.and_then(|next|app.view.panel_order.iter().position(|c|c==next))
            .unwrap_or(app.view.panel_order.len());
        app.view.panel_order.insert(at,source);
    }
}
fn editor(app: &mut App, ui: &mut Ui,seeked:bool) {
    let rect = ui.max_rect();
    let available = (rect.height() - 5.0).max(0.0);
    let arrangement_height = if app.view.arrangement_collapsed {
        42.0
    } else if app.piano_collapsed {
        available - super::piano::TOOLBAR_HEIGHT
    } else {
        app.view
            .arrangement_height
            .min((available - 170.0).max(80.0))
    }
    .clamp(0.0, available);
    let piano_height = if app.piano_collapsed {
        super::piano::TOOLBAR_HEIGHT.min(available - arrangement_height)
    } else {
        available - arrangement_height
    };
    let (arrangement, piano, divider) = if app.view.arrangement_first {
        let y = rect.top() + arrangement_height;
        (
            Rect::from_min_max(rect.min, pos2(rect.right(), y)),
            Rect::from_min_size(pos2(rect.left(), y + 5.0), vec2(rect.width(), piano_height)),
            Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), 5.0)),
        )
    } else {
        let y = rect.top() + piano_height;
        (
            Rect::from_min_size(
                pos2(rect.left(), y + 5.0),
                vec2(rect.width(), arrangement_height),
            ),
            Rect::from_min_max(rect.min, pos2(rect.right(), y)),
            Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), 5.0)),
        )
    };
    if seeked {
        let gutter=250.0_f32.min(arrangement.width()*0.36);
        super::timeline::reveal(&mut app.view.arrangement,arrangement.width()-gutter,app.editor.transport.position);
        super::timeline::reveal(&mut app.view.piano,piano.width()-super::timeline::GUTTER,app.editor.transport.position);
    }
    let position=app.editor.transport.position;
    super::arrangement::show(app, &mut child(ui, arrangement, "arrangement"));
    if position!=app.editor.transport.position {
        super::timeline::reveal(&mut app.view.piano,piano.width()-super::timeline::GUTTER,app.editor.transport.position);
    }
    let position=app.editor.transport.position;
    piano_stack(app, &mut child(ui, piano, "piano-stack"));
    if position!=app.editor.transport.position {
        let gutter=250.0_f32.min(arrangement.width()*0.36);
        super::timeline::reveal(&mut app.view.arrangement,arrangement.width()-gutter,app.editor.transport.position);
        ui.ctx().request_repaint();
    }
    if !app.view.arrangement_collapsed && !app.piano_collapsed {
        let response = ui
            .interact(divider, Id::new("main-divider"), Sense::drag())
            .on_hover_cursor(CursorIcon::ResizeVertical);
        if response.dragged() {
            app.view.arrangement_height = (app.view.arrangement_height
                + response.drag_delta().y
                    * if app.view.arrangement_first {
                        1.0
                    } else {
                        -1.0
                    })
            .clamp(80.0, (available - 170.0).max(80.0));
        }
    }
    if let Some(payload) = DragAndDrop::payload::<DragPayload>(ui.ctx())
        && matches!(*payload, DragPayload::Arrangement | DragPayload::Piano)
    {
        let response = ui.interact(rect, Id::new("arrangement-drop"), Sense::hover());
        if response.contains_pointer() {
            let first = ui.input(|i| i.pointer.hover_pos().is_some_and(|p| p.y < rect.center().y));
            ui.painter().hline(
                rect.x_range(),
                if first { rect.top() } else { rect.bottom() },
                Stroke::new(3.0, GREEN),
            );
            if response.dnd_release_payload::<DragPayload>().is_some() {
                app.view.arrangement_first = if matches!(*payload, DragPayload::Arrangement) {
                    first
                } else {
                    !first
                };
            }
        }
    }
}
fn piano_stack(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();
    let toolbar = Rect::from_min_size(rect.min, vec2(rect.width(), super::piano::TOOLBAR_HEIGHT));
    let tabs = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 28.0), rect.max);
    ui.painter().rect_filled(rect, 4, PANEL);
    ui.painter()
        .rect_stroke(rect, 4, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    super::piano::toolbar(app, &mut child(ui, toolbar, "piano-toolbar"));
    if app.piano_collapsed || rect.height() <= super::piano::TOOLBAR_HEIGHT {
        return;
    }
    let lanes: Vec<_> = app
        .view
        .lanes
        .iter()
        .filter(|l| l.visible)
        .cloned()
        .collect();
    let desired: f32 = lanes.iter().map(|l| l.height + 5.0).sum();
    let phonemes = if app.view.phonemes { 100.0 } else { 0.0 };
    let available = (tabs.top() - toolbar.bottom() - 125.0 - phonemes).max(0.0);
    let scale = if desired > available {
        available / desired.max(1.0)
    } else {
        1.0
    };
    let mut y = tabs.top() - desired * scale - phonemes;
    super::piano::show(
        app,
        &mut child(
            ui,
            Rect::from_min_max(pos2(rect.left(), toolbar.bottom()), pos2(rect.right(), y)),
            "piano",
        ),
    );
    if app.view.phonemes {
        let p = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), phonemes));
        super::phonemes::show(app,&mut child(ui,p,"phonemes"));
        y += phonemes;
    }
    let mut reorder = None;
    for lane in lanes {
        let r = Rect::from_min_size(
            pos2(rect.left(), y + 5.0),
            vec2(rect.width(), lane.height * scale),
        );
        let divider = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), 5.0));
        let response = ui
            .interact(divider, Id::new(("lane-divider", lane.id)), Sense::drag())
            .on_hover_cursor(CursorIcon::ResizeVertical);
        if response.dragged()
            && let Some(l) = app.view.lanes.iter_mut().find(|l| l.id == lane.id)
        {
            l.height = (l.height - response.drag_delta().y).clamp(64.0, 500.0);
        }
        super::lanes::show(app, &mut child(ui, r, ("lane", lane.id)), lane.id);
        if let Some(payload) = DragAndDrop::payload::<DragPayload>(ui.ctx())
            && let DragPayload::Lane(source) = *payload
            && source != lane.id
        {
            let response = ui.interact(r, Id::new(("lane-drop", lane.id)), Sense::hover());
            if response.contains_pointer() {
                ui.painter()
                    .hline(r.x_range(), r.top(), Stroke::new(3.0, GREEN));
                if response.dnd_release_payload::<DragPayload>().is_some() {
                    reorder = Some((source, lane.id));
                }
            }
        }
        y += (lane.height + 5.0) * scale;
    }
    if let Some((source, target)) = reorder {
        let old = app.view.lanes.iter().position(|l| l.id == source).unwrap();
        let lane = app.view.lanes.remove(old);
        let at = app.view.lanes.iter().position(|l| l.id == target).unwrap();
        app.view.lanes.insert(at, lane);
    }
    let mut tab_ui = child(ui, tabs, "lane-tabs");
    super::lane_tabs::show(app, &mut tab_ui);
}
