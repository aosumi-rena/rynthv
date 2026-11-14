mod cards;
mod catalog;
mod details;
mod feedback;
mod header;
mod toolbar;

use super::{context, error_label, paths, text};
use crate::{
    app::App,
    ui::{child, theme::*},
};
use eframe::egui::*;
use std::{collections::BTreeMap, path::PathBuf};

const SURFACE: Color32 = Color32::from_rgb(27, 27, 27);
const CARD: Color32 = Color32::from_rgb(38, 38, 38);
const ACCENT: Color32 = Color32::from_rgb(120, 214, 24);
const INK: Color32 = Color32::from_rgb(242, 242, 242);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sort {
    #[default]
    Default,
    Name,
    NameReverse,
    Version,
}
impl Sort {
    fn label(self) -> &'static str {
        match self {
            Self::Default => crate::i18n::t("默认"),
            Self::Name => crate::i18n::t("名称 A–Z"),
            Self::NameReverse => crate::i18n::t("名称 Z–A"),
            Self::Version => crate::i18n::t("版本最新"),
        }
    }
}
#[derive(Clone, Default)]
struct State {
    query: String,
    sort: Sort,
    language: String,
    vendor: String,
    versions: BTreeMap<String, crate::synthesis::voice::VoiceRevision>,
    expanded: Option<String>,
    root: Option<PathBuf>,
    sv1_root: Option<PathBuf>,
}

pub fn dialogs(app: &mut App, ctx: &Context) {
    let id = Id::new("voice-manager-state");
    if app.voice_manager {
        let mut state = ctx
            .data_mut(|data| data.get_temp::<State>(id))
            .unwrap_or_default();
        if state.root != app.synthesis.library.settings.databases
            || state.sv1_root != app.synthesis.library.settings.sv1_databases
        {
            state = State {
                root: app.synthesis.library.settings.databases.clone(),
                sv1_root: app.synthesis.library.settings.sv1_databases.clone(),
                ..Default::default()
            };
        }
        show(app, ctx, &mut state);
        ctx.data_mut(|data| data.insert_temp(id, state));
    } else {
        ctx.data_mut(|data| data.remove::<State>(id));
    }
    context::dialog(app, ctx);
    super::style_presets::dialog(app, ctx);
}

fn show(app: &mut App, ctx: &Context, state: &mut State) {
    let size = (ctx.content_rect().size() - vec2(32.0, 40.0))
        .min(vec2(1120.0, 650.0))
        .max(vec2(260.0, 240.0));
    let mut close = false;
    let modal = Modal::new(Id::new("voice-manager"))
        .backdrop_color(Color32::from_black_alpha(105))
        .frame(
            Frame::new()
                .fill(SURFACE)
                .corner_radius(5)
                .stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 50))),
        )
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), header::HEIGHT));
            close = header::show(app, ui, head);
            ui.painter().hline(
                head.x_range(),
                head.bottom(),
                Stroke::new(1.0, Color32::from_rgb(20, 20, 20)),
            );
            let margin = (rect.width() * 0.08).clamp(16.0, 90.0);
            let body = Rect::from_min_max(
                pos2(rect.left() + margin, head.bottom() + 26.0),
                rect.max - vec2(margin, 16.0),
            );
            let mut content = child(ui, body, "voice-store-content");
            ScrollArea::vertical()
                .id_salt("voice-store-scroll")
                .auto_shrink([false, false])
                .show(&mut content, |ui| {
                    ui.set_width(ui.available_width());
                    toolbar::show(app, ui, state);
                    ui.add_space(28.0);
                    let entries = catalog::entries(app, state);
                    if app.synthesis.library.is_scanning() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            text(ui, crate::i18n::t("正在扫描歌声…"));
                        });
                        return;
                    }
                    if let Some(error) = app.synthesis.library.settings_error() {
                        error_label(ui, crate::i18n::t("无法读取声库列表"));
                        ui.collapsing(crate::i18n::t("查看原因"), |ui| error_label(ui, error));
                        ui.menu_button(crate::i18n::t("选择声库目录…"), |ui| {
                            paths::directory_menu(app, ui)
                        });
                        return;
                    }
                    if let Some(error) = app.synthesis.library.catalog_error() {
                        error_label(
                            ui,
                            if app
                                .synthesis
                                .library
                                .catalog()
                                .is_some_and(|catalog| !catalog.entries.is_empty())
                            {
                                crate::i18n::t("部分声库目录无法读取")
                            } else {
                                crate::i18n::t("无法读取声库目录")
                            },
                        );
                        ui.collapsing(crate::i18n::t("查看原因"), |ui| error_label(ui, error));
                        ui.menu_button(crate::i18n::t("选择声库目录…"), |ui| {
                            paths::directory_menu(app, ui)
                        });
                        ui.add_space(12.0);
                    }
                    if entries.is_empty() {
                        empty(app, ui, state);
                    } else {
                        ui.spacing_mut().item_spacing.y = 16.0;
                        for entry in &entries {
                            cards::show(app, ui, entry, state);
                        }
                    }
                });
        });
    if close || modal.should_close() {
        app.voice_manager = false;
    }
}

fn empty(app: &mut App, ui: &mut Ui, state: &mut State) {
    Frame::new()
        .fill(CARD)
        .corner_radius(13)
        .inner_margin(28)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(100.0);
            if !state.query.is_empty() || !state.language.is_empty() || !state.vendor.is_empty() {
                text(
                    ui,
                    RichText::new(crate::i18n::t("没有匹配的歌声"))
                        .font(section_title_font())
                        .size(16.0),
                );
                if ui.button(crate::i18n::t("清除搜索与筛选")).clicked() {
                    state.query.clear();
                    state.language.clear();
                    state.vendor.clear();
                }
            } else {
                text(
                    ui,
                    RichText::new(crate::i18n::t("没有可显示的声库"))
                        .font(section_title_font())
                        .size(16.0),
                );
                ui.menu_button(crate::i18n::t("选择声库目录…"), |ui| paths::directory_menu(app, ui));
            }
        });
}
