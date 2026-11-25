use crate::{
    app::App,
    synthesis::instruments::Entry,
    ui::{
        resource_manager::{self, Sort},
        theme::*,
    },
};
use eframe::egui::*;
use std::collections::BTreeMap;
#[derive(Clone, Default)]
pub(super) struct State {
    pub query: String,
    pub sort: Sort,
    pub versions: BTreeMap<String, crate::synthesis::voice::VoiceRevision>,
    pub expanded: Option<String>,
    pub available_only: bool,
}
pub fn dialog(app: &mut App, ctx: &Context) {
    let id = Id::new("instrument-manager-state");
    if !app.instrument_manager {
        ctx.data_mut(|d| d.remove::<State>(id));
        return;
    }
    app.synthesis.instruments.ensure_scan(ctx);
    let mut state = ctx
        .data_mut(|d| d.get_temp::<State>(id))
        .unwrap_or_default();
    let close = resource_manager::dialog(
        ctx,
        "instrument-manager",
        &mut (&mut *app, &mut state),
        |(app, _), ui, rect| {
            resource_manager::header(
                ui,
                rect,
                crate::i18n::t("Instrument"),
                crate::i18n::t("Close"),
                |ui| super::settings::show(app, ui),
            )
        },
        |(app, state), ui| body(app, ui, state),
    );
    ctx.data_mut(|d| d.insert_temp(id, state));
    if close {
        app.instrument_manager = false;
    }
}
fn toolbar(app: &mut App, ui: &mut Ui, state: &mut State) {
    resource_manager::toolbar_style(ui);
    let wide = ui.available_width() >= 670.;
    if wide {
        ui.horizontal(|ui| {
            resource_manager::search(
                ui,
                &mut state.query,
                230.,
                crate::i18n::t("Search Instruments"),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                actions(app, ui, state)
            });
        });
    } else {
        resource_manager::search(
            ui,
            &mut state.query,
            ui.available_width(),
            crate::i18n::t("Search Instruments"),
        );
        ui.add_space(10.);
        ui.horizontal_wrapped(|ui| actions(app, ui, state));
    }
}
fn actions(app: &mut App, ui: &mut Ui, state: &mut State) {
    let filter = ui.add(
        Button::new(crate::i18n::t("筛选"))
            .min_size(vec2(76., 36.))
            .corner_radius(14),
    );
    Popup::from_toggle_button_response(&filter)
        .width(240.)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.checkbox(&mut state.available_only, crate::i18n::t("可加载"));
        });
    resource_manager::sorting(ui, &mut state.sort);
    if ui
        .add_enabled(
            !app.synthesis.instruments.is_scanning(),
            Button::new(crate::i18n::t("刷新")).min_size(vec2(52., 36.)),
        )
        .clicked()
    {
        app.synthesis.rescan_instruments();
    }
}
fn body(app: &mut App, ui: &mut Ui, state: &mut State) {
    toolbar(app, ui, state);
    ui.add_space(28.);
    if app.synthesis.instruments.is_scanning() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(crate::i18n::t("Rescan"));
        });
        return;
    }
    if let Some(error) = &app.synthesis.instruments.scan_error {
        ui.colored_label(Color32::LIGHT_RED, crate::i18n::message(error));
    }
    let query = state.query.trim().to_lowercase();
    let mut banks: BTreeMap<String, Entry> = BTreeMap::new();
    for e in &app.synthesis.instruments.catalog.entries {
        if state.available_only && e.auxiliary.is_none() {
            continue;
        }
        if !query.is_empty()
            && !e.name.to_lowercase().contains(&query)
            && !crate::i18n::t(&e.name).to_lowercase().contains(&query)
        {
            continue;
        }
        let rank = |e: &Entry| {
            (
                state.versions.get(&e.selection.database_id) == Some(&e.selection.revision),
                super::current(app).is_some_and(|c| c.selection == e.selection),
                e.auxiliary.is_some(),
                e.selection.revision,
            )
        };
        if banks
            .get(&e.selection.database_id)
            .is_none_or(|old| rank(e) > rank(old))
        {
            banks.insert(e.selection.database_id.clone(), e.clone());
        }
    }
    let mut entries: Vec<_> = banks.into_values().collect();
    entries.sort_by(|a, b| {
        let names = || {
            crate::i18n::t(&a.name)
                .to_lowercase()
                .cmp(&crate::i18n::t(&b.name).to_lowercase())
                .then(a.selection.database_id.cmp(&b.selection.database_id))
        };
        match state.sort {
            Sort::NameReverse => names().reverse(),
            Sort::Version => b
                .selection
                .revision
                .cmp(&a.selection.revision)
                .then_with(names),
            _ => names(),
        }
    });
    if entries.is_empty() {
        resource_manager::product(ui, |ui| {
            ui.set_min_height(100.);
            ui.label(
                RichText::new(crate::i18n::t("Select Instrument"))
                    .font(section_title_font())
                    .size(16.),
            );
            if !state.query.is_empty() || state.available_only {
                if ui.button(crate::i18n::t("清除搜索与筛选")).clicked() {
                    state.query.clear();
                    state.available_only = false;
                }
            }
        });
    } else {
        ui.spacing_mut().item_spacing.y = 16.;
        for entry in entries {
            super::manager_card::show(app, ui, &entry, state);
        }
    }
    if !app.synthesis.instruments.catalog.warnings.is_empty() {
        ui.collapsing(crate::i18n::t("查看原因"), |ui| {
            for warning in &app.synthesis.instruments.catalog.warnings {
                ui.colored_label(Color32::LIGHT_RED, warning);
            }
        });
    }
}
