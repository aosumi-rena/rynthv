use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use std::path::Path;
use std::cell::Cell;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

mod native;
mod metrics;

thread_local! {
    static NATIVE_SCALE: Cell<f32> = const { Cell::new(1.0) };
    static HEADING_SCALE: Cell<f32> = const { Cell::new(1.0) };
}

pub(super) fn native_size(height: f32, heading: bool) -> f32 {
    height * if heading { HEADING_SCALE.with(Cell::get) } else { NATIVE_SCALE.with(Cell::get) }
}

pub fn install(ctx: &Context) -> Result<(), String> {
    let mut fonts = FontDefinitions::default();
    let result = load(&mut fonts);

    if let Some(faces) = fonts.families.get_mut(&FontFamily::Proportional)
        && let Some(index) = faces.iter().position(|name| name == "NativeNoto")
    {
        let name = faces.remove(index);
        faces.insert(0, name);
    }

    let native_ui = fonts.families[&FontFamily::Proportional].clone();
    let scale = install_height_family(&mut fonts, "NativeUI", &native_ui);
    NATIVE_SCALE.with(|value| value.set(scale));

    let mut settings = fonts.families[&FontFamily::Proportional].clone();
    if fonts.font_data.contains_key("NativeNoto") {
        settings.retain(|name|name != "NativeNoto");
        settings.insert(0,"NativeNoto".into());
    }
    fonts.families.insert(FontFamily::Name("NativeSettings".into()),settings);

    let mut heading = fonts.families[&FontFamily::Proportional].clone();
    if fonts.font_data.contains_key("ChineseBold") {
        heading.insert(0, "ChineseBold".into());
    }
    fonts
        .families
        .insert(FontFamily::Name("Transport".into()), heading.clone());
    fonts
        .families
        .insert(FontFamily::Name("SectionTitle".into()), heading.clone());
    let scale = install_height_family(&mut fonts, "NativeHeading", &heading);
    HEADING_SCALE.with(|value| value.set(scale));
    ctx.set_fonts(fonts);
    result
}

fn install_height_family(fonts: &mut FontDefinitions, family: &str, faces: &[String]) -> f32 {
    let scale = faces.first()
        .and_then(|name| fonts.font_data.get(name))
        .and_then(|font| metrics::height_scale(font))
        .unwrap_or(1.0);
    fonts.families.insert(FontFamily::Name(family.into()), faces.to_vec());
    scale
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
