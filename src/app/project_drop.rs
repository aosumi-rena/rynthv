use super::{App, FileAction, MidiImportMode};
use eframe::egui::*;

impl App {
    pub(super) fn dropped_project_dialog(&mut self, ctx: &Context) {
        let Some(path) = self.dropped_project.clone() else {
            return;
        };
        let choice = crate::ui::prompt::choices(
            ctx,
            "dropped-project",
            crate::i18n::t("打开或导入工程"),
            &path.file_name().unwrap_or_default().to_string_lossy(),
            [
                crate::i18n::t("取消"),
                crate::i18n::t("导入当前工程"),
                crate::i18n::t("在新工程中打开"),
            ],
            false,
        );
        if let Some(choice) = choice {
            self.dropped_project = None;
            if choice == 2 {
                self.request_file(FileAction::OpenPath(path), ctx);
            } else if choice == 1 {
                self.commit_lyric();
                self.editor.finish();
                self.import_project_path(path, MidiImportMode::Tracks, ctx);
            }
        }
    }
}
