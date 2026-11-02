mod instrumental;
mod midi;
mod project_import;
mod transport_preferences;
mod voice;
pub use midi::{MidiImport, MidiImportMode, MidiImportPhase};

fn blank_project()->Project {
    let mut project=Project::default();
    project.tracks[0].name=format!("{} 1",crate::i18n::t("未命名音轨"));
    project.render_config.insert("filename".into(),crate::i18n::t("未命名").into());
    project
}

use crate::{
    editor::Editor,
    model::*,
    storage, synthesis,
    ui::{self, state::ViewState},
};
use eframe::egui::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum FileAction {
    New,
    Open,
    ImportMidi,
    Close,
    OpenPath(PathBuf),
}
pub struct App {
    pub editor: Editor,
    pub synthesis: synthesis::runtime::Runtime,
    pub view: ViewState,
    pub preferences: ui::settings::Preferences,
    pub preset_library: ui::voice_library::PresetLibrary,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
    pub settings: bool,
    pub voice_manager: bool,
    pub about: bool,
    pub piano_tool: ui::piano::Tool,
    pub piano_anchor: ui::piano::AnchorSelection,
    pub parameter_lane: ui::lanes::ParameterSelection,
    pub phoneme_focus: Option<(EntityId, Option<EntityId>)>,
    pub show_pitch: bool,
    pub show_waveform: bool,
    pub show_retakes: bool,
    pub show_time_tracks: bool,
    pub piano_collapsed: bool,
    pub piano_audition: ui::piano::KeyboardAudition,
    pub view_pan: ui::view_pan::ViewPan,
    pub gesture: Option<ui::piano::Gesture>,
    pub group_gesture: Option<ui::group_edit::GroupGesture>,
    pub audio_drag: Option<ui::instrumental::AudioDrag>,
    pub audio_import: Option<instrumental::AudioImport>,
    pub midi_import: Option<MidiImport>,
    project_import: Option<project_import::Import>,
    pub rename: Option<ui::rename::Rename>,
    pub time_edit: Option<ui::time_edit::TimeEdit>,
    pub bpm_edit: Option<ui::tempo_input::BpmEdit>,
    pub lyric: Option<ui::piano::LyricEdit>,
    pub fill_lyrics: Option<ui::fill_lyrics::FillLyrics>,
    pub pending: Option<FileAction>,
    pub allow_close: bool,
    pub status: String,
    context: Context,
    settings_path: Option<PathBuf>,
    last_layout: String,
    last_preferences: String,
    last_persist: Instant,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        ui::theme::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut error = ui::theme::fonts(&cc.egui_ctx).err();
        let settings_path = match rynthv::platform::config_directory() {
            Ok(directory) => Some(directory.join("workspace.json")),
            Err(message) => {
                error = Some(message);
                None
            }
        };
        let mut view = ViewState::default();
        if let Some(path) = settings_path.as_ref().filter(|path| path.exists()) {
            match std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<ViewState>(&b).map_err(|e| e.to_string()))
            {
                Ok(mut v) if v.valid() => {
                    for l in &mut v.lanes {
                        l.id = new_id();
                    }
                    view = v;
                }
                Ok(_) => error = Some(crate::i18n::t("保存的工作区尺寸或结构无效，已重置布局").into()),
                Err(e) => error = Some(crate::lformat!("无法读取工作区：{e}")),
            }
        }
        let preferences = match settings_path
            .as_ref()
            .map(|path| ui::settings::Preferences::load(&path.with_file_name("preferences.json")))
            .transpose()
        {
            Ok(preferences) => preferences.unwrap_or_default(),
            Err(message) => {
                error = Some(message);
                ui::settings::Preferences::default()
            }
        };
        preferences.apply(&cc.egui_ctx);
        cc.egui_ctx
            .options_mut(|options| options.zoom_with_keyboard = false);
        let last_preferences =
            serde_json::to_string_pretty(&preferences).expect("valid interface preferences");
        let mut app = Self {
            editor: Editor::new(blank_project()),
            synthesis: synthesis::runtime::Runtime::new(cc.egui_ctx.clone()),
            view,
            preferences,
            preset_library: Default::default(),
            path: None,
            error,
            settings: false,
            voice_manager: false,
            about: false,
            piano_tool: ui::piano::Tool::NoteSelect,
            piano_anchor: ui::piano::AnchorSelection::default(),
            parameter_lane: Default::default(),
            phoneme_focus: None,
            show_pitch: true,
            show_waveform: true,
            show_retakes: true,
            show_time_tracks: true,
            piano_collapsed: false,
            piano_audition: ui::piano::KeyboardAudition::default(),
            view_pan: ui::view_pan::ViewPan::default(),
            gesture: None,
            group_gesture: None,
            audio_drag: None,
            audio_import: None,
            midi_import: None,
            project_import: None,
            rename: None,
            time_edit: None,
            bpm_edit: None,
            lyric: None,
            fill_lyrics: None,
            pending: None,
            allow_close: false,
            status: String::new(),
            context: cc.egui_ctx.clone(),
            settings_path,
            last_layout: String::new(),
            last_preferences,
            last_persist: Instant::now(),
        };
        app.restore_transport_choices();
        app.synthesis.set_metronome(app.preferences.metronome);
        if let Some(path) = path {
            app.open_path(path);
        }
        app
    }
    pub fn open_path(&mut self, path: PathBuf) {
        self.project_import=None;
        if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case("svp")) {
            self.begin_project_import(path,MidiImportMode::NewProject,&self.context.clone());
            return;
        }
        ui::piano::cancel_audition(self);
        self.midi_import = None;
        match storage::load(&path) {
            Ok(project) => {
                self.editor = Editor::new(project);
                self.restore_transport_choices();
                self.synthesis.reset_project();
                self.path = Some(path);
                self.gesture = None;
                self.piano_anchor = ui::piano::AnchorSelection::default();
                self.parameter_lane = Default::default();
                self.phoneme_focus = None;
                self.group_gesture = None;
                self.audio_drag = None;
                self.audio_import = None;
                self.view_pan = ui::view_pan::ViewPan::default();
                self.rename = None;
                self.time_edit = None;
                self.bpm_edit = None;
                self.lyric = None;
                self.fill_lyrics = None;
                let reference = self.editor.active_ref();
                let first = reference.and_then(|r| {
                    self.editor.notes().iter().find_map(|n| {
                        r.note_range(n)
                            .map(|(start, _)| (start, n.pitch + r.pitch_offset))
                    })
                });
                self.view.piano.start = beats(first.map_or_else(
                    || reference.map_or(0, |r| r.blick_absolute_begin),
                    |(start, _)| start,
                ))
                .floor()
                .max(0.0);
                if let Some((_, pitch)) = first {
                    self.view.top_pitch = (pitch + 9).clamp(12, 127) as f32;
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    pub fn save_project(&mut self, save_as: bool) -> bool {
        ui::piano::cancel_audition(self);
        ui::tempo_input::finish(self);
        self.commit_lyric();
        self.editor.finish();
        let path = if save_as || self.path.is_none() {
            let name = self
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or(crate::i18n::t("未命名.svp"));
            rfd::FileDialog::new()
                .add_filter(crate::i18n::t("Synthesizer V 工程"), &["svp"])
                .set_file_name(name)
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(mut path) = path else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension("svp");
        }
        match storage::save(&path, &self.project_for_save()) {
            Ok(()) => {
                self.editor.mark_saved();
                self.path = Some(path);
                self.status = crate::i18n::t("工程已保存").into();
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
    pub fn request_file(&mut self, action: FileAction, ctx: &Context) {
        ui::piano::cancel_audition(self);
        ui::tempo_input::finish(self);
        self.time_edit = None;
        self.commit_lyric();
        self.fill_lyrics = None;
        self.editor.finish();
        if self.editor.dirty() {
            self.pending = Some(action);
        } else {
            self.perform_file(action, ctx);
        }
    }
    fn perform_file(&mut self, action: FileAction, ctx: &Context) {
        self.project_import=None;
        self.midi_import = None;
        self.gesture = None;
        self.piano_anchor = ui::piano::AnchorSelection::default();
                self.parameter_lane = Default::default();
                self.phoneme_focus = None;
        self.group_gesture = None;
        self.audio_drag = None;
        self.audio_import = None;
        self.synthesis.instrumentals.finish_import();
        self.view_pan.cancel(ctx);
        self.rename = None;
        self.time_edit = None;
        self.lyric = None;
        self.fill_lyrics = None;
        self.bpm_edit = None;
        match action {
            FileAction::New => {
                self.editor = Editor::new(blank_project());
                self.restore_transport_choices();
                self.synthesis.reset_project();
                self.path = None;
            }
            FileAction::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(crate::i18n::t("歌声工程"), rynthv::project_import::EXTENSIONS)
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            FileAction::ImportMidi => self.begin_midi_import(MidiImportMode::NewProject, ctx),
            FileAction::OpenPath(path)=>self.open_path(path),
            FileAction::Close => {
                self.allow_close = true;
                self.persist_layout();
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }
    pub fn edit_lyric(&mut self) {
        if let Some(note) = self
            .editor
            .notes()
            .iter()
            .find(|n| self.editor.selected.contains(&n.id))
        {
            self.lyric = Some(ui::piano::LyricEdit {
                id: note.id,
                text: note.lyrics.clone(),
                original: note.lyrics.clone(),
                focus: true,
                phonemes: false,
            });
        }
    }
    pub fn commit_lyric(&mut self) {
        if let Some(lyric) = self.lyric.take() {
            if lyric.text==lyric.original {return;}
            if lyric.phonemes {self.editor.set_phonemes(lyric.id,lyric.text);}
            else {self.editor.set_lyric(lyric.id, lyric.text);}
        }
    }
    fn persist_layout(&mut self) {
        self.last_persist = Instant::now();
        let Some(settings_path) = self.settings_path.as_ref() else {
            return;
        };
        if let Ok(json) = serde_json::to_string_pretty(&self.preferences)
            && json != self.last_preferences
        {
            let path = settings_path.with_file_name("preferences.json");
            let result = std::fs::create_dir_all(path.parent().unwrap())
                .map_err(|e| e.to_string())
                .and_then(|_| storage::atomic_write(&path, json.as_bytes()));
            match result {
                Ok(()) => self.last_preferences = json,
                Err(error) => self.error = Some(error),
            }
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.view)
            && json != self.last_layout
        {
            let result = std::fs::create_dir_all(settings_path.parent().unwrap())
                .map_err(|e| e.to_string())
                .and_then(|_| storage::atomic_write(settings_path, json.as_bytes()));
            match result {
                Ok(()) => self.last_layout = json,
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn dialogs(&mut self, ctx: &Context) {
        if let Some(action) = self.pending.clone() {
            let title=match &action {FileAction::New=>crate::i18n::t("新建..."),FileAction::Open|FileAction::OpenPath(_)=>crate::i18n::t("打开..."),FileAction::ImportMidi=>crate::i18n::t("导入..."),FileAction::Close=>crate::i18n::t("退出...")};
            match ui::prompt::unsaved(ctx,title) {
                Some(ui::prompt::Choice::Save) if self.save_project(false)=>{
                    self.pending=None;self.perform_file(action,ctx);
                }
                Some(ui::prompt::Choice::Discard)=>{self.pending=None;self.perform_file(action,ctx);}
                Some(ui::prompt::Choice::Cancel)=>{self.pending=None;}
                _=>{}
            }
        }
        if let Some(error) = self.error.clone() {
            if ui::prompt::error(ctx,&error){self.error=None;}
        }
        if self.about {
            Modal::new(Id::new("about")).show(ctx, |ui| {
                ui.heading(format!("RynthV {}", env!("CARGO_PKG_VERSION")));
                ui.label(crate::i18n::t("Rust + egui 歌声合成引擎"));
                ui.label(crate::i18n::t("多音轨编辑、歌词与音高调整、MIDI 导入和 WAV 导出。"));
                ui.label(crate::i18n::t("支持 Windows 和 Linux。"));
                if ui.button(crate::i18n::t("关闭")).clicked() {
                    self.about = false;
                }
            });
        }
        if self.settings {
            ui::settings::show(self, ctx);
        }
        ui::voice_library::dialogs(self, ctx);
    }
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui::piano::begin_keyboard_frame(self, &ctx);
        self.synthesis.poll();
        self.poll_audio_import();
        self.poll_midi_import();
        self.poll_project_import(&ctx);
        let dropped=ctx.input(|i|i.raw.dropped_files.iter().map(|f|f.path().to_path_buf()).collect::<Vec<_>>());
        if let Some(path)=dropped.first() {
            if dropped.len()!=1 {self.error=Some(crate::i18n::t("请一次拖入一个工程文件。").into());}
            else if path.extension().and_then(|e|e.to_str()).is_some_and(|e|rynthv::project_import::EXTENSIONS.iter().any(|ext|e.eq_ignore_ascii_case(ext))) {
                self.request_file(FileAction::OpenPath(path.clone()),&ctx);
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.request_file(FileAction::Close, &ctx);
        }
        let audio_seconds = self.synthesis.playback_clock(self.editor.transport.playing);
        let looped = self.editor.transport.update(
            &self.editor.project.time,
            self.editor.project.end().max(32 * QUARTER),
            audio_seconds,
        );
        let transport_position = self.editor.transport.position;
        if self.editor.transport.playing {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.pending.is_none()
            && self.error.is_none()
            && !self.settings
            && !self.voice_manager
            && self.synthesis.context_import.dialog.is_none()
            && !self.about
            && self.rename.is_none()
            && self.time_edit.is_none()
            && self.bpm_edit.is_none()
            && self.fill_lyrics.is_none()
            && !self.preset_library.is_naming()
            && self.midi_import.is_none()
            && !Popup::is_any_open(&ctx)
        {
            ui::chrome::shortcuts(self, &ctx);
        }
        let title = format!(
            "RynthV — {}{}",
            self.path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|p| p.to_str())
                .unwrap_or(crate::i18n::t("未命名")),
            if self.editor.dirty() { " *" } else { "" }
        );
        ctx.send_viewport_cmd(ViewportCommand::Title(title));
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0, ui::theme::BACKGROUND);
        let menu = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 27.0));
        ui::chrome::menus(self, &mut ui::child(ui, menu, "menu"));
        let transport = Rect::from_min_max(
            pos2(rect.left(), menu.bottom()),
            pos2(rect.right(), menu.bottom() + 44.0),
        );
        ui::transport::show(self, &mut ui::child(ui, transport, "transport"));
        let status_height = if self.preferences.show_status_bar {
            23.0
        } else {
            0.0
        };
        let status = Rect::from_min_max(pos2(rect.left(), rect.bottom() - status_height), rect.max);
        let work = Rect::from_min_max(
            pos2(rect.left() + 5.0, transport.bottom()),
            pos2(rect.right() - 5.0, status.top() - 4.0),
        );
        let seeked=transport_position!=self.editor.transport.position;
        ui::workspace::show(self, &mut ui::child(ui, work, "workspace"),seeked);
        ui::instrumental::update_drag(self, &ctx);
        if self.group_gesture.is_some()
            && ctx.input(|i| i.pointer.button_released(PointerButton::Primary))
        {
            ui::group_edit::finish(self);
        }
        if self.preferences.show_status_bar {
            let mut status_ui = ui::child(ui, status.shrink2(vec2(8.0, 1.0)), "status");
            status_ui.horizontal(|ui| {
                let hint = if self.status.is_empty() && self.piano_tool == ui::piano::Tool::PitchAnchor {
                    crate::i18n::t("双击创建控制点 · 拖动框选 / 移动 · 四边手柄缩放 · Delete 删除 · Esc 取消 · Ctrl+Z 撤销")
                } else if self.status.is_empty() && self.piano_tool == ui::piano::Tool::PitchDraw {
                    crate::i18n::t("左键拖动绘制音高 · 右键拖动擦除 · Esc 取消本次绘制 · Ctrl+Z 撤销 · 中键拖动视野")
                } else if self.status.is_empty() {
                    crate::i18n::t("双击创建音符 · 拖动框选 · 中键拖动视野 · Shift+滚轮水平移动 · Ctrl+滚轮缩放")
                } else {
                    &self.status
                };
                ui.label(RichText::new(crate::i18n::message(hint)).size(11.0).color(ui::theme::MUTED));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(crate::i18n::message(self.synthesis.status()))
                            .size(11.0)
                            .color(ui::theme::MUTED),
                    );
                });
            });
        }
        self.dialogs(&ctx);
        ui::rename::show(self, &ctx);
        if self.pending.is_none() && self.error.is_none() {
            ui::time_edit::show(self, &ctx);
            ui::fill_lyrics::show(self, &ctx);
            ui::midi_import::show(self, &ctx);
        }
        self.view_pan.end_frame(&ctx);
        ui::piano::end_keyboard_frame(self, &ctx);
        ui::piano::end_pitch_frame(self, &ctx);
        ui::piano::end_anchor_frame(self, &ctx);
        ui::lanes::end_frame(self,&ctx);
        ui::phonemes::end_frame(self,&ctx);
        ui::panels::end_performance_frame(self, &ctx);
        if !self.editor.is_editing() {
            self.synthesis
                .sync_project(&self.editor.project, self.editor.revision);
        }
        if self.editor.transport.playing {
            self.editor.transport.constrain_to_loop();
        }
        self.synthesis.set_loop_range(
            self.editor
                .transport
                .loop_seconds(&self.editor.project.time),
        );
        self.synthesis.sync_transport(
            self.editor.transport.playing,
            self.editor
                .project
                .time
                .seconds_at(self.editor.transport.position)
                .max(0.0),
            looped || self.editor.transport.position != transport_position,
        );
        if self.last_persist.elapsed() > Duration::from_secs(3) {
            self.persist_layout();
        }
    }
    fn on_exit(&mut self) {
        ui::piano::cancel_audition(self);
        self.persist_layout();
    }
}
