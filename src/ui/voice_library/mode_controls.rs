use super::mode_knob;
use crate::model::{EntityId, PublicModeComponent, PublicModeControl};
use eframe::egui::*;
use crate::ui::icons::{Icon, IconButton};

const CARD: Color32 = Color32::from_rgb(51, 49, 52);
const CAPTION: Color32 = Color32::from_rgb(163, 157, 166);

fn snap_id() -> Id {Id::new("vocal-mode-snap")}

pub(super) fn snap_button(ui: &mut Ui) {
    let mut enabled = snap_enabled(ui.ctx());
    if ui.add(IconButton::new(Icon::Magnet,crate::i18n::t("唱法对齐到 5%"))
        .size(vec2(32.0,24.0)).framed().selected(enabled)).clicked()
    {
        enabled = !enabled;
        ui.ctx().data_mut(|data| data.insert_temp(snap_id(),enabled));
    }
}

fn snap_enabled(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(snap_id())).unwrap_or(true)
}

pub(super) fn show(ui: &mut Ui, voice_key: &str, reference: EntityId, slot: usize,
    name: &str, control: &PublicModeControl, unavailable: Option<&str>, independent: bool, hidden: bool)
    -> Vec<(PublicModeComponent, f64)>
{
    let mut changes = Vec::new();
    let snap = snap_enabled(ui.ctx());
    ui.push_id(("vocal-mode", voice_key, reference, name), |ui| {

        let expanded_id = Id::new(("native-vocal-mode-expanded",slot));
        let mut expanded = independent && ui.ctx().data(|data| data.get_temp::<bool>(expanded_id)).unwrap_or(false);
        let (card, _) = ui.allocate_exact_size(vec2(ui.available_width(), 100.0), Sense::hover());
        ui.painter().rect_filled(card, 3, CARD);
        ui.painter().text(card.min + vec2(12.0, 9.0), Align2::LEFT_TOP,
            name, FontId::proportional(13.0), CAPTION);
        if hidden {
            ui.painter().text(pos2(card.right()-36.0, card.top()+10.0), Align2::RIGHT_TOP,
                crate::i18n::t("隐藏唱法"), FontId::proportional(10.0), CAPTION);
        }
        let toggle_rect = Rect::from_min_size(card.min + vec2(card.width()-30.0, 4.0), Vec2::splat(24.0));
        let toggle = ui.interact(toggle_rect, ui.id().with("expand"), Sense::click());
        let center = toggle_rect.center();
        ui.painter().line_segment([center-vec2(5.0,0.0), center+vec2(5.0,0.0)], Stroke::new(1.0,CAPTION));
        if !expanded {
            ui.painter().line_segment([center-vec2(0.0,5.0), center+vec2(0.0,5.0)], Stroke::new(1.0,CAPTION));
        }
        toggle.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true,
            if expanded {crate::i18n::t("折叠唱法")} else {crate::i18n::t("展开唱法")}));
        if toggle.clicked() {
            if independent {
                expanded = !expanded;
                ui.ctx().data_mut(|data| data.insert_temp(expanded_id, expanded));
            } else {
                ui.ctx().data_mut(|data|data.insert_temp(Id::new("unsupported-style-lanes"),true));
            }
        }
        let components = [PublicModeComponent::Pitch, PublicModeComponent::Timbre,
            PublicModeComponent::Pronunciation];
        let source = [control.pitch, control.timbre, control.pronunciation];
        if expanded {
            let width = (card.width()-20.0)/3.0;
            for (index, label) in [crate::i18n::t("音高"), crate::i18n::t("音色"), crate::i18n::t("发音")].into_iter().enumerate() {
                let rect = Rect::from_min_size(card.min + vec2(7.0+index as f32*(width+3.0), 34.0), vec2(width,60.0));
                if let Some(values) = mode_knob::show(ui, rect, ui.id().with(index), source,
                    Some(index), label, snap, unavailable)
                { changes.push((components[index], values[index])); }
            }
        } else {
            let rect = Rect::from_min_size(pos2(card.center().x-32.0,card.top()+25.0),vec2(64.0,72.0));
            if let Some(values) = mode_knob::show(ui, rect, ui.id().with("linked"), source,
                None, "", snap, unavailable)
            { changes.extend(components.into_iter().zip(values)); }
        }
    });
    changes
}
