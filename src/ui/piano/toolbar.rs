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
    if current()==InterfaceTheme::Classic {classic_contents(app,ui);return;}
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        title(app, ui);
        editing_tools(app, ui);
        ui.add_space(metrics().piano_tools_gap);
        for (icon, label, active) in [
            (Icon::Pitch, crate::i18n::t("显示音高曲线"), &mut app.show_pitch),
            (Icon::Waveform, crate::i18n::t("显示渲染波形"), &mut app.show_waveform),
            (Icon::Retake, crate::i18n::t("显示 AI 重录小部件"), &mut app.show_retakes),
        ] {
            if ui
                .add(
                    IconButton::new(icon, label)
                        .framed().tool()
                        .size(vec2(metrics().piano_toggle_width, metrics().piano_control_height))
                        .selected(*active),
                )
                .clicked()
            {
                *active = !*active;
            }
            ui.add_space(metrics().piano_toggle_gap);
        }
        super::scale_guide::toggle(app, ui);
        ui.add_space(metrics().piano_toggle_gap);
        ui.label(RichText::new(crate::i18n::t("网格")).color(muted_color()));
        ui.add_space(8.0);
        crate::ui::theme::native::combo("grid")
            .width(if current() == InterfaceTheme::Classic { 119.0 } else { 66.0 })
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
                    .size(vec2(metrics().piano_toggle_width, metrics().piano_control_height))
                    .framed().tool()
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
    let (rect, _) = ui.allocate_exact_size(vec2(metrics().piano_title_width, metrics().piano_title_height), Sense::hover());
    let mut header = crate::ui::child(ui, rect, "piano-title");
    header.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(metrics().piano_title_padding);
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
                .size(vec2(metrics().piano_collapse_width, metrics().piano_collapse_height))
                .icon_size(12.0),
            )
            .clicked()
        {
            app.commit_lyric();
            app.piano_collapsed = !app.piano_collapsed;
            ui.ctx().request_repaint();
        }
        ui.add_space(metrics().piano_title_gap);
        crate::ui::section_title(ui, crate::i18n::t("钢琴卷帘"), DragPayload::Piano);
    });
}

