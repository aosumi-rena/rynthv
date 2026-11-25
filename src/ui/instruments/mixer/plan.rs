use crate::{
    synthesis::instruments::{
        Entry,
        microphones::{Configuration, split},
        rooms::Pack,
    },
    ui::theme::*,
};
use eframe::egui::*;

pub(super) fn show(
    ui: &mut Ui,
    entry: &Entry,
    config: &mut Configuration,
    pack: Option<&Pack>,
    rect: Rect,
    color: Color32,
    scale: f32,
) {
    let font_scale = scale / 0.75;
    let mut content = crate::ui::child(ui, rect, "microphone-plan");
    let ui = &mut content;
    ui.scope(|ui| {
        let inactive = &mut ui.visuals_mut().widgets.inactive;
        inactive.bg_fill = Color32::TRANSPARENT;
        inactive.weak_bg_fill = Color32::TRANSPARENT;
        inactive.bg_stroke = Stroke::NONE;
        ComboBox::from_id_salt("instrument-room")
            .width(160. * scale)
            .selected_text(super::super::native_label(&format!(
                "Mic_{}",
                config.room.name
            )))
            .show_ui(ui, |ui| {
                if let Some(pack) = pack {
                    for room in &pack.rooms {
                        ui.selectable_value(
                            &mut config.room.name,
                            room.name.clone(),
                            super::super::native_label(&format!("Mic_{}", room.name)),
                        );
                    }
                } else if let Some(room) = &entry.room {
                    ui.selectable_value(
                        &mut config.room.name,
                        room.clone(),
                        super::super::native_label(&format!("Mic_{room}")),
                    );
                }
            });
    });
    let fit = scale / 0.75;
    let to_screen = |p: Vec2| rect.min + p * fit;
    let center = to_screen(vec2(190.5, 74.));
    let radius = 38. * scale;
    let mut pin = vec![center + vec2(0., radius + 10. * scale)];
    pin.extend((0..=72).map(|i| {
        let angle =
            std::f32::consts::FRAC_PI_2 + 0.18 + (std::f32::consts::TAU - 0.36) * i as f32 / 72.;
        center + vec2(angle.cos(), angle.sin()) * radius
    }));
    ui.painter()
        .add(Shape::closed_line(pin, Stroke::new(3. * scale, color)));
    let abbreviation = if entry.name == "Orchestral Flute 1" {
        "Fl".to_owned()
    } else {
        entry
            .name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
    };
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        abbreviation,
        FontId::proportional(12. * font_scale),
        color,
    );
    let active = config.mix(&entry.microphones);
    let mut groups = std::collections::BTreeMap::<String, Vec<Pos2>>::new();
    for name in &entry.microphones {
        let (group, side) = split(name);
        let Some((offset, angle)) = position(group, side) else {
            continue;
        };
        let point = to_screen(offset);
        groups.entry(group.into()).or_default().push(point);
        let tint = if active.iter().any(|v| v.name == *name) {
            color
        } else {
            MUTED.gamma_multiply(0.4)
        };
        microphone(
            ui,
            point,
            angle,
            matches!(group, "Wide" | "Ambi"),
            tint,
            font_scale,
        );
    }
    for points in groups.values() {
        if points.len() > 1 {
            let left = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
            let right = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
            let y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
            ui.painter().line_segment(
                [pos2(left + 14. * scale, y), pos2(right - 14. * scale, y)],
                Stroke::new(scale, MUTED.gamma_multiply(0.5)),
            );
            for point in points.iter().filter(|p| p.y < y - 24. * scale) {
                ui.painter().line_segment(
                    [*point + vec2(0., 24. * scale), pos2(point.x, y)],
                    Stroke::new(scale, MUTED.gamma_multiply(0.5)),
                );
            }
        }
    }
    let count = entry.close_microphones.len();
    for (index, (name, mic)) in entry.close_microphones.iter().enumerate() {
        let point = to_screen(vec2(
            190.5 + (index as f32 - (count.saturating_sub(1)) as f32 / 2.) * 18.,
            137.5,
        ));
        microphone(
            ui,
            point,
            0.,
            mic.kind == "condenser",
            if active.iter().any(|v| v.name == *name) {
                color
            } else {
                MUTED.gamma_multiply(0.4)
            },
            font_scale,
        );
    }
}

fn position(group: &str, side: &str) -> Option<(Vec2, f32)> {
    Some(match (group, side) {
        ("Decca", "L") => (vec2(138.5, 312.5), 0.25),
        ("Decca", "C") => (vec2(190.5, 273.5), 0.),
        ("Decca", "R") => (vec2(242.5, 312.5), -0.25),
        ("Wide", "L") => (vec2(86., 355.), 0.4),
        ("Wide", "R") => (vec2(295., 355.), -0.4),
        ("Ambi", "L") => (vec2(175., 366.), 0.05),
        ("Ambi", "R") => (vec2(206., 366.), -0.05),
        _ => return None,
    })
}

fn microphone(ui: &Ui, point: Pos2, angle: f32, large: bool, color: Color32, scale: f32) {
    let (image, size) = if large {
        (
            Image::new(include_image!(
                "../../../../assets/icons/rinstx/microphone-condenser.svg"
            )),
            vec2(12., 29.),
        )
    } else {
        (
            Image::new(include_image!(
                "../../../../assets/icons/rinstx/microphone-pencil.svg"
            )),
            vec2(8., 32.),
        )
    };
    image
        .tint(color)
        .rotate(angle, vec2(0.5, 0.5))
        .paint_at(ui, Rect::from_center_size(point, size * scale));
}
