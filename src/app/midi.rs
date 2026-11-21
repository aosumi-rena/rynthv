use super::App;
use crate::{editor::Editor, midi::MidiFile, model::beats, ui};
use eframe::egui::Context;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MidiImportMode {
    NewProject,
    Tracks,
}

pub enum MidiImportPhase {
    Loading(Receiver<Result<MidiFile, String>>),
    Ready { file: MidiFile, selected: Vec<bool> },
    Failed(String),
}

pub struct MidiImport {
    pub path: PathBuf,
    pub mode: MidiImportMode,
    pub phase: MidiImportPhase,
    pub validation_error: Option<String>,
    revision: u64,
}

impl App {
    pub fn begin_midi_import(&mut self, mode: MidiImportMode, ctx: &Context) {
        ui::piano::cancel_audition(self);
        ui::tempo_input::finish(self);
        self.commit_lyric();
        self.editor.finish();
        let Some(path) = rfd::FileDialog::new()
            .set_title(crate::i18n::t("导入工程"))
            .add_filter(crate::i18n::t("歌声工程"), rynthv::project_import::EXTENSIONS)
            .pick_file()
        else {
            return;
        };
        self.import_project_path(path, mode, ctx);
    }

    pub(super) fn import_project_path(&mut self, path: PathBuf, mode: MidiImportMode, ctx: &Context) {
        if mode == MidiImportMode::NewProject {
            self.open_path(path);
            return;
        }
        if crate::storage::ProjectFormat::from_path(&path) == Some(crate::storage::ProjectFormat::Rvp) {
            self.request_file(super::FileAction::OpenPath(path), ctx);
            return;
        }
        ui::piano::cancel_audition(self);
        ui::tempo_input::finish(self);
        self.commit_lyric();
        self.editor.finish();
        if rynthv::project_import::is_midi(&path) {
            self.begin_midi_path(path, mode, ctx);
        } else if rynthv::project_import::supports(&path) {
            self.begin_project_import(path, mode, ctx);
        } else {
            self.error = Some("PROJECT_UNSUPPORTED_FORMAT".into());
        }
    }

    pub(super) fn begin_midi_path(&mut self, path: PathBuf, mode: MidiImportMode, ctx: &Context) {
        let (tx, rx) = mpsc::channel();
        let source = path.clone();
        let repaint = ctx.clone();
        let worker = std::thread::Builder::new()
            .name("midi-import".into())
            .spawn(move || {
                let _ = tx.send(crate::midi::load(&source));
                repaint.request_repaint();
            });
        let phase = match worker {
            Ok(_) => MidiImportPhase::Loading(rx),
            Err(error) if mode == MidiImportMode::NewProject => {
                self.error = Some(crate::lformat!("无法启动 MIDI 读取线程：{error}"));
                return;
            }
            Err(error) => MidiImportPhase::Failed(crate::lformat!("无法启动 MIDI 读取线程：{error}")),
        };
        if mode == MidiImportMode::NewProject {
            self.status = crate::i18n::t("正在导入工程…").into();
        }
        self.midi_import = Some(MidiImport {
            path,
            mode,
            phase,
            validation_error: None,
            revision: self.editor.revision,
        });
    }

    pub(super) fn poll_midi_import(&mut self) {
        let Some(import) = &mut self.midi_import else {
            return;
        };
        let MidiImportPhase::Loading(receiver) = &import.phase else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(crate::i18n::t("MIDI 读取线程意外停止。").into()),
        };
        import.phase = match result {
            Ok(file) => {
                let selected = file
                    .tracks
                    .iter()
                    .map(|track| !track.notes.is_empty())
                    .collect();
                MidiImportPhase::Ready { file, selected }
            }
            Err(error) => MidiImportPhase::Failed(error),
        };
        if import.mode == MidiImportMode::NewProject {
            let import = self.midi_import.take().unwrap();
            if self.editor.revision != import.revision {
                self.status.clear();
                self.error = Some(crate::i18n::t("导入期间当前工程已更改，已保留当前工程。请保存后重新打开要导入的文件。").into());
            } else if let Err(error) = match &import.phase {
                MidiImportPhase::Failed(error) => Err(error.clone()),
                _ => self.apply_midi_import(&import),
            } {
                self.status.clear();
                self.error = Some(error);
            }
        }
    }

    pub fn apply_midi_import(&mut self, import: &MidiImport) -> Result<(), String> {
        let MidiImportPhase::Ready { file, selected } = &import.phase else {
            return Err(crate::i18n::t("MIDI 尚未读取完成。").into());
        };
        let indices: Vec<_> = selected
            .iter()
            .enumerate()
            .filter_map(|(index, &selected)| selected.then_some(index))
            .collect();
        self.commit_lyric();
        self.editor.finish();
        let summary: crate::editor::ImportSummary = match import.mode {
            MidiImportMode::NewProject => {
                self.scripts.stop();

                let (editor, summary) = Editor::from_midi(file, &indices)?;
                self.synthesis.reset_project();
                self.rocoflex.clear();
                self.editor = editor;
                self.restore_transport_choices();
                self.path = Some(import.path.clone());
                self.audio_import = None;
                self.gesture = None;
                self.piano_anchor = crate::ui::piano::AnchorSelection::default();
                self.group_gesture = None;
                self.audio_drag = None;
                self.view_pan = ui::view_pan::ViewPan::default();
                self.rename = None;
                self.time_edit = None;
                self.bpm_edit = None;
                self.fill_lyrics = None;
                summary
            }
            MidiImportMode::Tracks => self.editor.import_midi_tracks(file, &indices)?,
        };
        self.piano_collapsed = false;
        self.view.arrangement_collapsed = false;
        if let Some(first) = self.editor.notes().first() {
            let offset = self
                .editor
                .active_ref()
                .map_or(0, |reference| reference.blick_offset);
            let start = beats(first.onset + offset).floor().max(0.0);
            self.view.piano.start = start;
            self.view.arrangement.start = start;
            self.view.top_pitch = (first.pitch + 9).clamp(12, 127) as f32;
        }
        self.status = crate::lformat!(
            "已导入 {} 个音符到 {} 条音轨",
            summary.notes, summary.tracks
        );
        if summary.split_tracks > 0 {
            self.status
                .push_str(&crate::lformat!(" · 重叠音符拆出 {} 条声部", summary.split_tracks));
        }
        Ok(())
    }
}
