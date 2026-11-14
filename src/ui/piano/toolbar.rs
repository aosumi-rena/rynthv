use crate::{
    app::App,
    ui::{
        icons::{Icon, IconButton},
        state::DragPayload,
        theme::*,
    },
};
use eframe::egui::*;

pub fn toolbar(app: &mut App, ui: &mut Ui) {
    ui.style_mut().always_scroll_the_only_direction = true;
    ScrollArea::horizontal()
        .id_salt("piano-toolbar-scroll")
        .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, false])
        .show(ui, |ui| contents(app, ui));
}

fn contents(app: &mut App, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        title(app, ui);
        editing_tools(app, ui);
        ui.add_space(15.0);
        for (icon, label, active) in [
            (Icon::Pitch, crate::i18n::t("显示音高曲线"), &mut app.show_pitch),
            (Icon::Waveform, crate::i18n::t("显示渲染波形"), &mut app.show_waveform),
            (Icon::Retake, crate::i18n::t("显示 AI 重录小部件"), &mut app.show_retakes),
        ] {
            if ui
                .add(
                    IconButton::new(icon, label)
                        .framed()
                        .size(vec2(32.0, 24.0))
                        .selected(*active),
                )
                .clicked()
            {
                *active = !*active;
            }
            ui.add_space(7.0);
        }
        super::scale_guide::toggle(app, ui);
        ui.add_space(7.0);
        ui.label(RichText::new(crate::i18n::t("网格")).color(MUTED));
        ui.add_space(8.0);
        ComboBox::from_id_salt("grid")
            .width(96.0)
            .selected_text(crate::lformat!(
                "{} 分{}",
                app.view.grid,
                if app.view.triplet {
                    crate::i18n::t("三连音")
                } else {
                    crate::i18n::t("音符")
                }
            ))
            .show_ui(ui, |ui| {
                for value in [4, 8, 16, 32, 64, 128] {
                    if ui
                        .selectable_label(
                            app.view.grid == value && !app.view.triplet,
                            crate::lformat!("{value} 分音符"),
                        )
                        .clicked()
                    {
                        app.view.grid = value;
                        app.view.triplet = false;
                    }
                }
                for value in [8, 16, 32, 64] {
                    if ui
                        .selectable_label(
                            app.view.grid == value && app.view.triplet,
                            crate::lformat!("{value} 分三连音"),
                        )
                        .clicked()
                    {
                        app.view.grid = value;
                        app.view.triplet = true;
                    }
                }
            });
        ui.add_space(9.0);
        if ui
            .add(
                IconButton::new(Icon::Magnet, crate::i18n::t("对齐网格"))
                    .size(vec2(32.0, 24.0))
                    .framed()
                    .selected(app.view.snap),
            )
            .clicked()
        {
            app.view.snap = !app.view.snap;
        }
        crate::ui::scale::controls::toolbar(app, ui);
    });
}

fn title(app: &mut App, ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(110.0, 28.0), Sense::hover());
    let mut header = crate::ui::child(ui, rect, "piano-title");
    header.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(4.0);
        if ui
            .add(
                IconButton::new(
                    if app.piano_collapsed {
                        Icon::ChevronRight
                    } else {
                        Icon::ChevronDown
                    },
                    crate::i18n::t("展开 / 折叠钢琴卷帘"),
                )
                .size(vec2(20.0, 24.0))
                .icon_size(12.0),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_collapsed = !app.piano_collapsed;
            ui.ctx().request_repaint();
        }
        ui.add_space(3.0);
        crate::ui::section_title(ui, crate::i18n::t("钢琴卷帘"), DragPayload::Piano);
    });
}

fn editing_tools(app: &mut App, ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(160.0, 24.0), Sense::hover());
    ui.painter().rect_filled(rect, 3, BUTTON);
    let mut group = crate::ui::child(ui, rect, "piano-editing-tools");
    group.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if ui
            .add(
                IconButton::new(Icon::NoteSelect, crate::i18n::t("编辑音符（Alt+1）"))
                    .size(vec2(40.0, 24.0))
                    .grouped()
                    .selected(app.piano_tool == super::Tool::NoteSelect),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_tool = super::Tool::NoteSelect;
            app.status.clear();
        }
        if ui
            .add(
                IconButton::new(Icon::NoteDraw, crate::i18n::t("创建音符（Alt+2）"))
                    .size(vec2(40.0, 24.0))
                    .grouped()
                    .selected(app.piano_tool == super::Tool::NoteDraw),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_tool = super::Tool::NoteDraw;
            app.status.clear();
        }
        if ui
            .add(
                IconButton::new(Icon::PitchAnchor, crate::i18n::t("编辑音高控制点（Alt+3）"))
                    .size(vec2(40.0, 24.0))
                    .grouped()
                    .selected(app.piano_tool == super::Tool::PitchAnchor),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_tool = super::Tool::PitchAnchor;
            app.editor.selected.clear();
            app.show_pitch = true;
            app.status.clear();
        }
        if ui
            .add(
                IconButton::new(Icon::PitchDraw, crate::i18n::t("手绘音高（Alt+4）：左键绘制，右键擦除"))
                    .size(vec2(40.0, 24.0))
                    .grouped()
                    .selected(app.piano_tool == super::Tool::PitchDraw),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_tool = super::Tool::PitchDraw;
            app.show_pitch = true;
            app.status.clear();
        }
    });
}
