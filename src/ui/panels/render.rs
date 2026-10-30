use super::controls::*;
use crate::{
    app::App,
    model::Extra,
    synthesis::{runtime::{ExportRequest, export_directory}, audio::AudioFileFormat},
    ui::{theme::*, track_color::TrackColors},
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui) {
    let mut config = app.editor.project.render_config.clone();
    let mut tracks: Vec<_> = app
        .editor
        .project
        .tracks
        .iter()
        .map(|t| {
            (
                t.id,
                t.name.clone(),
                t.extra
                    .get("renderEnabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                TrackColors::new(t).accent,
            )
        })
        .collect();
    let original_tracks = tracks.clone();
    let mut export_clicked = false;
    padded(ui, |ui| {
        ui.weak(crate::i18n::t("目标文件夹"));
        let mut directory = string(&config, "destination");
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 34.0).max(20.0);
            ui.add_sized(
                vec2(width, 28.0),
                TextEdit::singleline(&mut directory)
                    .vertical_align(Align::Center)
                    .margin(vec2(7.0, 5.0))
                    .background_color(BORDER),
            );
            if folder_button(ui).clicked() {
                let mut picker = rfd::FileDialog::new().set_title(crate::i18n::t("选择导出文件夹"));
                if let Ok(path) = export_directory(&directory, app.path.as_deref()) {
                    picker = picker.set_directory(path);
                }
                if let Some(path) = picker.pick_folder() {
                    directory = path.display().to_string();
                }
            }
        });
        config.insert("destination".into(), directory.into());
        ui.weak(crate::i18n::t("相对路径以工程文件夹为基准"));
        ui.weak(crate::i18n::t("文件名称"));
        let mut name = string(&config, "filename");
        field(ui, &mut name);
        config.insert("filename".into(), name.into());
        ui.add_space(12.0);
        let ready = !app.editor.is_editing() && app.synthesis.export_ready(app.editor.revision);
        export_clicked = ui
            .add_enabled(
                ready,
                Button::new(crate::i18n::t("导出为文件")).min_size(vec2(ui.available_width(), 28.0)),
            )
            .on_disabled_hover_text(crate::i18n::t("等待伴奏加载和当前歌声合成完成后导出"))
            .clicked();
        if app.synthesis.is_exporting() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(crate::i18n::t("正在导出…"));
            });
        }
        if let Some(result) = &app.synthesis.export_result {
            match result {
                Ok(paths) => {
                    ui.label(crate::lformat!("已导出 {} 个文件", paths.len()))
                        .on_hover_text(
                            paths
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                }
                Err(message) => error(ui, message),
            }
        }
    });
    section(ui, crate::i18n::t("音轨"), |ui| {
        let mut mix = config
            .get("exportMixDown")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        Frame::new()
            .fill(BACKGROUND)
            .inner_margin(4)
            .show(ui, |ui| {
                ui.set_min_height(190.0);
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 3.0;
                track_row(ui, &mut mix, crate::i18n::t("混音导出"), TEXT);
                for (_, name, enabled, color) in &mut tracks {
                    track_row(ui, enabled, name, *color);
                }
            });
        config.insert("exportMixDown".into(), mix.into());
        pair(ui, |ui, width| {
            if ui
                .add_sized(vec2(width, 28.0), Button::new(crate::i18n::t("全选")))
                .clicked()
            {
                config.insert("exportMixDown".into(), true.into());
                for (_, _, enabled, _) in &mut tracks {
                    *enabled = true;
                }
            }
            if ui
                .add_sized(vec2(width, 28.0), Button::new(crate::i18n::t("取消全选")))
                .clicked()
            {
                config.insert("exportMixDown".into(), false.into());
                for (_, _, enabled, _) in &mut tracks {
                    *enabled = false;
                }
            }
        });
    });
    section(ui, crate::i18n::t("格式"), |ui| {
        ui.weak(crate::i18n::t("文件格式"));
        let mut file_format = config.get("audioFormat").and_then(serde_json::Value::as_str).unwrap_or("wav").to_owned();
        let formats: Vec<_> = AudioFileFormat::ALL.iter().map(|format| (format.id().to_owned(), format.label())).collect();
        combo(ui, "audio-file-format", &mut file_format, &formats);
        let flac = file_format == "flac";
        let opus = file_format == "opus";
        let mp3 = file_format == "mp3";
        config.insert("audioFormat".into(), file_format.into());
        if flac && config.get("bitDepth").and_then(serde_json::Value::as_u64) == Some(32) {
            config.insert("bitDepth".into(), 24.into());
        }
        if (opus && config.get("sampleRate").and_then(serde_json::Value::as_u64) != Some(48000))
            || (mp3 && config.get("sampleRate").and_then(serde_json::Value::as_u64) == Some(96000)) {
            config.insert("sampleRate".into(), 48000.into());
        }
        numeric(
            ui,
            &mut config,
            "numChannels",
            crate::i18n::t("声道"),
            &[(1, crate::i18n::t("单声道")), (2, crate::i18n::t("立体声"))],
        );
        ui.weak(crate::i18n::t("气声输出"));
        let mut aspiration = string(&config, "aspirationFormat");
        combo(
            ui,
            "aspiration-format",
            &mut aspiration,
            &[("noAspiration".to_owned(), crate::i18n::t("无"))],
        );
        config.insert("aspirationFormat".into(), aspiration.into());
        let depths = [(16, crate::i18n::t("16 位")), (24, crate::i18n::t("24 位")), (32, crate::i18n::t("32 位浮点"))];
        numeric(
            ui,
            &mut config,
            "bitDepth",
            crate::i18n::t("位深度"),
            if flac { &depths[..2] } else { &depths },
        );
        numeric(
            ui,
            &mut config,
            "sampleRate",
            crate::i18n::t("采样率"),
            if opus { &[(48000, "48000 Hz")] }
            else if mp3 { &[(44100, "44100 Hz"), (48000, "48000 Hz")] }
            else { &[
                (44100, "44100 Hz"),
                (48000, "48000 Hz"),
                (96000, "96000 Hz"),
            ] },
        );
    });
    if config != app.editor.project.render_config || tracks != original_tracks {
        app.editor.begin(crate::i18n::t("渲染设置"));
        app.editor.project.render_config = config;
        for (id, _, enabled, _) in tracks {
            if let Some(track) = app.editor.project.tracks.iter_mut().find(|t| t.id == id) {
                track.extra.insert("renderEnabled".into(), enabled.into());
            }
        }
        app.editor.finish();
    }
    if export_clicked {
        start_export(app);
    }
    section(ui, crate::i18n::t("合成状态"), |ui| {
        super::super::synthesis::render_status(app, ui)
    });
}

