use crate::{
    app::App,
    ui::{icons::Icon, theme::*},
};
use eframe::egui::*;

pub(super) fn card(app: &mut App, ui: &mut Ui) {
    let entry = super::current(app);
    let name = entry
        .map(|e| e.name.as_str())
        .unwrap_or(crate::i18n::t("Select Instrument"));
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 230.0), Sense::click());
    ui.painter().rect_filled(
        rect,
        5,
        if response.hovered() {
            BUTTON_HOVER
        } else {
            BORDER
        },
    );
    let art = Rect::from_min_max(rect.min + vec2(22.0, 18.0), rect.max - vec2(22.0, 52.0));
    artwork(ui, art, entry);
    ui.painter().text(
        pos2(rect.center().x, rect.bottom() - 25.0),
        Align2::CENTER_CENTER,
        crate::i18n::t(name),
        FontId::proportional(15.0),
        TEXT,
    );
    Icon::ChevronDown.paint(
        ui,
        Rect::from_center_size(rect.max - vec2(20.0, 25.0), Vec2::splat(12.0)),
        MUTED,
    );
    popup(app, &response);
}

pub(super) fn artwork(ui: &Ui, art: Rect, entry: Option<&crate::synthesis::instruments::Entry>) {
    if let Some(entry) = entry {
        if let Some(svg) = &entry.artwork {
            Image::from_bytes(
                format!(
                    "bytes://instrument/{:016x}.png",
                    xxhash_rust::xxh3::xxh3_64(svg)
                ),
                svg.clone(),
            )
            .fit_to_exact_size(art.size())
            .paint_at(ui, art);
        } else {
            Icon::Instrument.paint(ui, art.shrink(24.0), TEXT);
        }
    } else {
        Icon::Instrument.paint(ui, art.shrink(24.0), MUTED);
    }
}

pub fn popup(app: &mut App, anchor: &Response) {
    let selected = super::current(app).map(|e| e.selection.clone());
    if let Some(entry) = pick(app, anchor, selected.as_ref(), true) {
        app.commit_lyric();
        app.editor.select_instrument(&entry);
    }
}

pub fn waveform_picker(
    app: &mut App,
    ui: &mut Ui,
    selected: &mut Option<crate::model::InstrumentSelection>,
) {
    let entry = selected.as_ref().and_then(|selection| app.synthesis.instruments.entry(selection));
    let label = crate::i18n::t("Select Instrument");
    let hint = entry.map(|entry| format!("v{}", entry.selection.revision));
    let anchor = crate::ui::resource_picker::selection_card(
        ui, entry.map_or(label, |entry| crate::i18n::t(&entry.name)), hint.as_deref(),
        entry.map(|entry| entry.main.display().to_string()), label,
        |ui, rect| artwork(ui, rect, entry),
    );
    if let Some(entry) = pick(app, &anchor, selected.as_ref(), false) {
        *selected = Some(entry.selection);
    }
}

