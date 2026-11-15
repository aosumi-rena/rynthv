use crate::{app::App, ui::resource_manager};
use eframe::egui::*;
pub(super) fn show(app: &mut App, ui: &mut Ui, rect: Rect) -> bool {
    resource_manager::header(
        ui,
        rect,
        crate::i18n::t("歌声"),
        crate::i18n::t("关闭歌声管理"),
        |ui| super::super::paths::settings(app, ui),
    )
}
