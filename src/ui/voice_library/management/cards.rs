mod actions;
use super::{
    super::{artwork, family_name, language_name},
    ACCENT, manager_card_color, manager_ink_color, State, catalog, details,
};
use crate::{
    app::App,
    synthesis::{runtime::VoiceState, voice::VoiceCatalogEntry},
    ui::{child, theme::*},
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui, entry: &VoiceCatalogEntry, state: &mut State) {
    ui.push_id((&entry.database_id, "voice-product"), |ui| {
        Frame::new()
            .fill(manager_card_color())
            .corner_radius(13)
            .inner_margin(20)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let compact = ui.available_width() < 620.0;
                let (rect, _) = ui.allocate_exact_size(
                    vec2(ui.available_width(), if compact { 152.0 } else { 88.0 }),
                    Sense::hover(),
                );
                let art = Rect::from_min_size(
                    rect.min + vec2(0.0, 4.0),
                    Vec2::splat(if compact { 60.0 } else { 80.0 }),
                );
                artwork::paint(ui, art, entry.icon.as_ref());
                let origin = pos2(art.right() + 18.0, rect.top() + 3.0);
                let end = rect.right() - if compact { 30.0 } else { 174.0 };
                let name = Rect::from_min_max(origin, pos2(end, origin.y + 26.0));
                ui.painter()
                    .with_clip_rect(name.intersect(ui.clip_rect()))
                    .text(
                        name.min,
                        Align2::LEFT_TOP,
                        entry.localized_display_name(),
                        FontId::new(
                            if compact { 17.0 } else { 20.0 },
                            section_title_font().family,
                        ),
                        manager_ink_color(),
                    );
                let meta = Rect::from_min_max(origin + vec2(0.0, 30.0), pos2(end, origin.y + 54.0));
                let changed = version(ui, app, entry, state, meta);
                let tags = Rect::from_min_max(
                    if compact {
                        rect.min + vec2(0.0, 78.0)
                    } else {
                        origin + vec2(0.0, 59.0)
                    },
                    pos2(
                        end,
                        if compact {
                            rect.top() + 100.0
                        } else {
                            rect.bottom()
                        },
                    ),
                );
                tags_row(ui, entry, tags);
                let current = app.voice_selection().as_ref() == Some(&catalog::selection(entry));
                let badge_rect = if compact {
                    Rect::from_min_max(
                        pos2(rect.left(), rect.bottom() - 34.0),
                        pos2(rect.right() - 162.0, rect.bottom()),
                    )
                } else {
                    Rect::from_min_max(
                        pos2(rect.right() - 160.0, rect.top()),
                        pos2(rect.right() - 32.0, rect.top() + 28.0),
                    )
                };
                badge(ui, app, entry, current, badge_rect, compact);
                let menu_rect =
                    Rect::from_min_size(rect.right_top() - vec2(24.0, 2.0), Vec2::splat(26.0));
                let mut menu_ui = child(ui, menu_rect, ui.id().with("voice-actions"));
                actions::menu(app, &mut menu_ui, entry, state, current);
                let action_rect = Rect::from_min_max(rect.max - vec2(146.0, 36.0), rect.max);
                let mut action_ui = child(ui, action_rect, ui.id().with("load-voice"));
                actions::primary(app, &mut action_ui, entry, state, current, !changed);
                if state.expanded.as_ref() == Some(&entry.database_id) {
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(10.0);
                    details::show(app, ui, entry);
                }
            });
    });
}

