use super::widgets::*;
use crate::{app::App, ui::state::Column};
use eframe::egui::*;

pub(super) fn general(app: &mut App, ui: &mut Ui) {
    row(ui, crate::i18n::t("界面语言"), |ui| {
        ComboBox::from_id_salt("interface-language").width(ui.available_width())
            .selected_text(app.preferences.language.name()).show_ui(ui,|ui| {
                for language in crate::i18n::Language::ALL {
                    if ui.selectable_value(&mut app.preferences.language,language,language.name()).changed() {
                        crate::i18n::set_language(language);
                    }
                }
            });
    });
    row(ui, crate::i18n::t("界面缩放"), |ui| {
        let id = ui.make_persistent_id("zoom-draft");
        let mut percent = ui
            .ctx()
            .data_mut(|data| data.get_temp::<f32>(id))
            .unwrap_or(app.preferences.zoom * 100.0);
        ui.label(format!("{percent:.0}%"));
        ui.spacing_mut().slider_width = ui.available_width().max(40.0);
        let response = ui.add(
            Slider::new(&mut percent, 75.0..=200.0)
                .step_by(5.0)
                .show_value(false),
        );
        if response.drag_stopped() || response.changed() && !response.dragged() {
            app.preferences.zoom = percent / 100.0;
        }
        if response.dragged() {
            ui.ctx().data_mut(|data| data.insert_temp(id, percent));
        } else {
            ui.ctx().data_mut(|data| data.remove::<f32>(id));
        }
    });
    fixed(
        ui,
        crate::i18n::t("图形渲染"),
        "Direct3D 12",
        crate::i18n::t("当前使用 GPU 渲染，暂不支持切换渲染后端"),
    );
    section(ui, crate::i18n::t("帮助和指导"));
    toggle(ui, crate::i18n::t("显示状态栏"), &mut app.preferences.show_status_bar);
    toggle(ui, crate::i18n::t("显示控件操作提示"), &mut app.preferences.control_hints);
    section(ui, crate::i18n::t("工作区"));
    row(ui, crate::i18n::t("工具列宽度"), |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 55.0).max(40.0);
        ui.add(Slider::new(&mut app.view.panel_width, 140.0..=420.0).suffix(" px"));
    });
    row(ui, crate::i18n::t("布局"), |ui| {
        if ui.button(crate::i18n::t("重置工作区布局")).clicked() {
            app.view = Default::default();
        }
    });
}

pub(super) fn audio(app: &mut App, ui: &mut Ui) {
    section(ui, crate::i18n::t("音频输出"));
    fixed(
        ui,
        crate::i18n::t("输出设备"),
        crate::i18n::t("系统默认输出设备"),
        crate::i18n::t("当前跟随系统默认设备，设备切换尚未接入"),
    );
    fixed(
        ui,
        crate::i18n::t("播放格式"),
        crate::i18n::t("设备默认格式"),
        crate::i18n::t("预览音频自动适配输出设备的采样率和声道"),
    );
    if let Some(error) = app.synthesis.playback_error() {
        ui.colored_label(Color32::from_rgb(218, 142, 126), crate::i18n::message(&error));
    }
    section(ui, crate::i18n::t("MIDI 输入"));
    fixed(ui, crate::i18n::t("输入设备"), crate::i18n::t("未启用"), crate::i18n::t("MIDI 输入尚未接入"));
}

pub(super) fn editing(app: &mut App, ui: &mut Ui) {
    section(ui, crate::i18n::t("钢琴卷帘"));
    row(ui, crate::i18n::t("半音行高"), |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 55.0).max(40.0);
        ui.add(Slider::new(&mut app.view.row_height, 12.0..=36.0).suffix(" px"));
    });
    toggle(ui, crate::i18n::t("音符对齐网格"), &mut app.view.snap);
    row(ui, crate::i18n::t("网格精度"), |ui| {
        ComboBox::from_id_salt("settings-grid")
            .width(ui.available_width())
            .selected_text(crate::lformat!("{} 分音符", app.view.grid))
            .show_ui(ui, |ui| {
                for value in [4, 8, 16, 32, 64, 128] {
                    ui.selectable_value(&mut app.view.grid, value, crate::lformat!("{value} 分音符"));
                }
            });
    });
    toggle(ui, crate::i18n::t("三连音网格"), &mut app.view.triplet);
    section(ui, crate::i18n::t("显示"));
    toggle(ui, crate::i18n::t("显示音高线"), &mut app.show_pitch);
    toggle(ui, crate::i18n::t("显示波形"), &mut app.show_waveform);
    toggle(ui, crate::i18n::t("编排位于上方"), &mut app.view.arrangement_first);
    toggle(ui, crate::i18n::t("显示曲速和拍号轨"), &mut app.show_time_tracks);
    ui.add_space(10.0);
    ui.weak(crate::i18n::t("拖动标题可调整工具列顺序，拖动分隔线可调整区域尺寸。"));
    section(ui, crate::i18n::t("装饰"));
    toggle(ui, crate::i18n::t("显示角色图像"), &mut app.preferences.display_background_image);
    row(ui, crate::i18n::t("图像透明度"), |ui| {
        ui.spacing_mut().slider_width = (ui.available_width()-50.0).max(40.0);
        ui.label(format!("{:.0}%", app.preferences.background_transparency*100.0));
        ui.add(Slider::new(&mut app.preferences.background_transparency,0.0..=1.0)
            .step_by(0.1).show_value(false));
    });
}

pub(super) fn recording(ui: &mut Ui) {
    fixed(ui, crate::i18n::t("录音输入设备"), crate::i18n::t("未启用"), crate::i18n::t("录音功能尚未接入"));
    fixed(ui, crate::i18n::t("录音声道"), crate::i18n::t("未启用"), crate::i18n::t("录音功能尚未接入"));
    section(ui, crate::i18n::t("录制"));
    ui.weak(crate::i18n::t("录制功能尚未接入，相关设置暂不可用。"));
}

pub(super) fn rendering(app: &mut App, ui: &mut Ui) {
    crate::ui::voice_library::settings(app, ui);
    row(ui, crate::i18n::t("歌声数据库"), |ui| {
        if ui.button(crate::i18n::t("管理歌声…")).clicked() {
            app.settings = false;
            app.voice_manager = true;
        }
    });
    section(ui, crate::i18n::t("音频导出"));
    row(ui, crate::i18n::t("音频转换器（FFmpeg）"), |ui| {
        if let Some(path) = &app.synthesis.library.settings.audio_converter {
            ui.add(Label::new(path.display().to_string()).wrap());
        } else {
            ui.weak(crate::i18n::t("自动查找"));
        }
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::t("选择文件…")).clicked()
                && let Some(path) = rfd::FileDialog::new().set_title("FFmpeg").pick_file() {
                app.synthesis.set_audio_converter(Some(path));
            }
            if app.synthesis.library.settings.audio_converter.is_some()
                && ui.button(crate::i18n::t("自动查找")).clicked() {
                app.synthesis.set_audio_converter(None);
            }
        });
    });
    ui.weak(crate::i18n::t("其他格式先生成 WAV，再由 FFmpeg 转换。"));
    row(ui, crate::i18n::t("导出设置"), |ui| {
        if ui.button(crate::i18n::t("打开渲染面板")).clicked() {
            if !app.view.columns.contains(&Column::Render) {
                app.view.toggle(Column::Render);
            }
            app.settings = false;
        }
    });
    ui.weak(crate::i18n::t("目标文件夹、音轨、声道、位深度和采样率在渲染面板中设置。"));
}
