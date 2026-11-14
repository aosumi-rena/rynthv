use super::theme::{CANVAS, TEXT};
use crate::model::{Track, TrackColor};
use eframe::egui::*;

#[derive(Clone, Copy)]
pub struct TrackColors {
    pub accent: Color32,
    pub note: Color32,
    pub inactive: Color32,
    pub highlight: Color32,
}

impl TrackColors {
    pub fn new(track: &Track) -> Self {
        let accent = color32(track.disp_color);
        Self {
            accent,
            note: accent.lerp_to_gamma(Color32::BLACK, 0.30),
            inactive: accent.lerp_to_gamma(CANVAS, 0.60),
            highlight: accent.lerp_to_gamma(Color32::WHITE, 0.40),
        }
    }
}

fn color32(color: TrackColor) -> Color32 {
    let [r, g, b, a] = color.rgba();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

pub fn foreground(color: Color32) -> Color32 {
    let linear = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance =
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b());
    if luminance > 0.20 {
        CANVAS
    } else {
        Color32::from_gray(240)
    }
}

pub fn picker(ui: &mut Ui, current: TrackColor) -> Option<TrackColor> {
    let mut picked = None;
    ui.scope(|ui| {
        ui.label(RichText::new(crate::i18n::t("音轨颜色")).size(11.0).color(TEXT));
        ui.add_space(5.0);
        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
        for row in TrackColor::PALETTE.chunks(6) {
            ui.horizontal(|ui| {
                for &(name, color) in row {
                    let selected = current == color;
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                    response.widget_info(|| {
                        WidgetInfo::selected(
                            WidgetType::SelectableLabel,
                            ui.is_enabled(),
                            selected,
                            crate::i18n::t(name),
                        )
                    });
                    ui.painter()
                        .rect_filled(rect.shrink(2.0), 2, color32(color));
                    if selected || response.hovered() || response.has_focus() {
                        ui.painter().rect_stroke(
                            rect,
                            3,
                            Stroke::new(1.0, if selected { Color32::WHITE } else { TEXT }),
                            StrokeKind::Inside,
                        );
                    }
                    if response
                        .on_hover_cursor(CursorIcon::Default)
                        .on_hover_text(crate::i18n::t(name))
                        .clicked()
                    {
                        picked = Some(color);
                    }
                }
            });
        }
    });
    picked
}
