use eframe::egui::*;

pub(super) struct Row<'a> {
    pub symbol: String,
    pub category: &'a str,
    pub example: &'a str,
}

pub(super) fn geometry(root: Rect, anchor: Rect, count: usize) -> Rect {
    let size = vec2(396.0_f32.min(root.width().max(0.0)),
        (count as f32 * 32.0).min(160.0).min((root.height() - 8.0).max(0.0)) + 8.0_f32.min(root.height().max(0.0)));
    let x = (anchor.center().x - 21.0).clamp(root.left(), (root.right() - size.x).max(root.left()));
    let above = anchor.top() - 2.0 - size.y;
    let y = if above >= root.top() { above } else { anchor.bottom() + 2.0 };
    Rect::from_min_size(pos2(x, y.clamp(root.top(), (root.bottom() - size.y).max(root.top()))), size)
}

pub(super) fn show(ctx: &Context, id: Id, anchor: Rect, rows: &[Row<'_>], selected: usize, moved: bool) -> Option<usize> {
    let bounds = geometry(ctx.content_rect(), anchor, rows.len());
    let mut chosen = None;
    Area::new(id).order(Order::Foreground).constrain(false).fixed_pos(bounds.min).show(ctx, |ui| {
        crate::ui::theme::menu::popup_style(ui.style_mut());
        crate::ui::theme::menu::frame(ui.style()).inner_margin(Margin::symmetric(0, 4))
            .stroke(Stroke::NONE).show(ui, |ui| {
                ui.set_width(bounds.width());
                ui.spacing_mut().item_spacing.y = 0.0;
                ScrollArea::vertical().id_salt(id.with("rows")).max_height((bounds.height() - 8.0).max(0.0))
                    .auto_shrink([false, true]).show(ui, |ui| {
                        for (index, row) in rows.iter().enumerate() {
                            let response = draw_row(ui, row, index == selected);
                            if index == selected && moved { response.scroll_to_me(Some(Align::Center)); }
                            if response.clicked() { chosen = Some(index); }
                        }
                    });
            });
    });
    chosen
}

fn draw_row(ui: &mut Ui, row: &Row<'_>, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    if !ui.is_rect_visible(rect) { return response; }
    let palette = crate::ui::theme::palette();
    if selected || response.hovered() {
        ui.painter().rect_filled(rect.shrink2(vec2(4.0_f32.min(rect.width() / 2.0), 1.0)), 4, palette.popup_highlight);
    }
    let color = if selected { palette.popup_highlight_text } else { palette.popup_text };
    let content = rect.shrink2(vec2(16.0_f32.min(rect.width() / 2.0), 0.0));
    if content.width() <= 0.0 { return response; }
    let symbol = Rect::from_min_size(content.min, vec2(28.0_f32.min(content.width().max(0.0)), content.height()));
    ui.painter().with_clip_rect(symbol.intersect(ui.clip_rect())).text(symbol.left_center(), Align2::LEFT_CENTER,
        &row.symbol, FontId::proportional(14.0), color);
    let details = Rect::from_min_max(pos2(symbol.right(), content.top()), content.max);
    let painter = ui.painter().with_clip_rect(details.intersect(ui.clip_rect()));
    let category = painter.layout_no_wrap(row.category.to_owned(), FontId::proportional(12.0), color);
    let x = details.left() + category.size().x + 8.0;
    painter.galley(pos2(details.left(), details.center().y - category.size().y / 2.0), category, color);
    if !row.example.is_empty() {
        painter.text(pos2(x, details.center().y), Align2::LEFT_CENTER, row.example,
            FontId::proportional(12.0), if selected { color } else { palette.muted });
    }
    response
}
