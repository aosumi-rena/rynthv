use eframe::egui::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub language: crate::i18n::Language,
    pub zoom: f32,
    pub show_status_bar: bool,
    pub control_hints: bool,
    pub display_background_image: bool,
    pub background_transparency: f32,
    pub metronome: bool,
    pub return_on_pause: bool,
    pub looping: bool,
    pub sync_note_parameters: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: Default::default(),
            zoom: 1.0,
            show_status_bar: true,
            control_hints: false,
            display_background_image: true,
            background_transparency: 0.5,
            metronome: false,
            return_on_pause: true,
            looping: false,
            sync_note_parameters: false,
        }
    }
}

impl Preferences {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(crate::lformat!("无法读取界面设置：{error}")),
        };
        let value: Self =
            serde_json::from_slice(&bytes).map_err(|e| crate::lformat!("无法读取界面设置：{e}"))?;
        if !value.zoom.is_finite() || !(0.75..=2.0).contains(&value.zoom) {
            return Err(crate::i18n::t("界面缩放必须为 75% 至 200%").into());
        }
        if !value.background_transparency.is_finite() || !(0.0..=1.0).contains(&value.background_transparency) {
            return Err(crate::i18n::t("图像透明度必须为 0% 至 100%").into());
        }
        Ok(value)
    }

    pub fn apply(&self, ctx: &Context) {
        crate::i18n::set_language(self.language);
        ctx.set_zoom_factor(self.zoom);
        ctx.style_mut_of(eframe::egui::Theme::Dark, |style| {
            style.explanation_tooltips = self.control_hints
        });
    }
}
