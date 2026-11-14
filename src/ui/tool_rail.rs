use super::{
    child,
    icons::{Icon, IconButton},
    state::Column,
    theme::*,
};
use crate::app::App;
use eframe::egui::*;

pub const WIDTH: f32 = 42.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();

    ui.painter().rect_filled(rect, 4, PANEL);
    ui.painter()
        .rect_stroke(rect, 4, Stroke::new(1.0, BORDER), StrokeKind::Inside);

    let mut content = child(ui, rect.shrink2(vec2(3.0, 5.0)), "rail-content");
    content.spacing_mut().item_spacing.y = 12.0;
    for panel in Column::PANELS {
        if content
            .add(button(panel.icon(), panel.label()).selected(app.view.columns.contains(&panel)))
            .clicked()
        {
            app.view.toggle(panel);
        }
    }

    let mut lab = button(Icon::Retake, crate::i18n::t("VOICE_LAB")).selected(app.voice_finetuning.open);
    if app.voice_finetuning.open { lab = lab.background(GREEN.gamma_multiply(0.58)); }
    if content.add_enabled(app.rocoflex.provider.is_some(), lab).on_disabled_hover_text(crate::i18n::t("ROCOFLEX_UNAVAILABLE")).clicked() {
        app.voice_finetuning.open = !app.voice_finetuning.open;
    }

    content.add_space(7.0);
    let (separator, _) = content.allocate_exact_size(vec2(WIDTH - 6.0, 1.0), Sense::hover());
    content.painter().hline(
        separator.x_range(),
        separator.center().y,
        Stroke::new(1.0, EDGE),
    );
    content.add_space(5.0);
    if content.add(button(Icon::Cloud, crate::i18n::t("管理歌声"))).clicked() {
        app.voice_manager = true;
    }
    if content.add(button(Icon::Settings, crate::i18n::t("设置"))).clicked() {
        app.settings = true;
    }
}

fn button(icon: Icon, label: &'static str) -> IconButton {
    IconButton::new(icon, label)
        .size(Vec2::splat(WIDTH - 6.0))
        .icon_size(20.0)
}
