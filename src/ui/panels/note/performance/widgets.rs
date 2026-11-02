use crate::ui::theme::*;
use eframe::egui::*;

pub(super) fn expression(ui: &mut Ui, value: &mut [f64; 2], mixed: bool) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::splat(ui.available_width()), Sense::click_and_drag());
    let bounds = rect.shrink(9.0);
    if ui.is_enabled() {
        let original = *value;
        if response.double_clicked() {
            *value = [0.5; 2];
        } else if let Some(pointer) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            value[0] = ((pointer.x - bounds.left()) / bounds.width()).clamp(0.0, 1.0) as f64;
            value[1] = ((bounds.bottom() - pointer.y) / bounds.height()).clamp(0.0, 1.0) as f64;
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.has_focus() {
            lock_focus(ui, &response);
            ui.input(|input| {
                let step = if input.modifiers.shift { 0.001 } else { 0.01 };
                value[0] += key_delta(input, Key::ArrowLeft, Key::ArrowRight) * step;
                value[1] += key_delta(input, Key::ArrowDown, Key::ArrowUp) * step;
                if input.key_pressed(Key::Home) {
                    *value = [0.5; 2];
                }
            });
        }
        value.iter_mut().for_each(|v| *v = v.clamp(0.0, 1.0));
        if *value != original
            || response.double_clicked()
            || response.has_focus() && ui.input(|input| input.key_pressed(Key::Home))
            || mixed && (response.clicked() || response.dragged())
        {
            response.mark_changed();
        }
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 4, Color32::from_rgb(31, 30, 34));
    painter.hline(rect.x_range(), rect.center().y, Stroke::new(1.0, BUTTON));
    painter.vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BUTTON));
    for (position, anchor, text) in [
        (rect.min + vec2(8.0, 8.0), Align2::LEFT_TOP, crate::i18n::t("鲜活")),
        (
            rect.right_top() + vec2(-8.0, 8.0),
            Align2::RIGHT_TOP,
            crate::i18n::t("精修"),
        ),
        (
            rect.left_bottom() + vec2(8.0, -8.0),
            Align2::LEFT_BOTTOM,
            crate::i18n::t("机械"),
        ),
        (rect.max - vec2(8.0, 8.0), Align2::RIGHT_BOTTOM, crate::i18n::t("原生")),
    ] {
        painter.text(position, anchor, text, FontId::proportional(12.0), MUTED);
    }
    let center = pos2(
        bounds.left() + bounds.width() * value[0] as f32,
        bounds.bottom() - bounds.height() * value[1] as f32,
    );
    painter.circle_filled(center, 7.0, PANEL);
    painter.circle_stroke(
        center,
        8.0,
        Stroke::new(2.0, if mixed { MUTED } else { TEXT }),
    );
    if mixed {
        painter.text(
            rect.center() + vec2(0.0, 18.0),
            Align2::CENTER_TOP,
            crate::i18n::t("多个值"),
            FontId::proportional(11.0),
            MUTED,
        );
    }
    response.on_hover_cursor(CursorIcon::Crosshair)
}

pub(super) fn slider(
    ui: &mut Ui,
    value: &mut f64,
    mixed: bool,
    default: f64,
    label: &str,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click_and_drag());
    let bounds = rect.shrink2(vec2(9.0, 0.0));
    if ui.is_enabled() {
        let original = *value;
        if response.double_clicked() {
            *value = default;
        } else if let Some(pointer) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            *value = ((pointer.x - bounds.left()) / bounds.width()).clamp(0.0, 1.0) as f64;
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.has_focus() {
            lock_focus(ui, &response);
            ui.input(|input| {
                let step = if input.modifiers.shift { 0.001 } else { 0.01 };
                *value += (key_delta(input, Key::ArrowLeft, Key::ArrowRight)
                    + key_delta(input, Key::ArrowDown, Key::ArrowUp))
                    * step;
                if input.key_pressed(Key::Home) {
                    *value = default;
                }
            });
        }
        *value = value.clamp(0.0, 1.0);
        if *value != original
            || response.double_clicked()
            || response.has_focus() && ui.input(|input| input.key_pressed(Key::Home))
            || mixed && (response.clicked() || response.dragged())
        {
            response.mark_changed();
        }
    }
    let center = pos2(
        bounds.left() + bounds.width() * *value as f32,
        rect.center().y,
    );
    ui.painter()
        .hline(bounds.x_range(), center.y, Stroke::new(2.0, EDGE));
    ui.painter()
        .hline(bounds.left()..=center.x, center.y, Stroke::new(2.0, MUTED));
    ui.painter()
        .circle(center, 9.0, BUTTON, Stroke::new(0.7, BUTTON_ACTIVE));
    ui.painter().circle_filled(center, 3.0, TEXT);
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(crate::lformat!(
            "拖动调整所选音符的{label}；方向键微调，Shift 加方向键精细调整。双击或 Home 恢复默认，Esc 取消本次拖动。"
        ))
}

pub(super) fn labeled_slider(
    ui: &mut Ui,
    label: &str,
    text: String,
    value: &mut f64,
    mixed: bool,
    default: f64,
) -> Response {
    ui.horizontal(|ui| {
        ui.weak(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.weak(text));
    });
    slider(ui, value, mixed, default, label)
}

fn key_delta(input: &InputState, negative: Key, positive: Key) -> f64 {
    i32::from(input.key_pressed(positive)) as f64 - i32::from(input.key_pressed(negative)) as f64
}

fn lock_focus(ui: &Ui, response: &Response) {
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            response.id,
            EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
                ..Default::default()
            },
        );
    });
    if ui.input(|input| input.key_pressed(Key::Escape) && !input.pointer.primary_down()) {
        response.surrender_focus();
    }
}
