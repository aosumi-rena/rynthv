use crate::ui::resource_picker::{menu_row,separator};
use std::collections::BTreeMap;

use super::{artwork, error_label, family_filter::{self, FamilyFilter, LABELS}, family_name, selected_entry, weak};
use crate::{
    app::App,
    synthesis::{
        settings::VoiceSelection,
        voice::{VoiceAvailability, VoiceCatalog, VoiceCatalogEntry},
    },
    ui::{icons::Icon, theme::*},
};
use eframe::egui::*;

pub(super) fn selected_card(app: &mut App, ui: &mut Ui) {
    let entry = selected_entry(app);
    let track=app.editor.current_track();
    let missing=app.synthesis.voice_not_installed(track,app.editor.active_ref().unwrap_or(&track.main_ref));
    let name = if missing { crate::i18n::t("选择歌声") }
        else { app.voice_name().unwrap_or(crate::i18n::t("选择歌声")) };
    let response = selection_card(ui, entry, name);
    popup(app, &response);
}

fn selection_card(ui: &mut Ui, entry: Option<&VoiceCatalogEntry>, name: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::click());
    ui.painter().rect(
        rect,
        4,
        if response.hovered() { BUTTON } else { BORDER },
        Stroke::new(0.7, BUTTON),
        StrokeKind::Inside,
    );
    let art = Rect::from_min_size(rect.min, Vec2::splat(60.0));
    artwork::paint(ui, art, entry.and_then(|entry| entry.icon.as_ref()));
    let label_rect = Rect::from_min_max(
        pos2(art.right() + 8.0, rect.top()),
        rect.max - vec2(26.0, 0.0),
    );
    ui.painter()
        .with_clip_rect(label_rect.intersect(ui.clip_rect()))
        .text(
            label_rect.left_center() - vec2(0.0, if entry.is_some() { 9.0 } else { 0.0 }),
            Align2::LEFT_CENTER,
            entry.map_or(name, |entry| entry.localized_display_name()),
            FontId::proportional(13.0),
            TEXT,
        );
    if let Some(entry) = entry {
        ui.painter()
            .with_clip_rect(label_rect.intersect(ui.clip_rect()))
            .text(
                label_rect.left_center() + vec2(0.0, 12.0),
                Align2::LEFT_CENTER,
                source_hint(entry),
                FontId::proportional(10.5),
                MUTED,
            );
    }
    Icon::ChevronDown.paint(
        ui,
        Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), Vec2::splat(12.0)),
        MUTED,
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, crate::i18n::t("选择歌声")));
    let response = if let Some(entry) = entry {
        response.on_hover_text(entry.directory.display().to_string())
    } else { response };
    response.on_hover_cursor(CursorIcon::Default)
}

enum Choice { Voice(VoiceSelection), Manage, Clear }

pub fn popup(app: &mut App, anchor: &Response) {
    let selected = app.voice_selection();
    match voice_picker(anchor, app.synthesis.library.catalog(), selected.as_ref(), true,
        app.has_voice(), app.synthesis.library.is_scanning(), app.synthesis.library.catalog_error().is_some(), false)
    {
        Some(Choice::Voice(voice)) => app.select_voice(voice),
        Some(Choice::Manage) => app.voice_manager = true,
        Some(Choice::Clear) => app.unload_voice(),
        None => {}
    }
}

pub(crate) fn donor_picker(ui: &mut Ui, catalog: &VoiceCatalog, donor: &mut Option<VoiceSelection>) {
    let entry = donor.as_ref().and_then(|donor| catalog.entries.iter().find(|entry| entry.selection() == *donor));
    let anchor = selection_card(ui, entry, crate::i18n::t("选择歌声"));
    if let Some(Choice::Voice(voice)) = voice_picker(&anchor, Some(catalog), donor.as_ref(), false, false, false, false, false) {
        *donor = Some(voice);
    }
}

pub(crate) fn vocoder_picker(ui: &mut Ui, catalog: Option<&VoiceCatalog>, donor: &mut Option<VoiceSelection>, scanning: bool) {
    let entry = donor.as_ref().and_then(|key| catalog?.entries.iter().find(|entry| entry.selection() == *key));
    let anchor = selection_card(ui, entry, crate::i18n::t("选择歌声"));
    if let Some(Choice::Voice(voice)) = voice_picker(&anchor, catalog, donor.as_ref(), false, false, scanning, false, true) {
        *donor = Some(voice);
    }
}

