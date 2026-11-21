use crate::ui::{icons::Icon, theme::*};
use eframe::egui::*;
pub fn height(ctx: &Context) -> f32 {
    (ctx.content_rect().height() - 28.0).clamp(48.0, 1000.0)
}
pub fn frame(ctx: &Context) -> Frame {
    Frame::popup(&ctx.style_of(Theme::Dark))
        .fill(Color32::from_rgb(61, 59, 64))
        .stroke(Stroke::new(0.8, BUTTON_ACTIVE))
        .corner_radius(5)
        .inner_margin(4)
}
pub(crate) fn selection_card(
    ui: &mut Ui,
    name: &str,
    hint: Option<&str>,
    tooltip: Option<String>,
    label: &str,
    artwork: impl FnOnce(&Ui, Rect),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::click());
    ui.painter().rect(
        rect,
        4,
        if response.hovered() { BUTTON } else { BORDER },
        Stroke::new(0.7, BUTTON),
        StrokeKind::Inside,
    );
    let art = Rect::from_min_size(rect.min, Vec2::splat(60.0));
    artwork(ui, art);
    let label_rect = Rect::from_min_max(
        pos2(art.right() + 8.0, rect.top()),
        rect.max - vec2(26.0, 0.0),
    );
    let painter = ui
        .painter()
        .with_clip_rect(label_rect.intersect(ui.clip_rect()));
    painter.text(
        label_rect.left_center() - vec2(0.0, if hint.is_some() { 9.0 } else { 0.0 }),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(13.0),
        TEXT,
    );
    if let Some(hint) = hint {
        painter.text(
            label_rect.left_center() + vec2(0.0, 12.0),
            Align2::LEFT_CENTER,
            hint,
            FontId::proportional(10.5),
            MUTED,
        );
    }
    Icon::ChevronDown.paint(
        ui,
        Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), Vec2::splat(12.0)),
        MUTED,
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let response = if let Some(tooltip) = tooltip {
        response.on_hover_text(tooltip)
    } else {
        response
    };
    response.on_hover_cursor(CursorIcon::Default)
}

pub fn card(ui: &mut Ui, selected: bool, enabled: bool) -> (Response, Rect, Rect) {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), 76.),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if selected || (enabled && response.hovered()) {
        ui.painter()
            .rect_filled(rect, 5, if selected { BUTTON_HOVER } else { BUTTON });
    }
    let art = Rect::from_min_size(rect.min + vec2(8., 6.), Vec2::splat(64.));
    let content = Rect::from_min_max(
        pos2(art.right() + 12., rect.top() + 7.),
        rect.max - vec2(6., 4.),
    );
    (response, art, content)
}
pub fn header(ui: &mut Ui, title: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        section_title_font(),
        TEXT,
    );
    let origin = rect.right_center() - vec2(24.0, 7.0);
    for row in 0..3 {
        let p = origin + vec2(0.0, row as f32 * 5.0);
        ui.painter()
            .rect_filled(Rect::from_min_size(p, vec2(5.0, 4.0)), 0, ICON_TOOL);
        ui.painter().rect_filled(
            Rect::from_min_size(p + vec2(7.0, 0.0), vec2(9.0, 2.0)),
            0,
            ICON_TOOL,
        );
    }
}

pub fn menu_row(ui: &mut Ui, label: &str, icon: Option<Icon>, enabled: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), 32.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 3, BUTTON_HOVER);
    }
    let mut x = rect.left() + 8.0;
    if let Some(icon) = icon {
        icon.paint(
            ui,
            Rect::from_center_size(pos2(x + 10.0, rect.center().y), Vec2::splat(20.0)),
            TEXT,
        );
        x += 28.0;
    }
    ui.painter().text(
        pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        if enabled { TEXT } else { MUTED },
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    response.on_hover_cursor(CursorIcon::Default)
}

pub fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, BUTTON_ACTIVE),
    );
}
