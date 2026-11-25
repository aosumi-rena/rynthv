mod entry;
mod rail;
use crate::ui::theme::*;
use eframe::egui::*;
pub(crate) use rail::{Appearance, Range, rail};

#[derive(Clone)]
struct Drag {
    source: u64,
    value: f64,
    frame: u64,
}

pub(crate) fn preview(ui: &Ui, id: Id, source: f64) -> f64 {
    let frame = ui.ctx().cumulative_frame_nr();
    ui.ctx()
        .data(|d| d.get_temp::<Drag>(id.with("drag")))
        .filter(|d| d.source == source.to_bits() && d.frame + 1 >= frame)
        .map_or(source, |d| d.value)
}

pub(crate) struct Change {
    pub value: f64,
    pub reset: bool,
}
pub(crate) struct Output {
    pub response: Response,
    pub change: Option<Change>,
}

pub(crate) struct Slider<'a> {
    id: Id,
    label: &'a str,
    source: f64,
    range: Range,
    suffix: &'a str,
    appearance: Appearance,
    bipolar: bool,
    numeric: bool,
    live: bool,
    formatter: Option<&'a dyn Fn(f64) -> String>,
}
impl<'a> Slider<'a> {
    pub fn new(id: Id, label: &'a str, source: f64, min: f64, max: f64, default: f64) -> Self {
        Self {
            id,
            label,
            source,
            range: Range {
                min,
                max,
                default,
                step: 0.,
            },
            suffix: "",
            appearance: Appearance::Voice,
            bipolar: false,
            numeric: false,
            live: false,
            formatter: None,
        }
    }
    pub fn step(mut self, step: f64) -> Self {
        self.range.step = step;
        self
    }
    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }
    pub fn appearance(mut self, appearance: Appearance) -> Self {
        self.appearance = appearance;
        self
    }
    pub fn bipolar(mut self) -> Self {
        self.bipolar = true;
        self
    }
    pub fn numeric(mut self, numeric: bool) -> Self {
        self.numeric = numeric;
        self
    }
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }
    pub fn format(mut self, format: &'a dyn Fn(f64) -> String) -> Self {
        self.formatter = Some(format);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Output {
        let compact = self.label.is_empty();
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), if compact { 28. } else { 51. }),
            Sense::hover(),
        );
        let caption = if compact {
            Rect::from_min_max(
                rect.right_top() - vec2(70., 0.),
                rect.right_top() + vec2(0., 28.),
            )
        } else {
            Rect::from_min_size(rect.right_top() + vec2(-75., 1.), vec2(75., 18.))
        };
        let bounds = Rect::from_min_max(
            rect.min,
            pos2(
                if compact {
                    caption.left() - 6.
                } else {
                    rect.right()
                },
                rect.bottom(),
            ),
        );
        let rail_rect = Rect::from_center_size(
            pos2(
                bounds.center().x,
                bounds.top() + if compact { 14. } else { 32. },
            ),
            vec2(bounds.width().max(20.), 22.),
        );
        let mut value = preview(ui, self.id, self.source);
        let cancel = ui.memory(|m| m.has_focus(self.id.with("rail")))
            && ui.input(|i| i.key_pressed(Key::Escape));
        let mut response = rail(
            ui,
            self.id.with("rail"),
            rail_rect,
            &mut value,
            self.range,
            self.appearance,
            self.bipolar,
            false,
        );
        let reset = response.double_clicked()
            || response.has_focus() && ui.input(|i| i.key_pressed(Key::Home));
        if cancel {
            value = self.source;
        }
        let mut change = None;
        if ui.is_enabled() && !cancel {
            if response.dragged() {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(
                        self.id.with("drag"),
                        Drag {
                            source: self.source.to_bits(),
                            value,
                            frame: ui.ctx().cumulative_frame_nr(),
                        },
                    )
                });
            }
            if response.drag_stopped()
                || response.clicked()
                || reset
                || response.changed() && !response.dragged()
                || self.live && value != self.source
            {
                change = Some(Change { value, reset });
            }
        }
        if !response.dragged() || cancel {
            ui.ctx()
                .data_mut(|d| d.remove::<Drag>(self.id.with("drag")));
        }
        if !compact {
            ui.painter().text(
                rect.min + vec2(0., 1.),
                Align2::LEFT_TOP,
                self.label,
                FontId::proportional(12.),
                MUTED,
            );
        }
        let format = |v| self.formatter.map_or_else(|| self.range.text(v), |f| f(v));
        if let Some(next) = entry::show(
            ui,
            self.id,
            caption,
            self.source,
            value,
            self.range,
            &format,
            self.suffix,
            self.numeric || compact || response.hovered() || response.dragged(),
        ) {
            change = Some(Change {
                value: next,
                reset: false,
            });
        }
        if change.is_some() {
            response.mark_changed();
        }
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Slider, ui.is_enabled(), self.label));
        Output { response, change }
    }
}

pub(crate) fn value<N: emath::Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut N,
    min: f64,
    max: f64,
    default: N,
    step: f64,
    suffix: &str,
) -> Response {
    let id = ui.make_persistent_id(label);
    let output = Slider::new(id, "", value.to_f64(), min, max, default.to_f64())
        .step(step)
        .suffix(suffix)
        .numeric(true)
        .live()
        .show(ui);
    if let Some(change) = output.change {
        *value = N::from_f64(change.value);
    }
    output.response
}
