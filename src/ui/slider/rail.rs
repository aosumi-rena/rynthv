use crate::ui::theme::*;
use eframe::egui::*;

#[derive(Clone, Copy)]
pub(crate) enum Appearance {
    Voice,
    Instrument(Color32),
}
#[derive(Clone, Copy)]
pub(crate) struct Range {
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub step: f64,
}
impl Range {
    pub fn quantize(self, value: f64) -> f64 {
        (if self.step > 0. {
            self.min + ((value - self.min) / self.step).round() * self.step
        } else {
            value
        })
        .clamp(self.min, self.max)
    }
    pub fn text(self, value: f64) -> String {
        if self.step >= 1. {
            format!("{value:.0}")
        } else {
            format!("{value:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        }
    }
}

pub(crate) fn rail(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    value: &mut f64,
    range: Range,
    appearance: Appearance,
    bipolar: bool,
    mixed: bool,
) -> Response {
    let mut response = ui.interact(rect, id, Sense::click_and_drag());
    let bounds = rect.shrink2(vec2(9.5, 0.));
    let original = *value;
    let mut reset = false;
    if ui.is_enabled() {
        if response.double_clicked() {
            *value = range.default;
            reset = true;
        } else if (response.clicked() || response.dragged())
            && let Some(p) = response.interact_pointer_pos()
        {
            *value = range.quantize(
                range.min
                    + f64::from(((p.x - bounds.left()) / bounds.width().max(1.)).clamp(0., 1.))
                        * (range.max - range.min),
            );
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                        ..Default::default()
                    },
                )
            });
            ui.input(|i| {
                let delta = i32::from(i.key_pressed(Key::ArrowRight))
                    + i32::from(i.key_pressed(Key::ArrowUp))
                    - i32::from(i.key_pressed(Key::ArrowLeft))
                    - i32::from(i.key_pressed(Key::ArrowDown));
                if delta != 0 {
                    let step = if range.step > 0. {
                        range.step
                    } else {
                        (range.max - range.min) / 100.
                    };
                    *value = (*value
                        + f64::from(delta) * step * if i.modifiers.shift { 0.1 } else { 1. })
                    .clamp(range.min, range.max);
                }
                if i.key_pressed(Key::Home) {
                    *value = range.default;
                    reset = true;
                }
            });
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                *value = original;
                response.surrender_focus();
            }
        }
        if *value != original || reset || mixed && (response.clicked() || response.dragged()) {
            response.mark_changed();
        }
    }
    let y = bounds.center().y;
    let fraction = ((*value - range.min) / (range.max - range.min)).clamp(0., 1.) as f32;
    let center = pos2(lerp(bounds.x_range(), fraction), y);
    let origin = if bipolar {
        lerp(
            bounds.x_range(),
            ((range.default - range.min) / (range.max - range.min)) as f32,
        )
    } else {
        bounds.left()
    };
    let painter = ui.painter();
    match appearance {
        Appearance::Voice => {
            painter.hline(bounds.x_range(), y, Stroke::new(2., EDGE));
            painter.line_segment(
                [pos2(origin, y), center],
                Stroke::new(2., if bipolar { TEXT } else { MUTED }),
            );
            painter.circle(center, 9.5, EDGE, Stroke::NONE);
            painter.circle(
                center,
                8.5,
                Color32::from_rgb(65, 63, 68),
                Stroke::new(0.7, BUTTON_ACTIVE),
            );
            painter.circle_filled(center, 3., TEXT);
        }
        Appearance::Instrument(color) => {
            painter.hline(bounds.x_range(), y, Stroke::new(2., BUTTON));
            painter.line_segment([pos2(origin, y), center], Stroke::new(2., color));
            painter.circle(center, 6., color, Stroke::new(0.5, EDGE));
        }
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}
