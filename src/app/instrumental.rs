use super::App;
use crate::model::{AudioClip, EntityId};
use eframe::egui::Context;
use std::path::PathBuf;

pub(super) const EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "aif", "aiff"];

pub struct AudioImport {
    pub track: EntityId,
    path: PathBuf,
    onset: f64,
}

impl App {
    pub fn import_audio(&mut self, target: Option<EntityId>, ctx: &Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(crate::i18n::t("导入伴奏"))
            .add_filter(crate::i18n::t("音频文件"), EXTENSIONS)
            .pick_file()
        else {
            return;
        };
        self.import_audio_path(target, path, ctx);
    }

    pub(super) fn import_audio_path(&mut self, target: Option<EntityId>, path: PathBuf, ctx: &Context) {
        self.commit_lyric();
        self.editor.finish();
        let track = target.unwrap_or_else(|| self.editor.add_instrumental_track());
        self.begin_audio_import(track, path, ctx);
    }

    pub fn reload_audio(&mut self, track: EntityId, ctx: &Context) {
        if let Some(path) = self
            .editor
            .project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.audio_clip())
            .map(|c| c.path.clone())
        {
            self.begin_audio_import(track, path, ctx);
        }
    }

    fn begin_audio_import(&mut self, track: EntityId, path: PathBuf, ctx: &Context) {
        let onset = self
            .editor
            .project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.audio_clip())
            .map_or(0.0, |c| c.onset_seconds);
        self.synthesis.instrumentals.begin_import(path.clone(), ctx);
        self.audio_import = Some(AudioImport { track, path, onset });
        self.status = crate::i18n::t("正在导入伴奏…").into();
    }

    pub(super) fn poll_audio_import(&mut self) {
        let Some(import) = &self.audio_import else {
            return;
        };
        if !self
            .editor
            .project
            .tracks
            .iter()
            .any(|t| t.id == import.track && t.is_instrumental())
        {
            self.audio_import = None;
            self.synthesis.instrumentals.finish_import();
            self.status.clear();
            return;
        }
        if let Some(error) = self.synthesis.instrumentals.error(&import.path) {
            self.error = Some(error.to_owned());
            self.audio_import = None;
            self.synthesis.instrumentals.finish_import();
            self.status = crate::i18n::t("伴奏导入失败").into();
            return;
        }
        let Some(asset) = self.synthesis.instrumentals.asset(&import.path) else {
            return;
        };

        if self.editor.is_editing() {
            return;
        }
        let duration = asset.audio.duration_seconds();
        let import = self.audio_import.take().unwrap();
        self.editor.set_audio_clip(
            import.track,
            Some(AudioClip {
                path: import.path,
                onset_seconds: import.onset,
                duration_seconds: duration,
                trim_begin_seconds: None,
                trim_end_seconds: None,
            }),
        );
        self.synthesis.instrumentals.finish_import();
        self.status = crate::i18n::t("伴奏已导入，可拖动片段调整位置").into();
    }
}
