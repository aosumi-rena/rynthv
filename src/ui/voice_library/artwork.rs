use std::path::Path;

use crate::ui::{icons::Icon, theme::*};
use crate::synthesis::voice::VoiceArtwork;
use eframe::egui::*;

pub(super) fn paint(ui: &Ui, rect: Rect, artwork: Option<&VoiceArtwork>) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.painter()
        .rect_filled(rect, 4, Color32::from_rgb(53, 57, 55));
    if let Some(artwork) = artwork {
        image(artwork)
            .fit_to_exact_size(rect.size())
            .corner_radius(4)
            .paint_at(ui, rect);
    } else {
        Icon::Microphone.paint(ui, rect.shrink(rect.width() * 0.26), MUTED);
    }
}

pub(crate) fn image(artwork:&VoiceArtwork) -> Image<'static> {
    match artwork {
        VoiceArtwork::File {path} => Image::from_uri(file_uri(path)),
        VoiceArtwork::Embedded {uri,bytes} => Image::from_bytes(uri.clone(),load::Bytes::Shared(bytes.clone())),
    }
}

fn file_uri(path: &Path) -> String {
    #[cfg(windows)]
    {
        let raw=path.to_string_lossy();
        let normalized=if let Some(value)=raw.strip_prefix(r"\\?\UNC\") {format!(r"\\{value}")}
            else {raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_owned()};
        let path = normalized.replace('\\', "/");
        if path.starts_with("//") {
            format!("file:{path}")
        } else {
            format!("file:///{path}")
        }
    }
    #[cfg(not(windows))]
    {

        format!("file://{}", path.to_string_lossy())
    }
}
