use super::widgets::section;
use crate::ui::theme::*;
use eframe::egui::*;

pub(super) fn show(ui: &mut Ui) {
    ui.weak(crate::i18n::t("当前快捷键；自定义绑定尚未接入。"));
    for (title, entries) in [
        (
            crate::i18n::t("工程"),
            &[
                (crate::i18n::t("新建"), "Ctrl+N"),
                (crate::i18n::t("打开"), "Ctrl+O"),
                (crate::i18n::t("保存"), "Ctrl+S"),
                (crate::i18n::t("另存为"), "Ctrl+Shift+S"),
            ][..],
        ),
        (
            crate::i18n::t("编辑"),
            &[
                (crate::i18n::t("撤销"), "Ctrl+Z"),
                (crate::i18n::t("重做"), "Ctrl+Y / Ctrl+Shift+Z"),
                (crate::i18n::t("全选音符"), "Ctrl+A"),
                (crate::i18n::t("复制 / 剪切 / 粘贴"), "Ctrl+C / X / V"),
                (crate::i18n::t("重复音符"), "Ctrl+D"),
                (crate::i18n::t("删除音符"), "Delete"),
                (crate::i18n::t("取消选择"), "Esc"),
                (crate::i18n::t("填入歌词"), "Ctrl+L"),
                (crate::i18n::t("静音音符"), "Ctrl+M"),
                (crate::i18n::t("重录新的采样"), "Alt+T"),
                (crate::i18n::t("裁剪保留使用采样"), "Shift+Alt+T"),
                (crate::i18n::t("升高 / 降低八度"), "Ctrl+↑ / ↓"),
                (crate::i18n::t("音符面板"), "Ctrl+B"),
            ][..],
        ),
        (
            crate::i18n::t("走带控制"),
            &[
                (crate::i18n::t("播放 / 暂停"), "Space"),
                (crate::i18n::t("停止"), "Ctrl+Space"),
                (crate::i18n::t("跳到结尾"), "Ctrl+End"),
                (crate::i18n::t("按网格移动走带"), "← / →"),
            ][..],
        ),
        (
            crate::i18n::t("工具与视野"),
            &[
                (crate::i18n::t("选择 / 铅笔工具"), "Alt+1 / Alt+2"),
                (crate::i18n::t("创建音符 / 编辑歌词"), crate::i18n::t("双击")),
                (crate::i18n::t("平移视野"), crate::i18n::t("中键拖动")),
                (crate::i18n::t("水平滚动"), crate::i18n::t("Shift+滚轮")),
                (crate::i18n::t("缩放"), crate::i18n::t("Ctrl+滚轮")),
            ][..],
        ),
    ] {
        section(ui, title);
        for (label, key) in entries {
            ui.horizontal(|ui| {
                ui.set_min_height(28.0);
                ui.label(*label);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    Frame::new()
                        .fill(BUTTON)
                        .corner_radius(3)
                        .inner_margin(Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.label(RichText::new(crate::ui::chrome::shortcut_text(key)).color(TEXT));
                        });
                });
            });
        }
    }
}
