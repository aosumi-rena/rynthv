use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

mod native;

pub fn install(ctx: &Context) -> Result<(), String> {
    let mut fonts = FontDefinitions::default();
    let result = load(&mut fonts);

    let mut heading = fonts.families[&FontFamily::Proportional].clone();
    if fonts.font_data.contains_key("ChineseBold") {
        heading.insert(0, "ChineseBold".into());
    }
    fonts
        .families
        .insert(FontFamily::Name("Transport".into()), heading.clone());
    fonts
        .families
        .insert(FontFamily::Name("SectionTitle".into()), heading);
    ctx.set_fonts(fonts);
    result
}

fn load(fonts: &mut FontDefinitions) -> Result<(), String> {
    let regular = system_font(false)?;
    fonts.font_data.insert("Chinese".into(), regular.into());
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Chinese".into());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("Chinese".into());
    let bold = system_font(true)?;
    fonts.font_data.insert("ChineseBold".into(), bold.into());
    native::load(fonts)?;
    #[cfg(target_os = "windows")]
    if let Some(root)=std::env::var_os("WINDIR") {
        for (name,file) in [("JapaneseFallback","meiryo.ttc"),("KoreanFallback","malgun.ttf")] {
            let path=std::path::PathBuf::from(&root).join("Fonts").join(file);
            if path.is_file() && let Ok(data)=read_font(&path,0) {
                fonts.font_data.insert(name.into(),data.into());
                for family in [FontFamily::Proportional,FontFamily::Monospace] {
                    fonts.families.entry(family).or_default().push(name.into());
                }
            }
        }
    }
    Ok(())
}

fn read_font(path: &Path, index: u32) -> Result<FontData, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Cannot load UI font {}: {error}", path.display()))?;
    Ok(FontData {
        index,
        ..FontData::from_owned(bytes)
    })
}

#[cfg(target_os = "windows")]
fn system_font(bold: bool) -> Result<FontData, String> {
    let root = std::env::var_os("WINDIR").ok_or(crate::i18n::t("缺少 WINDIR，无法加载中文字体"))?;
    let path = std::path::PathBuf::from(root).join("Fonts").join(if bold {
        "msyhbd.ttc"
    } else {
        "msyh.ttc"
    });
    read_font(&path, 0)
}

#[cfg(target_os = "linux")]
fn system_font(bold: bool) -> Result<FontData, String> {
    linux::load(bold)
}

#[cfg(target_os = "macos")]
fn system_font(bold: bool) -> Result<FontData, String> {
    macos::load(bold)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn system_font(_bold: bool) -> Result<FontData, String> {
    Err("UI system font loading is supported on Windows, Linux and macOS only".into())
}