fn start_export(app: &mut App) {
    let request = match ExportRequest::from_project(&app.editor.project, app.path.as_deref(), app.synthesis.library.settings.audio_converter.as_deref()) {
        Ok(request) => request,
        Err(message) => {
            app.synthesis.export_result = Some(Err(message));
            return;
        }
    };
    let existing: Vec<_> = request
        .outputs
        .iter()
        .filter(|(path, _)| path.exists())
        .map(|(path, _)| path.display().to_string())
        .collect();
    if !existing.is_empty()
        && rfd::MessageDialog::new()
            .set_title(crate::i18n::t("覆盖音频文件"))
            .set_description(crate::lformat!(
                "以下文件已存在，是否覆盖？\n{}",
                existing.join("\n")
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            != rfd::MessageDialogResult::Yes
    {
        return;
    }
    app.synthesis.export_project(request, app.editor.revision);
}

fn string(config: &Extra, key: &str) -> String {
    config
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned()
}
fn numeric(ui: &mut Ui, config: &mut Extra, key: &str, label: &str, values: &[(u64, &str)]) {
    ui.weak(label);
    let mut value = config
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(values[0].0);
    combo(ui, key, &mut value, values);
    config.insert(key.into(), value.into());
}
fn track_row(ui: &mut Ui, checked: &mut bool, name: &str, color: Color32) {
    let row = Frame::new()
        .fill(PANEL)
        .corner_radius(2)
        .inner_margin(Margin {
            left: 12,
            right: 6,
            top: 5,
            bottom: 5,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.checkbox(checked, name);
        });
    ui.painter().rect_filled(
        Rect::from_min_size(row.response.rect.min, vec2(3.0, row.response.rect.height())),
        0,
        color,
    );
}
fn folder_button(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    ui.painter().rect(
        rect,
        3,
        if response.hovered() {
            BUTTON_HOVER
        } else {
            BORDER
        },
        Stroke::new(0.6, BUTTON),
        StrokeKind::Inside,
    );
    let c = rect.center();
    let points = [
        (-5.5, -3.0),
        (-5.5, -5.0),
        (-1.0, -5.0),
        (1.0, -3.0),
        (5.5, -3.0),
        (5.5, 5.0),
        (-5.5, 5.0),
        (-5.5, -3.0),
    ];
    ui.painter().add(Shape::line(
        points.into_iter().map(|(x, y)| c + vec2(x, y)).collect(),
        Stroke::new(1.1, MUTED),
    ));
    response.on_hover_text(crate::i18n::t("选择目标文件夹"))
}
