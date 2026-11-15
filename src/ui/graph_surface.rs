use super::{icons::Icon, theme::*, timeline::GUTTER};
use eframe::egui::*;
pub fn canvas(
    ui: &mut Ui,
    rect: Rect,
    icon: Option<Icon>,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> (Rect, Response) {
    let canvas = Rect::from_min_max(rect.min + vec2(GUTTER, 0.0), rect.max);
    ui.painter().rect_filled(rect, 0, PANEL);
    if let Some(icon) = icon {
        icon.paint(
            ui,
            Rect::from_center_size(
                pos2(rect.left() + 22.0, rect.top() + 16.0),
                vec2(18.0, 18.0),
            ),
            TEXT,
        );
    }
    let response = ui.interact(canvas, ui.id().with(id), Sense::click_and_drag());
    (canvas, response)
}
