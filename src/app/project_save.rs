use super::App;
use crate::{i18n::t, storage::{self, ProjectFormat}, ui};
use std::path::{Path, PathBuf};

impl App {
    pub fn can_save_project_as(&self) -> bool {
        ProjectFormat::ALL.into_iter().any(|format| {
            format != ProjectFormat::Rvp && format.can_save(&self.editor.project)
        })
    }

    pub fn save_project(&mut self, save_as: bool) -> bool {
        if save_as && !self.can_save_project_as() {
            return false;
        }
        ui::piano::cancel_audition(self);
        ui::tempo_input::finish(self);
        self.commit_lyric();
        self.editor.finish();

        let project = self.project_for_save();
        let current = self.path.as_deref().and_then(ProjectFormat::from_path)
            .filter(|format| format.can_save(&project));
        let target = if !save_as && current.is_some() {
            self.path.clone()
        } else {
            let rvp_only = !self.can_save_project_as() || (!save_as && self.path.is_some());
            let primary = current.unwrap_or(ProjectFormat::Rvp);
            let formats: Vec<_> = if rvp_only {
                vec![ProjectFormat::Rvp]
            } else {
                std::iter::once(primary).chain(ProjectFormat::ALL.into_iter().filter(|&format| {
                    format != primary && format.can_save(&project)
                })).collect()
            };
            let Some(path) = self.pick_project_save_path(&formats) else { return false };
            if !ProjectFormat::from_path(&path).is_some_and(|format| formats.contains(&format)) {
                self.error = Some(t(if rvp_only { "RVP_SAVE_REQUIRED" } else { "PROJECT_FORMAT_INCOMPATIBLE" }).into());
                return false;
            }
            Some(path)
        };
        let Some(path) = target else { return false };

        match storage::save(&path, &project) {
            Ok(()) => {
                self.editor.mark_saved();
                self.path = Some(path);
                self.status = t("工程已保存").into();
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn pick_project_save_path(&self, formats: &[ProjectFormat]) -> Option<PathBuf> {
        let name = self.path.as_deref().and_then(Path::file_stem).and_then(|name| name.to_str())
            .unwrap_or(t("未命名"));
        let mut dialog = rfd::FileDialog::new().set_title(t("保存工程"));
        for format in formats {
            dialog = dialog.add_filter(format.label(), &[format.extension()]);
        }
        if let Some(parent) = self.path.as_deref().and_then(Path::parent) {
            dialog = dialog.set_directory(parent);
        }
        let mut path = dialog.set_file_name(name).save_file()?;
        if path.extension().is_none_or(|extension| extension.is_empty()) {
            path.set_extension(formats[0].extension());
        }
        Some(path)
    }
}
