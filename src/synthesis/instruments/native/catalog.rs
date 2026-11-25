use super::{
    Result,
    archive::Archive,
    fail, id,
    metadata::{self, Value},
    render::PROGRAMS,
};
use crate::{
    model::InstrumentSelection,
    synthesis::{
        instruments::{Catalog, Entry, InstrumentSettings, UnavailableRevision},
        voice::VoiceRevision,
    },
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn identity(p: &Path) -> Result<String> {
    let s = std::fs::metadata(p).map_err(|e| e.to_string())?;
    Ok(format!(
        "{}:{}:{}",
        p.display(),
        s.len(),
        s.modified()
            .ok()
            .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos())
    ))
}
pub fn scan(settings: &InstrumentSettings, cancel: Arc<AtomicBool>) -> Result<Catalog> {
    let mut out = Catalog::default();
    let mut visited = HashSet::new();
    for mut root in settings.roots() {
        if root.join("databases").is_dir() {
            root = root.join("databases");
        }
        if !root.is_dir() {
            out.warnings
                .push(format!("{}: directory not found", root.display()));
            continue;
        }
        let is_bank = |path: &Path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| uuid::Uuid::parse_str(name).is_ok())
        };
        let selected_revision = root.parent().is_some_and(is_bank);
        let selected_bank = is_bank(&root);
        let source_root = if selected_revision {
            root.parent().and_then(Path::parent).unwrap_or(&root)
        } else if selected_bank {
            root.parent().unwrap_or(&root)
        } else {
            &root
        };
        let display = crate::synthesis::voice::MetadataIndex::load(source_root);
        let banks = if selected_revision {
            vec![root.parent().unwrap().to_owned()]
        } else if selected_bank {
            vec![root.clone()]
        } else {
            directories(&root)?
        };
        for bank in banks {
            if bank
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .ends_with(".disabled")
            {
                continue;
            }
            let revisions = if selected_revision {
                vec![root.clone()]
            } else {
                directories(&bank)?
            };
            for rev in revisions {
                let canonical = rev.canonicalize().unwrap_or_else(|_| rev.clone());
                if !visited.insert(canonical) {
                    continue;
                }
                let revision_name = rev.file_name().unwrap_or_default().to_string_lossy();
                let revision = match revision_name.parse::<VoiceRevision>() {
                    Ok(revision) => revision,
                    Err(error) => {
                        out.warnings.push(format!("{}: {error}", rev.display()));
                        continue;
                    }
                };
                let selection = InstrumentSelection {
                    database_id: bank
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    revision,
                };
                let mut mains = Vec::new();
                let mut aux = Vec::new();
                let mut errors = Vec::new();
                let mut models = std::fs::read_dir(&rev)
                    .map_err(|e| e.to_string())?
                    .filter_map(|e| e.ok().map(|v| v.path()))
                    .filter(|p| {
                        p.extension()
                            .is_some_and(|s| s.eq_ignore_ascii_case("dnni"))
                    })
                    .collect::<Vec<_>>();
                models.sort();
                for path in models {
                    if cancel.load(Ordering::Relaxed) {
                        return fail("Instrument scan cancelled");
                    }
                    let result = (|| -> Result<()> {
                        let a = Archive::open(&path, settings.accept_any_v4_signature)?;
                        if a.contains(&id(PROGRAMS[7])) {
                            let (root, nonce) = metadata::description(&a)?;
                            let extra = metadata::extra(&a)?;
                            let gain_db = match extra.get("gain_db") {
                                Some(Value::Float(v)) => *v as f32,
                                Some(Value::Int(v)) => *v as f32,
                                None => 0.,
                                _ => return fail("Invalid instrument model gain"),
                            };
                            if !gain_db.is_finite() || (gain_db > -100. && !10f32.powf(gain_db * 0.05).is_finite()) {
                                return fail("Invalid instrument model gain");
                            }
                            let pitch_range =
                                if let Some(Value::Array(v)) = extra.get("pitch_range") {
                                    if v.len() == 2 {
                                        if let (Value::Int(a), Value::Int(b)) = (&v[0], &v[1]) {
                                            Some([*a as i32, *b as i32])
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                            let room = if let Some(Value::String(s)) = extra.get("room") {
                                Some(s.clone())
                            } else {
                                None
                            };
                            let name = metadata::name(&root, nonce)?;
                            let vendor = super::manifest::vendor(&rev, &name, &selection.revision.to_string())
                                .unwrap_or_default();
                            let artwork = if vendor.is_empty() {
                                display.lookup_unique_name(&name)
                            } else {
                                display.lookup(&name, &vendor)
                            }
                                .and_then(|m| m.icon_path.as_ref())
                                .filter(|p| {
                                    std::fs::metadata(p).is_ok_and(|m| m.len() <= 8 * 1024 * 1024)
                                })
                                .and_then(|path| std::path::absolute(path).ok());
                            mains.push(Entry {
                                selection: selection.clone(),
                                name,
                                vendor,
                                main: path.clone(),
                                auxiliary: None,
                                pitch_range,
                                gain_db,
                                microphones: metadata::microphone_order(&root)?
                                    .into_iter().map(|(name,_)|name).collect(),
                                close_microphones: metadata::placements(&extra)?,
                                identity: identity(&path)?,
                                room,
                                artwork,
                                explicit_vibrato: super::protocol::Protocol::from_main(&a)?.explicit_vibrato(),
                            });
                        } else if a.contains(&id(PROGRAMS[0]))
                            && a.contains(&id(PROGRAMS[1]))
                        {
                            aux.push(path.clone());
                        }
                        Ok(())
                    })();
                    if let Err(e) = result {
                        out.warnings.push(format!("{}: {e}", path.display()));
                        errors.push(format!(
                            "{}: {e}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                    }
                }
                if !errors.is_empty() && (mains.is_empty() || aux.len() != 1) {
                    out.unavailable
                        .push(UnavailableRevision { selection, errors });
                }
                for mut entry in mains {
                    if aux.len() == 1 {
                        entry.auxiliary = Some(aux[0].clone());
                        entry.identity.push_str(&identity(&aux[0])?);
                    }
                    out.entries.push(entry);
                }
            }
        }
    }
    out.unavailable.retain(|failure| {
        !out.entries
            .iter()
            .any(|entry| entry.selection == failure.selection && entry.auxiliary.is_some())
    });
    use_latest_artwork(&mut out);
    out.entries.sort_by(|a, b| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
            .then(a.selection.database_id.cmp(&b.selection.database_id))
            .then(b.selection.revision.cmp(&a.selection.revision))
    });
    Ok(out)
}

fn use_latest_artwork(catalog: &mut Catalog) {
    let mut latest = HashMap::new();
    let revisions = catalog
        .entries
        .iter()
        .map(|entry| (&entry.selection, entry.artwork.as_ref()))
        .chain(catalog.unavailable.iter().map(|entry| (&entry.selection, None)));
    for (selection, artwork) in revisions {
        let revision = selection.revision;
        let newest = latest.entry(selection.database_id.clone()).or_insert_with(|| {
            (revision, artwork.cloned())
        });
        if revision > newest.0 {
            *newest = (revision, artwork.cloned());
        }
    }
    for entry in &mut catalog.entries {
        entry.artwork.clone_from(&latest[&entry.selection.database_id].1);
    }
}
fn directories(p: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut a = std::fs::read_dir(p)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|v| v.path()))
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>();
    a.sort();
    Ok(a)
}
