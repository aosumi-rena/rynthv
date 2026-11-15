mod cards;
mod catalog;
mod details;
mod feedback;
mod header;
mod toolbar;

use super::{error_label, paths, text};
use crate::{app::App, ui::theme::*};
use eframe::egui::*;
use std::{collections::BTreeMap, path::PathBuf};

use crate::ui::resource_manager::{ACCENT, CARD, INK, Sort};
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
    super::style_presets::dialog(app, ctx);
}

fn show(app: &mut App, ctx: &Context, state: &mut State) {
    let close = crate::ui::resource_manager::dialog(
        ctx,
        "voice-manager",
        &mut (&mut *app, &mut *state),
        |(app, _), ui, head| header::show(app, ui, head),
        |(app, state), ui| {
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
        },
    );
    if close {
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
                ui.menu_button(crate::i18n::t("选择声库目录…"), |ui| {
                    paths::directory_menu(app, ui)
                });
            }
        });
}
