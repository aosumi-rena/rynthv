use crate::{synthesis::voice::{VoiceCatalogEntry, VoiceFamily}, ui::theme::TEXT};
use eframe::egui::*;

pub(super) const LABELS: [&str; 6] = ["R3 AI 2", "R3 AI", "R2 AI", "R2 AI Flat", "R2 STD", "R1 STD"];

pub(super) fn category_of(entry: &VoiceCatalogEntry) -> u8 {
    match entry.family {
        Some(VoiceFamily::Sv2) => 0,
        Some(VoiceFamily::Sv2Compatible) => 1,
        Some(VoiceFamily::Sv1) => if entry.database_id.starts_with("flat:") {3} else {2},
        Some(VoiceFamily::R2Std) => 4,
        Some(VoiceFamily::R1Std) => 5,
        None => 6,
    }
}

#[derive(Clone, Copy)]
pub(super) struct FamilyFilter([bool; 6]);

impl Default for FamilyFilter {
    fn default() -> Self { Self([true; 6]) }
}

impl FamilyFilter {
    pub(super) fn includes(self, entry: &VoiceCatalogEntry) -> bool {
        // Unclassified entries remain available in the unfiltered catalogue.
        self.0.get(category_of(entry) as usize).copied().unwrap_or(self.all())
    }

    fn all(self) -> bool { self.0.iter().all(|selected| *selected) }

    pub(super) fn show(&mut self, ui: &mut Ui) -> bool {
        let before = self.0;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(5.0, 4.0);
            for (index, label) in LABELS.iter().enumerate() {
                if toggle(ui, label, self.0[index]).clicked() {
                    self.0[index] = !self.0[index];
                }
            }
            if toggle(ui, crate::i18n::t("VOICE_FILTER_ALL"), self.all()).clicked() {
                let selected = !self.all();
                self.0.fill(selected);
            }
        });
        self.0 != before
    }
}

fn toggle(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(11.0), TEXT);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 24.0), Sense::click());
    let accent = ui.visuals().selection.stroke.color;
    let stroke = if selected {
        Stroke::new(1.0, if response.hovered() { accent.gamma_multiply(0.5) } else { accent })
    } else {
        Stroke::NONE
    };
    ui.painter().rect_stroke(rect.shrink(0.5), 4, stroke, StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT);
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), selected, label));
    response.on_hover_cursor(CursorIcon::Default)
}
