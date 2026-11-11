use super::{
    child,
    icons::{Icon, IconButton},
    label,
    state::DragPayload,
    theme::*,
    timeline::*,
};
use crate::{app::App, model::*};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect.shrink(metrics().surface_inset), metrics().surface_radius, canvas_color());
    let gutter = metrics().arrangement_gutter(rect.width());
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), metrics().arrangement_header_height));
    ui.painter().rect_filled(
        header,
        if app.view.arrangement_collapsed {
            CornerRadius::same(metrics().surface_radius)
        } else {
            CornerRadius {
                nw: metrics().surface_radius,
                ne: metrics().surface_radius,
                sw: 0,
                se: 0,
            }
        },
        panel_color(),
    );
    ui.painter()
        .rect_stroke(rect.shrink(metrics().surface_inset), metrics().surface_radius, Stroke::new(1.0, border_color()), StrokeKind::Inside);
    let mut bar = child(
        ui,
        Rect::from_min_max(
            header.min + vec2(6.0, metrics().arrangement_header_padding),
            pos2(
                rect.left() + if current() == InterfaceTheme::Classic { 143.0 } else { gutter - 6.0 },
                header.top() + metrics().arrangement_title_height - metrics().arrangement_header_padding,
            ),
        ),
        "arrangement-header",
    );
    bar.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        if ui
            .add(
                IconButton::new(
                    if app.view.arrangement_collapsed {
                        Icon::ChevronRight
                    } else {
                        Icon::ChevronDown
                    },
                    crate::i18n::t("展开 / 折叠编排"),
                )
                .size(vec2(12.0, 24.0))
                .icon_size(12.0),
            )
            .clicked()
        {
            app.view.arrangement_collapsed = !app.view.arrangement_collapsed;
        }
        super::section_title(ui, crate::i18n::t("编排"), DragPayload::Arrangement);
        if current() == InterfaceTheme::Modern { ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let create = ui.add(
                IconButton::new(Icon::Plus, crate::i18n::t("创建音轨"))
                    .native().size(vec2(24.0, 24.0))
                    .icon_size(17.0),
            );
            crate::ui::theme::menu::popup(&create).show(|ui| super::instrumental::create_menu(app, ui));
            if ui
                .add(
                    IconButton::new(Icon::TempoTracks, crate::i18n::t("显示 / 隐藏曲速和拍号轨"))
                        .native().size(vec2(24.0, 24.0))
                        .icon_size(17.0)
                        .selected(app.show_time_tracks)
                        .background(panel_color()),
                )
                .clicked()
            {
                app.show_time_tracks = !app.show_time_tracks;
            }
        }); }
    });
    if current() == InterfaceTheme::Classic {
        let native = Rect::from_min_size(rect.min + vec2(149.0, 0.0), vec2((rect.width() - 149.0).max(0.0), 26.0));
        super::transport::classic(app, &mut child(ui, native, "classic-transport"));
        let menu_rect = Rect::from_min_size(rect.min + vec2((gutter - 29.0).max(0.0), 26.0), vec2(24.0, 24.0));
        let mut menu_ui = child(ui, menu_rect, "classic-track-menu");
        let response = menu_ui.add(IconButton::new(Icon::Menu, crate::i18n::t("创建音轨")).native().size(vec2(24.0, 24.0)));
        crate::ui::theme::menu::popup(&response).show(|ui| {
            super::instrumental::create_menu(app, ui);
            crate::ui::theme::native::checkbox(ui,&mut app.show_time_tracks, crate::i18n::t("显示 / 隐藏曲速和拍号轨"));
        });
    }
    if app.view.arrangement_collapsed {
        return;
    }
    let ruler = Rect::from_min_max(
        pos2(rect.left() + gutter, rect.top() + if current() == InterfaceTheme::Classic { metrics().arrangement_title_height } else { 0.0 }),
        pos2(rect.right(), header.bottom()),
    );
    ui.painter().hline(
        header.left()..=ruler.left(),
        header.bottom(),
        Stroke::new(1.0, edge_color()),
    );
    let time_height = if app.show_time_tracks {
        super::time_tracks::ROW_HEIGHT * 2.0
    } else {
        0.0
    };
    let body = Rect::from_min_max(
        pos2(ruler.left(), header.bottom()),
        rect.max - vec2(0.0, 6.0),
    );
    let pan = app.view_pan.drag(
        ui,
        super::view_pan::Target::Arrangement,
        Rect::from_min_max(ruler.min, body.max),
        app.gesture.is_none() && app.group_gesture.is_none() && app.audio_drag.is_none(),
    );
    app.view.arrangement.start = (app.view.arrangement.start
        - pan.x as f64 / app.view.arrangement.beat_width as f64)
        .max(0.0);
    let response = ui.interact(body, Id::new("arrangement-background"), Sense::click());
    let header_scroll = Rect::from_min_max(
        pos2(rect.left(), header.bottom() + time_height),
        pos2(body.left(), body.bottom()),
    );
    let vertical = if app.view_pan.is_active() {
        pan.y
    } else if ui.rect_contains_pointer(header_scroll) {

        wheel_delta(ui, header_scroll.height()).y
    } else {
        scroll(ui, &response, body, &mut app.view.arrangement,2.0..=100.0)
    };
    let scroll_id = Id::new("track-scroll");
    let max_scroll =
        (app.editor.project.tracks.len() as f32 * metrics().track_height - (body.height() - time_height)).max(0.0);
    let scroll = ui.ctx().data_mut(|d| {
        let value = d.get_temp::<f32>(scroll_id).unwrap_or(0.0);
        let value = (value - vertical).clamp(0.0, max_scroll);
        d.insert_temp(scroll_id, value);
        value
    });
    follow_playhead(
        ui,
        Rect::from_min_max(ruler.min, body.max),
        &mut app.view.arrangement,
        app.editor.transport.position,
        app.editor.transport.playing
            && app.gesture.is_none()
            && app.group_gesture.is_none()
            && app.audio_drag.is_none()
            && !app.view_pan.is_active(),
    );
    let hover_position = loop_region::show(
        ui,
        Id::new("arrangement-ruler"),
        ruler,
        &app.view.arrangement,
        &mut app.editor.transport,
        loop_region::RulerOptions {
            marker_step: app.view.snap.then_some(ARRANGEMENT_GRID),
            seek_step: None,
            enabled: !app.view_pan.is_active()
                && app.gesture.is_none()
                && app.group_gesture.is_none()
                && app.audio_drag.is_none(),
        },
    );
    draw_ruler(
        ui,
        ruler,
        &app.view.arrangement,
        &app.editor.project.time,
        ARRANGEMENT_GRID,
    );
    grid(
        ui,
        Rect::from_min_max(
            body.min,
            pos2(
                body.right(),
                (body.top() + time_height + app.editor.project.tracks.len() as f32 * metrics().track_height)
                    .min(body.bottom()),
            ),
        ),
        &app.view.arrangement,
        &app.editor.project.time,
        ARRANGEMENT_GRID,
    );
    if app.show_time_tracks {
        for (index, text) in [crate::i18n::t("拍号"), crate::i18n::t("曲速")].into_iter().enumerate() {
            let y = header.bottom() + index as f32 * 28.0;
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(rect.left(), y), vec2(gutter, 28.0)),
                0,
                panel_color(),
            );
            label(ui, pos2(rect.left() + 14.0, y + 14.0), text, muted_color());
            ui.painter()
                .hline(rect.x_range(), y + 28.0, Stroke::new(1.0, edge_color()));
        }
        super::time_tracks::show(
            app,
            ui,
            Rect::from_min_size(body.min, vec2(body.width(), time_height)).intersect(body),
        );
    }
    let tracks_rect = Rect::from_min_max(
        pos2(rect.left(), header.bottom() + time_height),
        rect.max - vec2(0.0, 6.0),
    );
    super::arrangement_tracks::show(
        app,
        &mut child(ui, tracks_rect, "tracks"),
        ruler.left(),
        scroll,
    );
    ui.painter()
        .vline(ruler.left(), rect.y_range(), Stroke::new(1.0, edge_color()));
    super::scrollbars::horizontal(
        ui,
        "arrangement-scrollbar",
        Rect::from_min_max(
            pos2(ruler.left() + 2.0, rect.bottom() - 6.0),
            rect.max - vec2(2.0, 0.0),
        ),
        &mut app.view.arrangement,
        beats(app.editor.project.end()) + 32.0,
    );
    if let Some(position) = hover_position {
        hover_playhead(ui,Rect::from_min_max(ruler.min,body.max),ruler.bottom(),&app.view.arrangement,position);
    }
    playhead(
        ui,
        Rect::from_min_max(ruler.min, body.max),
        ruler.bottom(),
        &app.view.arrangement,
        app.editor.transport.position,
    );
    super::rendering_hint::show(ui,body,app.synthesis.is_synthesizing());
}
