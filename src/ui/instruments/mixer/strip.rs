use crate::{
    synthesis::instruments::microphones::{Configuration, split},
    ui::theme::*,
};
use eframe::egui::*;

pub(super) fn width(name: &str, names: &[String]) -> f32 {
    match names.iter().filter(|n| split(n).0 == name).count() {
        0 => 0.,
        1 => 144.,
        n => n as f32 * 72.,
    }
}

pub(super) fn show(
    ui: &mut Ui,
    rect: Rect,
    name: &str,
    names: &[String],
    config: &mut Configuration,
    scale: f32,
    color: Color32,
) {
    let font_scale = scale / 0.75;
    let mut channels: Vec<_> = names.iter().filter(|n| split(n).0 == name).collect();
    channels.sort_by_key(|n| match split(n).1 {
        "L" => 0,
        "C" => 1,
        "R" => 2,
        _ => 0,
    });
    if channels.is_empty() {
        return;
    }
    let group = config
        .group_mut(name)
        .expect("materialized microphone group");
    ui.painter().rect_filled(
        rect,
        match name {
            "Close" => CornerRadius {
                nw: 10,
                sw: 10,
                ..Default::default()
            },
            "Ambi" => CornerRadius {
                ne: 10,
                se: 10,
                ..Default::default()
            },
            _ => CornerRadius::ZERO,
        },
        BORDER,
    );
    let bounds = rect.shrink(12. * scale);
    let mut content = crate::ui::child(ui, bounds, ("microphone-strip", name));
    let ui = &mut content;
    let heading = Rect::from_min_size(bounds.min, vec2(bounds.width(), 24. * font_scale));
    super::strip_header::show(ui, heading, name, group, channels.len() > 1, scale);
    let channel_width = bounds.width() / channels.len() as f32;
    for (index, channel) in channels.iter().enumerate() {
        let channel_rect = Rect::from_min_size(
            bounds.min + vec2(index as f32 * channel_width, 56. * scale),
            vec2(channel_width, bounds.height() - 56. * scale),
        );
        let mut column = crate::ui::child(ui, channel_rect, (name, channel));
        let key = split(channel).1;
        let mic = group
            .mics
            .iter()
            .find(|v| v.key == key)
            .unwrap()
            .value
            .clone();
        let mut gain = if mic.gain > 0. {
            20. * mic.gain.log10()
        } else {
            -60.
        };
        column.vertical_centered(|ui| {
            let side = match key {
                "L" => "Mic_Left",
                "R" => "Mic_Right",
                "C" => "Mic_Center",
                _ => "Mic_Front",
            };
            let text = super::super::native_label(side);
            let (label, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 26. * scale), Sense::hover());
            ui.painter().text(
                label.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(12. * font_scale),
                MUTED,
            );
            ui.add_space(5. * scale - ui.spacing().item_spacing.y);
            let changed = fader(ui, &mut gain, 420. * scale, scale, color);
            ui.add_space(16. * scale - ui.spacing().item_spacing.y);
            let edited = ui
                .scope(|ui| {
                    let inactive = &mut ui.visuals_mut().widgets.inactive;
                    inactive.bg_fill = Color32::TRANSPARENT;
                    inactive.weak_bg_fill = Color32::TRANSPARENT;
                    inactive.bg_stroke = Stroke::NONE;
                    ui.add(
                        DragValue::new(&mut gain)
                            .range(-60. ..=6.)
                            .speed(0.1)
                            .fixed_decimals(1),
                    )
                    .changed()
                })
                .inner;
            if changed || edited {
                let next = 10f32.powf(gain / 20.);
                for other in &mut group.mics {
                    if other.key == key {
                        other.value.gain = next;
                    } else if group.link {
                        other.value.gain = (other.value.gain * next / mic.gain.max(0.001))
                            .clamp(0.001, 10f32.powf(0.3));
                    }
                }
            }
        });
    }
}

fn fader(ui: &mut Ui, db: &mut f32, height: f32, scale: f32, color: Color32) -> bool {
    let fit = scale / 0.75;
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(72. * fit), height),
        Sense::click_and_drag(),
    );
    let top = rect.top() + 27.5 * fit;
    let bottom = rect.bottom() - 2.5 * fit;
    let mut changed = false;
    if (response.clicked() || response.dragged())
        && let Some(p) = response.interact_pointer_pos()
    {
        let fraction = ((bottom - p.y) / (bottom - top)).clamp(0., 1.);
        let next = -60. + 66. * fraction.sqrt();
        changed = *db != next;
        *db = next;
    }
    if response.double_clicked() {
        changed = *db != 0.;
        *db = 0.;
    }
    let y = bottom - (((*db + 60.) / 66.).clamp(0., 1.)).powi(2) * (bottom - top);
    let x = rect.center().x;
    let painter = ui.painter();
    painter.line_segment(
        [pos2(x, top), pos2(x, bottom)],
        Stroke::new(5. * scale, ROW),
    );
    painter.line_segment(
        [pos2(x, y), pos2(x, bottom)],
        Stroke::new(3. * scale, color),
    );
    for v in [6., 0., -6., -12., -24., -36., -60.] {
        let y = bottom - ((v + 60.) / 66f32).powi(2) * (bottom - top);
        painter.line_segment(
            [pos2(x + 15. * scale, y), pos2(x + 20. * scale, y)],
            Stroke::new(scale, MUTED.gamma_multiply(0.7)),
        );
    }
    let handle = Rect::from_center_size(pos2(x, y), vec2(26., 42.) * scale);
    painter.rect_filled(handle, 20. * scale, BUTTON_HOVER.gamma_multiply(0.9));
    painter.rect_stroke(
        handle,
        20. * scale,
        Stroke::new(scale, MUTED.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    painter.line_segment(
        [pos2(x - 8. * scale, y), pos2(x + 8. * scale, y)],
        Stroke::new(2.5 * scale, TEXT),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true, crate::i18n::t("Gain")));
    changed
}