fn voice_picker(anchor: &Response, catalog: Option<&VoiceCatalog>, selected: Option<&VoiceSelection>,
    track_actions: bool, can_clear: bool, scanning: bool, catalog_error: bool, r2_only: bool) -> Option<Choice>
{
    let picker_height = crate::ui::resource_picker::height(&anchor.ctx);
    let mut choice = None;
    let popup = Popup::menu(anchor);
    let search_id = popup.get_id().with("voice-name-search");
    let focus_id = search_id.with("pending-focus");
    let filter_id = popup.get_id().with("voice-family-filter");

    let opening=!popup.is_open();
    if opening {
        anchor.ctx.data_mut(|data| {
            data.remove::<String>(search_id);
            data.insert_temp(focus_id, true);
        });
    }
    let mut query = anchor.ctx.data(|data| data.get_temp::<String>(search_id)).unwrap_or_default();
    let mut filter = if r2_only { FamilyFilter::r2_ai() } else { anchor.ctx.data(|data| data.get_temp::<FamilyFilter>(filter_id)).unwrap_or_default() };
    popup
        .width(288.0)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(crate::ui::resource_picker::frame(&anchor.ctx))
        .show(|ui| {
            ui.set_width(280.0);
            ui.set_height(picker_height);
            ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
            crate::ui::resource_picker::header(ui,crate::i18n::t("歌声"));
            let input_id=search_id.with("input");
            let focus = ui.ctx().data(|data| data.get_temp::<bool>(focus_id)).unwrap_or(false);
            if focus {ui.ctx().memory_mut(|memory|memory.request_focus(input_id));}
            let search = ui.add_sized(vec2(ui.available_width(),28.0),
                TextEdit::singleline(&mut query).id(input_id)
                    .hint_text(crate::i18n::t("搜索歌声名称"))
                    .vertical_align(Align::Center));

            if focus {
                search.request_focus();
                ui.ctx().request_repaint();
                if !ui.is_sizing_pass() && ui.is_visible() && ui.is_enabled() {
                    ui.ctx().data_mut(|data|data.remove::<bool>(focus_id));
                }
            }
            ui.ctx().data_mut(|data| {
                data.insert_temp(search_id,query.clone());
            });
            let filter_changed = if r2_only { FamilyFilter::show_r2_ai(ui); false } else { filter.show(ui) };
            ui.ctx().data_mut(|data| data.insert_temp(filter_id, filter));
            separator(ui);
            if track_actions {
                if menu_row(ui, crate::i18n::t("管理歌声"), Some(Icon::Cloud), true).clicked() {
                    choice = Some(Choice::Manage);
                    ui.close();
                }
                separator(ui);
                if menu_row(ui, crate::i18n::t("取消当前音轨歌声"), None, can_clear).clicked() {
                    choice = Some(Choice::Clear);
                    ui.close();
                }
                separator(ui);
            }
            if scanning {
                ui.horizontal(|ui| {
                    ui.spinner();
                    weak(ui, crate::i18n::t("正在扫描歌声…"));
                });
            }
            let needle = query.trim().to_lowercase();
            let representatives = representative_entries(catalog, selected, !track_actions);
            let entries: Vec<_> = representatives.iter().copied()
                .filter(|entry| filter.includes(entry) && entry.localized_display_name().to_lowercase().contains(&needle)).collect();
            if catalog_error {
                error_label(
                    ui,
                    if entries.is_empty() {
                        crate::i18n::t("无法读取声库目录，请在“管理歌声”中查看原因。")
                    } else {
                        crate::i18n::t("部分声库目录无法读取，已列出可读取的歌声。")
                    },
                );
            }
            let mut list=ScrollArea::vertical()
                .id_salt("voice-picker-list")
                .max_height(ui.available_height())
                .auto_shrink([false, false]);
            if opening || search.changed() || filter_changed {list=list.vertical_scroll_offset(0.0);}
            list.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if entries.is_empty() && !scanning {
                        weak(ui, if track_actions && needle.is_empty() && representatives.is_empty() {crate::i18n::t("没有可选歌声，请在“管理歌声”中设置 databases 目录。")}
                            else {crate::i18n::t("没有匹配的歌声")});
                    }
                    for category in 0..7 {
                        let banks:Vec<_>=entries.iter().filter(|entry|category_of(entry)==category).collect();
                        if banks.is_empty() {continue;}
                        {
                            separator(ui);
                            let title=LABELS.get(category as usize).copied()
                                .unwrap_or_else(||crate::i18n::t("其他歌声 · 暂不可用"));
                            menu_row(ui,title,None,false);
                        }
                        for entry in banks {
                            let selection = VoiceSelection {
                                database_id: entry.database_id.clone(),
                                revision: entry.revision,
                            };
                            if card(ui, entry, selected == Some(&selection)).clicked() {
                                choice = Some(Choice::Voice(selection));
                                ui.close();
                            }
                        }
                    }
                });
        });
    choice
}

