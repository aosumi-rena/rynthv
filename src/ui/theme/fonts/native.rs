use eframe::egui::{FontDefinitions, FontFamily};
use std::path::PathBuf;

pub(super) fn load(fonts: &mut FontDefinitions) -> Result<(), String> {
    if let Some(path) = editor_font() {
        insert(fonts, "NativeNoto", &path)?;
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("WINDIR") {

        let path = PathBuf::from(root).join("Fonts/tahoma.ttf");
        if path.is_file() { insert(fonts, "NativeWindowsSans", &path)?; }
    }
    Ok(())
}

fn insert(fonts: &mut FontDefinitions, name: &str, path: &std::path::Path) -> Result<(), String> {
    fonts.font_data.insert(name.into(), super::read_font(path, 0)?.into());
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let faces = fonts.families.entry(family).or_default();

        let index = faces.iter().position(|face| face == "NativeNoto")
            .or_else(||faces.iter().position(|face| face == "Chinese"))
            .map_or(faces.len(), |i| i + 1);
        faces.insert(index, name.into());
    }
    Ok(())
}

fn editor_font() -> Option<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(settings) = crate::synthesis::settings::SynthesisSettings::load() {
        for version in crate::synthesis::settings::PhonemeVersion::ALL {
            if let Some(root)=settings.phoneme_data.get(version).and_then(std::path::Path::parent) {
                paths.push(root.join("fonts/NotoSansCJKjp-Regular.otf"));
            }
        }
    }
    #[cfg(windows)]
    for variable in ["ProgramW6432", "ProgramFiles"] {
        if let Some(root) = std::env::var_os(variable) {
            paths.push(PathBuf::from(root).join("Synthesizer V Studio 2 Pro/fonts/NotoSansCJKjp-Regular.otf"));
        }
    }
    paths.into_iter().find(|path| path.is_file())
}
