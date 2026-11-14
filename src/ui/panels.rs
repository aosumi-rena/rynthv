mod controls;
mod dictionary;
mod note;
mod render;
pub(crate) use note::end_performance_frame;

use super::{
    child,
    state::{Column, DragPayload},
    theme::*,
};
use crate::app::App;
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui, column: Column) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 4, PANEL);
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), 42.0));
    let mut header_ui = child(
        ui,
        header.shrink2(vec2(14.0, 2.0)),
        ("panel-header", column),
    );
    header_ui.spacing_mut().interact_size.y = header.height() - 4.0;
    header_ui.horizontal(|ui| {
        let (icon, _) = ui.allocate_exact_size(vec2(20.0, 24.0), Sense::hover());
        column.icon().paint(
            ui,
            Rect::from_center_size(icon.center(), Vec2::splat(20.0)),
            MUTED,
        );
        super::section_title(ui, column.label(), DragPayload::Column(column));
    });
    ui.interact(header,Id::new(("panel-header-drag",column)),Sense::drag())
        .dnd_set_drag_payload(DragPayload::Column(column));
    ui.painter()
        .hline(header.x_range(), header.bottom(), Stroke::new(1.0, EDGE));
    let mut content = child(
        ui,
        Rect::from_min_max(pos2(rect.left(), header.bottom()), rect.max),
        ("panel-content", column),
    );
    ScrollArea::vertical()
        .id_salt(("panel-scroll", column))
        .show(&mut content, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().interact_size.y = 28.0;
            ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(4);
            if app.editor.current_track().is_instrumental()
                && matches!(column, Column::Note | Column::Voice | Column::Dictionary)
            {
                controls::padded(ui, |ui| {
                    ui.weak(crate::i18n::t("当前选择的是伴奏轨，请选择歌声音轨以编辑此项。"));
                });
                return;
            }
            match column {
                Column::Note => note::show(app, ui),
                Column::Voice => super::voice_library::show(app, ui),
                Column::Render => render::show(app, ui),
                Column::Dictionary => dictionary::show(app, ui, rect.height() - header.height()),
                Column::Script => controls::padded(ui, |ui| {
                    ui.weak(crate::i18n::t("脚本运行环境尚未接入"));
                }),
                Column::Editor => {}
            }
        });
    ui.painter()
        .rect_stroke(rect, 4, Stroke::new(1.0, BORDER), StrokeKind::Inside);
}