pub(super) fn category_of(entry:&VoiceCatalogEntry)->u8 {
    family_filter::category_of(entry)
}

fn representative_entries<'a>(catalog: Option<&'a VoiceCatalog>, selected: Option<&VoiceSelection>,
    loadable_only: bool) -> Vec<&'a VoiceCatalogEntry>
{
    let mut banks: BTreeMap<&str, &VoiceCatalogEntry> = BTreeMap::new();
    let rank = |entry: &VoiceCatalogEntry| {
        (
            selected.is_some_and(|s| {
                s.database_id == entry.database_id && s.revision == entry.revision
            }),
            entry.availability.is_loadable(),
            entry.revision,
        )
    };
    if let Some(catalog) = catalog {
        for entry in &catalog.entries {
            if loadable_only && !entry.availability.is_loadable() { continue; }
            let current = banks.entry(&entry.database_id).or_insert(entry);
            if rank(entry) > rank(current) {
                *current = entry;
            }
        }
    }
    let mut entries: Vec<_> = banks.into_values().collect();
    entries.sort_by(|a, b| {
        a.localized_display_name().to_lowercase()
            .cmp(&b.localized_display_name().to_lowercase())
            .then_with(||a.localized_display_name().cmp(b.localized_display_name()))
            .then_with(|| a.database_id.cmp(&b.database_id))
    });
    entries
}

fn card(ui: &mut Ui, entry: &VoiceCatalogEntry, selected: bool) -> Response {
    let enabled = entry.availability.is_loadable();
    let (response, art, content) = crate::ui::resource_picker::card(ui,selected,enabled);
    artwork::paint(ui, art, entry.icon.as_ref());
    let painter = ui
        .painter()
        .with_clip_rect(content.intersect(ui.clip_rect()));
    painter.text(
        content.min,
        Align2::LEFT_TOP,
        entry.localized_display_name(),
        section_title_font(),
        if enabled { TEXT } else { MUTED },
    );
    painter.text(
        content.min + vec2(0.0, 23.0),
        Align2::LEFT_TOP,
        match &entry.availability {
            VoiceAvailability::Loadable => source_hint(entry),
            VoiceAvailability::Unsupported(reason) => reason.clone(),
        },
        FontId::proportional(12.0),
        MUTED,
    );
    let mut x = content.left();
    let mut tags = Vec::new();
    if let Some(range) = &entry.pitch_range {
        tags.push(range.as_str());
    }
    tags.extend(entry.languages.iter().map(|s| language_tag(s)));
    for tag in tags {
        let galley = painter.layout_no_wrap(tag.to_owned(), FontId::proportional(10.5), TEXT);
        let badge = Rect::from_min_size(
            pos2(x, content.top() + 44.0),
            vec2(galley.size().x + 8.0, 18.0),
        );
        painter.rect_filled(badge, 2, BUTTON_ACTIVE);
        painter.galley(
            badge.left_center() + vec2(4.0, -galley.size().y / 2.0),
            galley,
            TEXT,
        );
        x = badge.right() + 4.0;
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, entry.localized_display_name()));
    response
        .on_hover_cursor(CursorIcon::Default)
        .on_hover_text(match &entry.availability {
            VoiceAvailability::Loadable => crate::lformat!(
                "{} · {} · 版本 {}\n{}",
                entry.localized_display_name(),
                family_name(entry),
                entry.revision,
                format!("{}\n{}",entry.localized_vendor(),entry.directory.display())
            ),
            VoiceAvailability::Unsupported(reason) => format!("{}\n{reason}\n{}", entry.localized_display_name(), entry.directory.display()),
        })
}

pub(super) fn source_hint(entry:&VoiceCatalogEntry)->String {
    let family=family_name(entry);
    format!("{} · {}",family,entry.revision)
}

fn language_tag(language: &str) -> &str {
    match language {
        "mandarin" | "mandarin-xsampa" | "zh" => "ZH",
        "english" | "english-arpabet" | "en" => "EN",
        "japanese" | "japanese-romaji" | "ja" => "JA",
        "spanish" | "spanish-xsampa" | "es" => "ES",
        "cantonese" | "cantonese-xsampa" | "yue" => "YUE",
        "korean" | "korean-xsampa" | "ko" => "KO",
        "italian" | "italian-xsampa" => "IT",
        "french" | "french-xsampa" => "FR",
        "german" | "german-xsampa" => "DE",
        "portuguese" | "portuguese-xsampa" => "PT-BR",
        other => other,
    }
}
