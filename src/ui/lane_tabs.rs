use super::{
    icons::{Icon, IconButton},
    piano::Gesture,
    state::Lane,
    theme::*,
};
use crate::app::App;
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui) {
    let experiments_available=app.voice_inventory().and_then(|v|v.configuration.as_ref())
        .is_some();
    for lane in &mut app.view.lanes {
        if !lane.foreground.is_visible() || (!experiments_available && matches!(lane.foreground,crate::model::Parameter::Experimental{..})) {lane.foreground=crate::model::Parameter::Pitch;}
        if !lane.background.is_visible() || (!experiments_available && matches!(lane.background,crate::model::Parameter::Experimental{..})) {lane.background=crate::model::Parameter::Loudness;}
    }
    let mut close = None;
    ScrollArea::horizontal()
        .id_salt("lane-tabs-scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if tab(ui, Icon::Phonemes, crate::i18n::t("音素时值"), app.view.phonemes, None)
                    .0
                    .clicked()
                {
                    app.view.phonemes = !app.view.phonemes;
                }
                for lane in &mut app.view.lanes {
                    let (response, closed) = tab(
                        ui,
                        Icon::Curve,
                        &format!("{}/{}", crate::ui::parameter_label(&lane.foreground), crate::ui::parameter_label(&lane.background)),
                        lane.visible,
                        Some(lane.id),
                    );
                    if closed {
                        close = Some(lane.id);
                    } else if response.clicked() || response.secondary_clicked() {
                        lane.visible = !lane.visible;
                    }
                }
                if ui
                    .add_enabled(
                        app.view.lanes.len() < 16,
                        IconButton::new(Icon::Plus, crate::i18n::t("新增参数区域"))
                            .size(vec2(30.0, 26.0))
                            .icon_size(14.0),
                    )
                    .clicked()
                {
                    app.view.lanes.push(Lane::new());
                }
            });
        });
    if let Some(id) = close {

        app.view.lanes.retain(|lane| lane.id != id);
        if matches!(app.gesture, Some(Gesture::Parameter { lane, .. }) if lane == id)
            || matches!(&app.gesture,Some(Gesture::Curve(g)) if g.lane==id) {
            app.editor.finish();
            app.gesture = None;
        }
        ui.ctx().request_repaint();
    }
}

fn tab(ui: &mut Ui, icon: Icon, text: &str, active: bool, lane: Option<u64>) -> (Response, bool) {
    let color = if active { TEXT } else { MUTED };
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), FontId::proportional(12.0), color);
    let close_width = if lane.is_some() { 24.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(
        vec2(galley.size().x + 32.0 + close_width, 27.0),
        Sense::hover(),
    );
    let label_rect = Rect::from_min_max(rect.min, rect.max - vec2(close_width, 0.0));
    let id = ui.id().with(("lane-tab", lane));
    let response = ui.interact(label_rect, id, Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, active, text));
    if response.hovered() {
        ui.painter().rect_filled(label_rect, 0, BUTTON);
    }
    icon.paint(
        ui,
        Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), Vec2::splat(16.0)),
        color,
    );
    ui.painter().galley(
        pos2(rect.left() + 25.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    let mut closed = false;
    if lane.is_some() {
        let close_rect = Rect::from_min_max(pos2(label_rect.right(), rect.top()), rect.max)
            .shrink2(vec2(2.0, 3.0));

        let close = ui.interact(close_rect, id.with("close"), Sense::click());
        close.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, crate::lformat!("关闭{text}")));
        if close.hovered() {
            ui.painter().rect_filled(close_rect, 3, BUTTON_HOVER);
        }
        Icon::Close.paint(
            ui,
            Rect::from_center_size(close_rect.center(), Vec2::splat(12.0)),
            if close.hovered() { TEXT } else { MUTED },
        );
        closed = close.clicked();
        close
            .on_hover_cursor(CursorIcon::Default)
            .on_hover_text(crate::i18n::t("关闭参数区域"));
    }
    ui.painter().vline(
        rect.right(),
        rect.top() + 5.0..=rect.bottom() - 5.0,
        Stroke::new(1.0, BORDER),
    );
    (response.on_hover_cursor(CursorIcon::Default), closed)
}