fn version(
    ui: &mut Ui,
    app: &App,
    entry: &VoiceCatalogEntry,
    state: &mut State,
    rect: Rect,
) -> bool {
    let text_width = (rect.width() - 76.0).max(0.0);
    let galley =
        ui.painter()
            .layout_no_wrap(entry.localized_vendor().to_owned(), FontId::proportional(12.0), muted_color());
    let vendor_width = galley.size().x.min(text_width);
    ui.painter()
        .with_clip_rect(
            Rect::from_min_size(rect.min, vec2(vendor_width, rect.height()))
                .intersect(ui.clip_rect()),
        )
        .galley(rect.min + vec2(0.0, 3.0), galley, muted_color());
    let mut revision = entry.revision;
    let mut controls = child(
        ui,
        Rect::from_min_size(rect.min + vec2(vendor_width + 8.0, 0.0), vec2(68.0, 24.0)),
        ui.id().with("revision"),
    );
    controls.spacing_mut().interact_size.y = 22.0;
    controls.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(10);
    controls.visuals_mut().widgets.inactive.bg_fill = button_color();
    controls.visuals_mut().widgets.inactive.weak_bg_fill = button_color();
    crate::ui::theme::native::combo("version")
        .width(64.0)
        .selected_text(format!("v{}", revision))
        .show_ui(&mut controls, |ui| {
            if let Some(catalog) = app.synthesis.library.catalog() {
                for version in catalog.entries.iter().filter(|v| {
                    v.database_id == entry.database_id
                        && (state.language.is_empty() || v.languages.contains(&state.language))
                        && (state.vendor.is_empty() || v.vendor == state.vendor)
                }) {
                    ui.selectable_value(
                        &mut revision,
                        version.revision,
                        crate::lformat!(
                            "版本 {}{}",
                            version.revision,
                            if version.availability.is_loadable() {
                                ""
                            } else {
                                crate::i18n::t(" · 不可用")
                            }
                        ),
                    );
                }
            }
        });
    if revision != entry.revision {
        state.versions.insert(entry.database_id.clone(), revision);
        true
    } else {
        false
    }
}

fn tags_row(ui: &Ui, entry: &VoiceCatalogEntry, rect: Rect) {
    let mut values = vec![family_name(entry)];
    if entry.genres.is_empty() {
        values.extend(entry.languages.iter().map(|s| language_name(s)));
    } else {
        values.extend(entry.genres.iter().map(String::as_str));
    }
    let mut x = rect.left();
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    for value in values {
        let galley = painter.layout_no_wrap(value.to_owned(), FontId::proportional(11.0), text_color());
        let width = galley.size().x + 18.0;
        if x + width > rect.right() {
            break;
        }
        let badge = Rect::from_min_size(pos2(x, rect.top()), vec2(width, 22.0));
        painter.rect_filled(badge, 11, Color32::from_rgb(65, 65, 65));
        painter.galley(
            badge.left_center() + vec2(9.0, -galley.size().y / 2.0),
            galley,
            text_color(),
        );
        x = badge.right() + 6.0;
    }
}

fn badge(ui: &Ui, app: &App, entry: &VoiceCatalogEntry, current: bool, rect: Rect, left: bool) {
    let key = entry.selection();
    let voice = app.synthesis.voice_state(&key);
    let (label, color, check) = if !matches!(voice, VoiceState::Unloaded) {
        match voice {
            VoiceState::Loaded(_) if app.synthesis.voice_error(&key).is_some() => {
                (crate::i18n::t("待配置"), Color32::from_rgb(219, 167, 101), false)
            }
            VoiceState::Loaded(_) => (if current { crate::i18n::t("当前音轨") } else { crate::i18n::t("已加载") }, ACCENT, true),
            VoiceState::Loading => (crate::i18n::t("正在加载"), muted_color(), false),
            VoiceState::Failed(_) => (crate::i18n::t("加载失败"), Color32::from_rgb(218, 142, 126), false),
            VoiceState::Unloaded => (crate::i18n::t("未加载"), muted_color(), false),
        }
    } else if entry.availability.is_loadable() {
        (crate::i18n::t("可加载"), muted_color(), false)
    } else {
        (crate::i18n::t("不可用"), muted_color(), false)
    };
    let point = if left {
        rect.left_center() + vec2(if check { 19.0 } else { 0.0 }, 0.0)
    } else {
        rect.right_center()
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(11.0), color);
    let x = if left {
        point.x
    } else {
        point.x - galley.size().x
    };
    ui.painter()
        .galley(pos2(x, point.y - galley.size().y / 2.0), galley, color);
    if check {
        let c = pos2(x - 12.0, point.y);
        crate::ui::icons::Icon::MenuCheck.paint(ui,Rect::from_center_size(c,vec2(18.0,15.0)),color);
    }
}