fn editing_tools(app: &mut App, ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(metrics().piano_tool_width * 4.0, metrics().piano_control_height), Sense::hover());
    ui.painter().rect_filled(rect, 3, button_color());
    let mut group = crate::ui::child(ui, rect, "piano-editing-tools");
    group.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if ui
            .add(
                IconButton::new(Icon::NoteSelect, crate::i18n::t("编辑音符（Alt+1）"))
                    .size(vec2(metrics().piano_tool_width, metrics().piano_control_height))
                    .grouped().tool()
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
                    .size(vec2(metrics().piano_tool_width, metrics().piano_control_height))
                    .grouped().tool()
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
                    .size(vec2(metrics().piano_tool_width, metrics().piano_control_height))
                    .grouped().tool()
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
                    .size(vec2(metrics().piano_tool_width, metrics().piano_control_height))
                    .grouped().tool()
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


fn classic_contents(app: &mut App, ui: &mut Ui) {
    let (row,_) = ui.allocate_exact_size(vec2(ui.available_width().max(840.0),26.0),Sense::hover());
    let mut heading=crate::ui::child(ui,Rect::from_min_size(row.min,vec2(126.0,26.0)),"classic-piano-title");
    title(app,&mut heading);
    let pitch=app.piano_tool.is_pitch();
    let freehand=matches!(app.piano_tool,super::Tool::NoteDraw|super::Tool::PitchDraw);
    for (i,icon,label,selected,tool) in [
        (0,Icon::MultiEdit,crate::i18n::t("Multi-editing tool"),!freehand,if pitch {super::Tool::PitchAnchor}else{super::Tool::NoteSelect}),
        (1,Icon::Freehand,crate::i18n::t("Freehand tool"),freehand,if pitch {super::Tool::PitchDraw}else{super::Tool::NoteDraw}),
    ] {
        let rect=Rect::from_min_size(row.min+vec2(126.0+i as f32*24.0,4.0),vec2(24.0,16.0));
        if ui.put(rect,IconButton::new(icon,label).size(rect.size()).grouped().tool().group_edges(i==0,i==1).selected(selected)).clicked(){choose_classic_tool(app,tool);}
    }
    let snap=Rect::from_min_size(row.min+vec2(178.0,4.0),vec2(18.0,16.0));
    if ui.put(snap,IconButton::new(Icon::Magnet,crate::i18n::t("对齐网格")).size(snap.size()).framed().tool().selected(app.view.snap)).clicked(){app.view.snap=!app.view.snap;}
    for (i,icon,label,selected,tool) in [
        (0,Icon::ModeNote,crate::i18n::t("Note Editing mode"),!pitch,if freehand {super::Tool::NoteDraw}else{super::Tool::NoteSelect}),
        (1,Icon::ModePitch,crate::i18n::t("Direct Pitch Editing mode"),pitch,if freehand {super::Tool::PitchDraw}else{super::Tool::PitchAnchor}),
    ] {
        let rect=Rect::from_min_size(row.min+vec2(200.0+i as f32*24.0,4.0),vec2(24.0,16.0));
        if ui.put(rect,IconButton::new(icon,label).size(rect.size()).grouped().tool().group_edges(i==0,i==1).selected(selected)).clicked(){choose_classic_tool(app,tool);}
    }
    for (i,icon,label,active) in [
        (0,Icon::InputPitch,crate::i18n::t("Show Input Pitch"),&mut app.show_pitch),
        (1,Icon::OutputPitch,crate::i18n::t("Show Output Pitch"),&mut app.show_output_pitch),
        (2,Icon::Waveform,crate::i18n::t("显示渲染波形"),&mut app.show_waveform),
        (3,Icon::Phonemes,crate::i18n::t("Show Phoneme Timing"),&mut app.view.phonemes),
        (4,Icon::Retake,crate::i18n::t("显示 AI 重录小部件"),&mut app.show_retakes),
    ] {
        let rect=Rect::from_min_size(row.min+vec2(260.0+i as f32*24.0,4.0),vec2(24.0,16.0));
        if ui.put(rect,IconButton::new(icon,label).size(rect.size()).framed().tool().group_edges(i==0,i==4).selected(*active)).clicked(){*active=!*active;}
    }
    let mut grid=crate::ui::child(ui,Rect::from_min_size(row.min+vec2(392.0,4.0),vec2(119.0,16.0)),"classic-piano-grid");
    grid.spacing_mut().interact_size.y=16.0;
    grid.spacing_mut().button_padding=vec2(4.0,0.0);
    crate::ui::theme::native::combo("classic-piano-grid").width(119.0)
        .selected_text(crate::lformat!("{} 分{}",app.view.grid,if app.view.triplet {crate::i18n::t("三连音")}else{crate::i18n::t("音符")}))
        .show_ui(&mut grid,|ui| {
            for triplet in [false,true] {
                for value in [4,8,16,32,64,128] {
                    if triplet && (value==4||value==128){continue;}
                    let text=if triplet {crate::lformat!("{value} 分三连音")}else{crate::lformat!("{value} 分音符")};
                    if ui.selectable_label(app.view.grid==value && app.view.triplet==triplet,text).clicked(){app.view.grid=value;app.view.triplet=triplet;}
                }
            }
        });
    let mut extensions=crate::ui::child(ui,Rect::from_min_size(row.min+vec2(523.0,0.0),vec2(row.width()-523.0,26.0)),"classic-piano-extensions");
    extensions.horizontal_centered(|ui| {
        super::scale_guide::toggle(app,ui);
        crate::ui::scale::controls::toolbar(app,ui);
    });
    ui.expand_to_include_rect(extensions.min_rect());
}

fn choose_classic_tool(app: &mut App, tool: super::Tool) {
    app.commit_lyric();
    app.piano_tool=tool;
    if tool==super::Tool::PitchAnchor {app.editor.selected.clear();}
    if tool.is_pitch(){app.show_pitch=true;}
    app.status.clear();
}
