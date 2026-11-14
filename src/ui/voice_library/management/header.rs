use super::{super::paths, INK};
use crate::{
    app::App,
    ui::{
        child,
        icons::{Icon, IconButton},
        theme::*,
    },
};
use eframe::egui::*;

pub(super) const HEIGHT: f32 = 68.0;

pub(super) fn show(app: &mut App, ui: &mut Ui, rect: Rect) -> bool {
    let wide = rect.width() >= 800.0;
    let main = Rect::from_min_max(
        rect.min + vec2(22.0, 12.0),
        pos2(rect.right() - 18.0, rect.top() + 54.0),
    );
    let mut bar = child(ui, main, "voice-manager-header");
    let mut close = false;
    bar.spacing_mut().interact_size.y = 34.0;
    bar.horizontal(|ui| {
        if rect.width() >= 420.0 {
            let (logo, _) = ui.allocate_exact_size(Vec2::splat(27.0), Sense::hover());
            Icon::Brand.paint(ui, logo, Color32::WHITE);
        }
        ui.label(
            RichText::new("RynthV")
                .font(section_title_font())
                .size(18.0)
                .color(INK),
        );
        if wide {
            ui.label(RichText::new(crate::i18n::t("歌声")).size(13.0).color(MUTED));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            close = ui
                .add(
                    IconButton::new(Icon::Close, crate::i18n::t("关闭歌声管理"))
                        .size(Vec2::splat(28.0))
                        .icon_size(15.0),
                )
                .clicked();
            let resources = ui.add(
                IconButton::new(Icon::Settings, crate::i18n::t("资源目录设置"))
                    .size(Vec2::splat(28.0))
                    .icon_size(18.0),
            );
            Popup::from_toggle_button_response(&resources)
                .width((ui.ctx().content_rect().width() - 48.0).clamp(180.0, 380.0))
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        ScrollArea::vertical()
                            .id_salt("voice-manager-directories")
                            .max_height((ui.ctx().content_rect().height() - 100.0).max(120.0))
                            .show(ui, |ui| paths::settings(app, ui));
                    });
                });
        });
    });
    close
}