fn pick(
    app: &mut App,
    anchor: &Response,
    selected: Option<&crate::model::InstrumentSelection>,
    require_auxiliary: bool,
) -> Option<crate::synthesis::instruments::Entry> {
    if anchor.clicked() {
        app.synthesis.instruments.ensure_scan(&anchor.ctx);
    }
    let popup = Popup::menu(anchor);
    let id = popup.get_id().with("instrument-search");
    let focus = id.with("focus");
    if !popup.is_open() {
        anchor.ctx.data_mut(|d| {
            d.remove::<String>(id);
            d.insert_temp(focus, true);
        });
    }
    let mut query = anchor
        .ctx
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_default();
    let mut banks = std::collections::BTreeMap::new();
    for entry in &app.synthesis.instruments.catalog.entries {
        let current = banks
            .entry(entry.selection.database_id.clone())
            .or_insert(entry);
        let rank = |e: &crate::synthesis::instruments::Entry| {
            (
                selected == Some(&e.selection),
                !require_auxiliary || e.auxiliary.is_some(),
                e.selection.revision.clone(),
            )
        };
        if rank(entry) > rank(current) {
            *current = entry;
        }
    }
    let mut entries: Vec<_> = banks.into_values().cloned().collect();
    entries.sort_by_key(|e| crate::i18n::t(&e.name).to_lowercase());
    let mut choose = None;
    popup
        .width(288.0)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(crate::ui::resource_picker::frame(&anchor.ctx))
        .show(|ui| {
            ui.set_width(280.0);
            ui.set_height(crate::ui::resource_picker::height(ui.ctx()));
            ui.spacing_mut().item_spacing = vec2(0., 4.);
            crate::ui::resource_picker::header(ui, crate::i18n::t("Instrument"));
            let pending_focus = ui
                .ctx()
                .data(|d| d.get_temp::<bool>(focus))
                .unwrap_or(false);
            if pending_focus {
                ui.ctx().memory_mut(|m| m.request_focus(id.with("input")));
            }
            let search = ui.add_sized(
                vec2(ui.available_width(), 28.0),
                TextEdit::singleline(&mut query)
                    .id(id.with("input"))
                    .hint_text(crate::i18n::t("Search Instruments"))
                    .vertical_align(Align::Center),
            );
            if pending_focus {
                search.request_focus();
                ui.ctx().request_repaint();
                if !ui.is_sizing_pass() && ui.is_visible() && ui.is_enabled() {
                    ui.ctx().data_mut(|d| d.remove::<bool>(focus));
                }
            }
            ui.ctx().data_mut(|d| d.insert_temp(id, query.clone()));
            crate::ui::resource_picker::separator(ui);
            if require_auxiliary {
                if crate::ui::resource_picker::menu_row(
                    ui,
                    crate::i18n::t("Manage Instruments"),
                    Some(Icon::Cloud),
                    true,
                ).clicked() {
                    app.instrument_manager = true;
                    ui.close();
                }
                crate::ui::resource_picker::separator(ui);
            }
            if app.synthesis.instruments.is_scanning() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(crate::i18n::t("正在加载…"));
                });
            }
            if let Some(error) = &app.synthesis.instruments.scan_error {
                ui.colored_label(Color32::LIGHT_RED, crate::i18n::message(error));
            }
            let query = query.to_lowercase();
            let mut list = ScrollArea::vertical()
                .id_salt("instrument-picker-list")
                .max_height(ui.available_height())
                .auto_shrink([false, false]);
            if search.changed() {
                list = list.vertical_scroll_offset(0.);
            }
            list.show(ui, |ui| {
                for entry in &entries {
                    let name = crate::i18n::t(&entry.name);
                    if !name.to_lowercase().contains(&query)
                        && !entry.name.to_lowercase().contains(&query)
                        && !entry.vendor.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let available = !require_auxiliary || entry.auxiliary.is_some();
                    let (response, art, content) = crate::ui::resource_picker::card(
                        ui,
                        selected == Some(&entry.selection),
                        available,
                    );
                    artwork(ui, art, Some(entry));
                    let painter = ui
                        .painter()
                        .with_clip_rect(content.intersect(ui.clip_rect()));
                    painter.text(
                        content.min,
                        Align2::LEFT_TOP,
                        name,
                        section_title_font(),
                        if available { TEXT } else { MUTED },
                    );
                    painter.text(
                        content.min + vec2(0., 23.),
                        Align2::LEFT_TOP,
                        crate::i18n::t(&entry.vendor),
                        FontId::proportional(12.),
                        MUTED,
                    );
                    painter.text(
                        content.min + vec2(0., 44.),
                        Align2::LEFT_TOP,
                        format!("v{}", entry.selection.revision),
                        FontId::proportional(11.),
                        MUTED,
                    );
                    response
                        .widget_info(|| WidgetInfo::labeled(WidgetType::Button, available, name));
                    if response
                        .on_hover_text(entry.main.display().to_string())
                        .clicked() && available
                    {
                        choose = Some(entry.clone());
                        ui.close();
                    }
                }
            });
        });
    choose
}
