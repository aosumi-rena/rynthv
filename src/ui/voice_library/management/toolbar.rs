use super::{super::language_name, CARD, INK, Sort, State};
use crate::{
    app::App,
    ui::{
        child,
        icons::{Icon, IconButton},
        theme::*,
    },
};
use eframe::egui::*;
use std::collections::BTreeSet;

pub(super) fn show(app: &mut App, ui: &mut Ui, state: &mut State) {
    ui.spacing_mut().interact_size.y = 36.0;
    ui.visuals_mut().widgets.inactive.bg_fill = CARD;
    ui.visuals_mut().widgets.inactive.weak_bg_fill = CARD;
    ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(12);
    let wide = ui.available_width() >= 670.0;
    if wide {
        ui.horizontal(|ui| {
            search(ui, &mut state.query, 230.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                actions(app, ui, state)
            });
        });
    } else {
        search(ui, &mut state.query, ui.available_width());
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| actions(app, ui, state));
    }
}

fn search(ui: &mut Ui, query: &mut String, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 5, Color32::from_rgb(43, 43, 43));
    let center = rect.left_center() + vec2(19.0, -1.0);
    ui.painter()
        .circle_stroke(center, 5.5, Stroke::new(1.8, TEXT));
    ui.painter().line_segment(
        [center + vec2(4.0, 4.0), center + vec2(9.0, 9.0)],
        Stroke::new(1.8, TEXT),
    );
    let has_text = !query.is_empty();
    let mut input = child(
        ui,
        Rect::from_min_max(
            rect.min + vec2(35.0, 0.0),
            rect.max - vec2(if has_text { 30.0 } else { 8.0 }, 0.0),
        ),
        ui.id().with("voice-search"),
    );
    input.add_sized(
        input.max_rect().size(),
        TextEdit::singleline(query)
            .frame(Frame::NONE)
            .font(FontId::proportional(14.0))
            .hint_text(crate::i18n::t("搜索歌声"))
            .vertical_align(Align::Center)
            .margin(vec2(0.0, 9.0)),
    );
    if has_text {
        let mut clear = child(
            ui,
            Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), Vec2::splat(24.0)),
            "clear-voice-search",
        );
        if clear
            .add(
                IconButton::new(Icon::Close, crate::i18n::t("清除搜索"))
                    .size(Vec2::splat(24.0))
                    .icon_size(12.0),
            )
            .clicked()
        {
            query.clear();
        }
    }
}

fn filters(app: &App, ui: &mut Ui, state: &mut State) {
    let count = usize::from(!state.language.is_empty()) + usize::from(!state.vendor.is_empty());
    let filter = ui.add(
        Button::new(if count == 0 {
            crate::i18n::t("筛选").into()
        } else {
            crate::lformat!("筛选 · {count}")
        })
        .min_size(vec2(76.0, 36.0))
        .corner_radius(14),
    );
    Popup::from_toggle_button_response(&filter)
        .width(240.0)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                let mut languages = BTreeSet::new();
                let mut vendors = BTreeSet::new();
                if let Some(catalog) = app.synthesis.library.catalog() {
                    for entry in &catalog.entries {
                        languages.extend(entry.languages.iter().cloned());
                        if !entry.vendor.is_empty() {
                            vendors.insert(entry.vendor.clone());
                        }
                    }
                }
                ui.label(RichText::new(crate::i18n::t("筛选歌声")).strong().color(INK));
                ui.weak(crate::i18n::t("声库语言"));
                ComboBox::from_id_salt("manager-language-filter")
                    .width(ui.available_width())
                    .selected_text(if state.language.is_empty() {
                        crate::i18n::t("全部语言")
                    } else {
                        language_name(&state.language)
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.language, String::new(), crate::i18n::t("全部语言"));
                        for language in languages {
                            let label = language_name(&language).to_owned();
                            ui.selectable_value(&mut state.language, language, label);
                        }
                    });
                ui.weak(crate::i18n::t("厂商"));
                ComboBox::from_id_salt("manager-vendor-filter")
                    .width(ui.available_width())
                    .selected_text(if state.vendor.is_empty() {
                        crate::i18n::t("全部厂商")
                    } else {
                        &state.vendor
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.vendor, String::new(), crate::i18n::t("全部厂商"));
                        for vendor in vendors {
                            ui.selectable_value(&mut state.vendor, vendor.clone(), vendor);
                        }
                    });
                if ui.button(crate::i18n::t("清除筛选")).clicked() {
                    state.language.clear();
                    state.vendor.clear();
                }
            });
        });
    if count > 0 {
        filter.on_hover_text(crate::i18n::t("已启用筛选")).highlight();
    }
}

fn actions(app: &mut App, ui: &mut Ui, state: &mut State) {
    let reverse = ui.layout().main_dir() == Direction::RightToLeft;
    if reverse {
        filters(app, ui, state);
        sorting(ui, state, true);
        refresh(app, ui);
    } else {
        refresh(app, ui);
        sorting(ui, state, false);
        filters(app, ui, state);
    }
}

fn sorting(ui: &mut Ui, state: &mut State, reverse: bool) {
    if !reverse {
        ui.label(RichText::new(crate::i18n::t("排序方式")).color(MUTED));
    }
    ComboBox::from_id_salt("voice-manager-sort")
        .width(90.0)
        .selected_text(state.sort.label())
        .show_ui(ui, |ui| {
            for sort in [Sort::Default, Sort::Name, Sort::NameReverse, Sort::Version] {
                ui.selectable_value(&mut state.sort, sort, sort.label());
            }
        });
    if reverse {
        ui.label(RichText::new(crate::i18n::t("排序方式")).color(MUTED));
    }
}

fn refresh(app: &mut App, ui: &mut Ui) {
    if ui
        .add_enabled(
            !app.synthesis.library.is_scanning() && app.synthesis.library.settings.has_databases(),
            Button::new(crate::i18n::t("刷新")).min_size(vec2(52.0, 36.0)),
        )
        .clicked()
    {
        app.synthesis.rescan_voices();
    }
}
