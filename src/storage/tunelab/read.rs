use super::*;
use std::collections::BTreeMap;

pub(super) fn project(source: Value) -> Result<Project> {
    let version = source
        .get("version")
        .and_then(Value::as_u64)
        .ok_or("Missing TuneLab version")?;
    if version > 1 {
        return Err(format!("Unsupported TuneLab version {version}"));
    }
    let mut p = Project::default();
    p.tracks.clear();
    p.time.tempo.clear();
    p.time.meter.clear();
    p.extra.insert(
        SOURCE.into(),
        retained(&source, &["tracks", "tempos", "timeSignatures"]),
    );
    for mark in array(&source, "tempos")? {
        p.time.tempo.push(Tempo {
            position: clock(number(mark, "pos", 0.)?)?,
            bpm: number(mark, "bpm", 120.)?,
            extra: Extra::new(),
        });
    }
    for mark in array(&source, "timeSignatures")? {
        p.time.meter.push(serde_json::from_value(json!({"index":mark["barIndex"],"numerator":mark["numerator"],"denominator":mark["denominator"]})).map_err(|e|e.to_string())?);
    }
    controls::export_config(&source["exportConfig"], &mut p.render_config, true);
    for track in array(&source, "tracks")? {
        let origin = uuid::Uuid::new_v4().to_string();
        let mut destinations: BTreeMap<String, usize> = BTreeMap::new();
        let parts = array(track, "parts")?;
        if parts.is_empty() {
            p.tracks.push(track_header(track, p.tracks.len(), &origin)?);
        }
        for (part_index, part) in parts.iter().enumerate() {
            let kind = text(part, "type", "");
            if !["midi", "audio"].contains(&kind) {
                return Err(format!("Unsupported TuneLab part type {kind}"));
            }
            let instrument =
                part.pointer("/voice/kind").and_then(Value::as_str) == Some("instrument");
            let bank = text(&part["voice"], "id", "");
            let revision = text(
                &part["properties"],
                if instrument {
                    "instrument_version"
                } else {
                    "voice_version"
                },
                "",
            );
            let binding = if kind == "audio" {
                format!("audio:{part_index}")
            } else if instrument {
                format!("instrument:{bank}:{revision}")
            } else {
                "voice".into()
            };
            let index = if let Some(&i) = destinations.get(&binding) {
                i
            } else {
                let i = p.tracks.len();
                let mut t = track_header(track, i, &origin)?;
                if instrument {
                    t.instrument = Some(InstrumentTrack {
                        selection: if bank.is_empty() || revision.is_empty() || revision == "-2" {
                            None
                        } else {
                            Some(InstrumentSelection {
                                database_id: bank.into(),
                                revision: revision.parse()?,
                            })
                        },
                        ambient_seed: 0,
                    });
                }
                p.tracks.push(t);
                destinations.insert(binding, i);
                i
            };
            let anchor = clock(number(part, "pos", 0.)?)?;
            let start = if version == 0 {
                0
            } else {
                clock(number(part, "startOffset", 0.)?)?
            };
            let end = clock(number(
                part,
                if version == 0 { "dur" } else { "endOffset" },
                0.,
            )?)?;
            if end < start {
                return Err("Reversed TuneLab part bounds".into());
            }
            if kind == "audio" {
                let source_start = p.time.seconds_at(anchor);
                let stop = p.time.seconds_at(anchor + end);
                let path = text(part, "path", "");
                let clip = (!path.is_empty()).then(|| AudioClip {
                    path: path.into(),
                    onset_seconds: source_start,
                    duration_seconds: (stop - source_start).max(1. / 48000.),
                    trim_begin_seconds: Some(p.time.seconds_at(anchor + start)),
                    trim_end_seconds: Some(stop),
                });
                let t = &mut p.tracks[index];
                t.instrumental = Some(InstrumentalTrack { clip });
                t.main_ref.extra.insert(SOURCE.into(), part.clone());
                t.main_ref
                    .extra
                    .insert("rynthvTuneLabPartOrder".into(), json!(part_index));
                continue;
            }
            let mut group = Group::new(text(part, "name", ""));
            group
                .extra
                .insert(SOURCE.into(), retained(part, &["notes"]));
            for row in array(part, "notes")? {
                let pitch = number(row, "pitch", 60.)?;
                if pitch.fract() != 0. || !(0. ..=127.).contains(&pitch) {
                    return Err("Invalid TuneLab note pitch".into());
                }
                let mut n = Note::new(
                    clock(number(row, "pos", 0.)?)?,
                    clock(number(row, "dur", 0.)?)?,
                    pitch as i32,
                );
                n.lyrics = text(row, "lyric", "").into();
                n.extra.insert(SOURCE.into(), row.clone());
                controls::read_note(row, &mut n, instrument, version)?;
                group.notes.push(n);
            }
            controls::read_curves(part, &mut group)?;
            let mut reference = GroupRef::new(&group, anchor + end);
            reference.blick_offset = anchor;
            reference.blick_absolute_begin = anchor + start;
            reference
                .extra
                .insert("rynthvTuneLabPartOrder".into(), json!(part_index));
            reference.database = json!({"id":bank,"name":text(&part["voice"],"name",""),"backendType":if instrument{"W"}else{""},"version":revision});
            controls::read_voice(part, &mut reference)?;
            if !p.tracks[index].main_group.extra.contains_key(SOURCE)
                && p.tracks[index].groups.is_empty()
            {
                p.tracks[index].main_group = group;
                p.tracks[index].main_ref = reference;
            } else {
                p.library.push(group);
                p.tracks[index].groups.push(reference);
            }
        }
    }
    if p.tracks.is_empty() {
        p.tracks.push(Track::new(0));
    }
    Ok(p)
}
fn track_header(v: &Value, index: usize, origin: &str) -> Result<Track> {
    let mut t = Track::new(index);
    t.name = text(v, "name", "").into();
    t.extra.insert(SOURCE.into(), retained(v, &["parts"]));
    t.extra.insert("rynthvTuneLabTrackId".into(), json!(origin));
    if let Some(c) = v.get("color").and_then(Value::as_str) {
        let c = c.trim_start_matches('#');
        t.disp_color = serde_json::from_value(json!(if c.len() == 6 {
            format!("ff{c}")
        } else {
            c.to_owned()
        }))
        .map_err(|e| format!("TuneLab track color: {e}"))?;
    }
    for key in ["mute", "solo"] {
        t.mixer.insert(
            key.into(),
            json!(v.get(key).and_then(Value::as_bool).unwrap_or(false)),
        );
    }
    t.mixer
        .insert("gainDecibel".into(), json!(number(v, "gain", 0.)?));
    t.mixer.insert("pan".into(), json!(number(v, "pan", 0.)?));
    t.extra.insert(
        "renderEnabled".into(),
        json!(
            v.get("exportEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        ),
    );
    Ok(t)
}
