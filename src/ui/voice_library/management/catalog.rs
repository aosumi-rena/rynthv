use super::{
    super::{family_name, language_name},
    Sort, State,
};
use crate::{
    app::App,
    synthesis::{settings::VoiceSelection, voice::VoiceCatalogEntry},
};
use std::collections::BTreeMap;

pub(super) fn selection(entry: &VoiceCatalogEntry) -> VoiceSelection {
    VoiceSelection {
        database_id: entry.database_id.clone(),
        revision: entry.revision,
    }
}

pub(super) fn entries(app: &App, state: &State) -> Vec<VoiceCatalogEntry> {
    let mut banks: BTreeMap<&str, &VoiceCatalogEntry> = BTreeMap::new();
    let query = state.query.trim().to_lowercase();
    if let Some(catalog) = app.synthesis.library.catalog() {
        let rank = |entry: &VoiceCatalogEntry| {
            (
                state.versions.get(&entry.database_id) == Some(&entry.revision),
                app.voice_selection().as_ref() == Some(&selection(entry)),
                entry.availability.is_loadable(),
                entry.revision,
            )
        };
        for entry in &catalog.entries {
            if !state.language.is_empty() && !entry.languages.contains(&state.language)
                || !state.vendor.is_empty() && entry.vendor != state.vendor
            {
                continue;
            }
            let matches = query.is_empty()
                || entry.localized_display_name().to_lowercase().contains(&query)
                || entry.localized_vendor().to_lowercase().contains(&query)
                || family_name(entry).to_lowercase().contains(&query)
                || entry.languages.iter().any(|s| {
                    language_name(s).contains(&query) || s.to_lowercase().contains(&query)
                });
            if !matches {
                continue;
            }
            let current = banks.entry(&entry.database_id).or_insert(entry);
            if rank(entry) > rank(current) {
                *current = entry;
            }
        }
    }
    let mut entries: Vec<_> = banks.into_values().cloned().collect();
    entries.sort_by(|a, b| {
        let category=super::super::selector::category_of(a).cmp(&super::super::selector::category_of(b));
        if !category.is_eq() {return category;}
        let names = || {
            a.localized_display_name().to_lowercase()
                .cmp(&b.localized_display_name().to_lowercase())
                .then_with(|| a.localized_display_name().cmp(b.localized_display_name()))
                .then_with(|| a.database_id.cmp(&b.database_id))
        };
        match state.sort {
            Sort::Default => names(),
            Sort::Name => names(),
            Sort::NameReverse => names().reverse(),
            Sort::Version => b.revision.cmp(&a.revision).then_with(names),
        }
    });
    entries
}
