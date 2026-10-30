use super::units::*;
use crate::model::{
    Automation, Extra, Group, GroupRef, Note, Project, QUARTER, Track, pitch::PitchControl,
};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct NoteInput {
    pub id: u64,
    pub onset: LocalBlick,
    pub duration: i64,
    pub pitch: i32,
    pub lyrics: String,
    pub phonemes: String,
    pub musical_type: String,

    pub attributes: Extra,
    pub extensions: Extra,
}

impl From<&Note> for NoteInput {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id,
            onset: LocalBlick(note.onset),
            duration: note.duration,
            pitch: note.pitch,
            lyrics: note.lyrics.clone(),
            phonemes: note.phonemes.clone(),
            musical_type: note.musical_type.clone(),
            attributes: synthesis_extensions(&note.attributes),
            extensions: synthesis_extensions(&note.extra),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceInput {

    pub track_default_is_self: bool,
    pub model_source_identity: Option<String>,
    pub database: Value,
    pub instance_options: Extra,
    pub track_default_database: Value,
    pub track_default_options: Extra,
    pub track_options: Extra,
    pub dictionary: Option<crate::model::PronunciationDictionary>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TempoPoint {
    pub position: GlobalBlick,
    pub bpm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TempoMapping {
    pub origin_seconds: f64,
    pub points: Vec<TempoPoint>,
    pub extensions: Extra,
    pub event_extensions: Vec<(GlobalBlick, Extra)>,
}

impl TempoMapping {
    pub(super) fn new(project: &Project) -> Self {
        let mut points: Vec<TempoPoint> = Vec::new();
        for tempo in &project.time.tempo {

            if points.last().is_none_or(|p| p.bpm != tempo.bpm) {
                points.push(TempoPoint {
                    position: GlobalBlick(tempo.position),
                    bpm: tempo.bpm,
                });
            }
        }
        Self {
            origin_seconds: project.time.start_time_seconds,
            points,
            extensions: synthesis_extensions(&project.time.extra),
            event_extensions: project
                .time
                .tempo
                .iter()
                .filter_map(|tempo| {
                    let extra = synthesis_extensions(&tempo.extra);
                    (!extra.is_empty()).then_some((GlobalBlick(tempo.position), extra))
                })
                .collect(),
        }
    }

    pub fn seconds_at(&self, position: GlobalBlick) -> f64 {
        let mut seconds = self.origin_seconds;
        if position.0 < 0 {
            return seconds + position.0 as f64 / QUARTER as f64 * 60.0 / self.points[0].bpm;
        }
        for (index, tempo) in self.points.iter().enumerate() {
            if position.0 <= tempo.position.0 {
                break;
            }
            let end = self
                .points
                .get(index + 1)
                .map_or(position.0, |p| p.position.0.min(position.0));
            seconds += (end - tempo.position.0) as f64 / QUARTER as f64 * 60.0 / tempo.bpm;
        }
        seconds
    }

    pub fn position_at(&self, seconds: f64) -> GlobalBlick {
        let mut remaining = seconds - self.origin_seconds;
        if remaining < 0.0 {
            return GlobalBlick((remaining * self.points[0].bpm / 60.0 * QUARTER as f64) as i64);
        }
        for (index, tempo) in self.points.iter().enumerate() {
            if let Some(next) = self.points.get(index + 1) {
                let duration =
                    (next.position.0 - tempo.position.0) as f64 / QUARTER as f64 * 60.0 / tempo.bpm;
                if remaining >= duration {
                    remaining -= duration;
                    continue;
                }
            }

            return GlobalBlick(
                tempo.position.0 + (remaining * tempo.bpm / 60.0 * QUARTER as f64) as i64,
            );
        }
        GlobalBlick(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnerInput {
    pub experiments: crate::synthesis::experiments::ExperimentalContext,
    pub group_id: String,
    pub content_offset: GlobalBlick,
    pub visible_begin: GlobalBlick,
    pub visible_end: Option<GlobalBlick>,

    pub section_bounds: Option<(GlobalBlick, Option<GlobalBlick>)>,
    pub pitch_offset: i32,
    pub notes: Vec<NoteInput>,
    pub automation: BTreeMap<String, Automation>,
    pub pitch_controls: Vec<PitchControl>,
    pub group_options: Extra,
    pub voice: VoiceInput,
    pub tempo: TempoMapping,
    pub sample_rate: u32,
    pub output_options: Extra,
    pub project_options: Extra,
}

impl OwnerInput {

    pub fn matches_project(
        &self,
        project: &Project,
        track: &Track,
        reference: &GroupRef,
        group: &Group,
    ) -> bool {
        let mut candidate=Self::new(
                project,
                track,
                reference,
                group,
                self.voice.model_source_identity.as_deref(),
            );
        candidate.experiments=self.experiments.clone();
        candidate.sections().iter().any(|(_,input)| input==self)
    }

    pub(super) fn new(
        project: &Project,
        track: &Track,
        reference: &GroupRef,
        group: &Group,
        voice_identity: Option<&str>,
    ) -> Self {
        let mut output_options = synthesis_extensions(&project.render_config);
        for key in [
            "destination",
            "audioFormat",
            "filename",
            "exportMixDown",
            "exportPitch",
            "bitDepth",
            "sampleRate",
            "numChannels",
            "aspirationFormat",
            "bypassPan",
            "bypassGain",
            "bypassEffects",
        ] {
            output_options.remove(key);
        }
        let mut track_options = synthesis_extensions(&track.extra);

        track_options.remove("renderEnabled");
        Self {
            group_id: group.uuid.clone(),
            experiments: Default::default(),
            content_offset: GlobalBlick(reference.blick_offset),
            visible_begin: GlobalBlick(reference.blick_absolute_begin),
            visible_end: (reference.blick_absolute_end >= 0)
                .then_some(GlobalBlick(reference.blick_absolute_end)),
            section_bounds: None,
            pitch_offset: reference.pitch_offset,
            notes: group.notes.iter().map(NoteInput::from).collect(),
            automation: group.parameters.iter().map(|(k,v)|(k.clone(),v.clone()))
                .chain(group.vocal_modes.iter().map(|(k,v)|(format!("vocalMode_{k}"),v.clone())))
                .collect(),
            pitch_controls: group.pitch_controls.clone(),
            group_options: group_extensions(&group.extra),
            voice: VoiceInput {
                track_default_is_self: reference.id == track.main_ref.id,
                model_source_identity: voice_identity.map(str::to_owned),
                database: reference.database.clone(),
                instance_options: reference_extensions(&reference.extra),
                track_default_database: track.main_ref.database.clone(),
                track_default_options: reference_extensions(&track.main_ref.extra),
                track_options,
                dictionary: project.dictionary_for(track, reference).cloned(),
            },
            tempo: TempoMapping::new(project),

            sample_rate: crate::synthesis::score::SAMPLE_RATE,
            output_options,
            project_options: project_extensions(&project.extra),
        }
    }

    pub fn coverage(&self) -> Option<Coverage> {
        if self.sample_rate == 0 {
            return None;
        }
        let note_ends: Vec<_> = self
            .notes
            .iter()
            .filter(|note| note.attributes.get("muted").and_then(Value::as_bool) != Some(true))
            .filter_map(|note| {
                let start = note.onset.0.checked_add(self.content_offset.0)?;
                let end = start.checked_add(note.duration)?;
                (end > self.visible_begin.0 && self.visible_end.is_none_or(|limit| start < limit.0))
                    .then_some(end)
            })
            .collect();
        let last_note = *note_ends.iter().max()?;
        let (publication_begin, publication_end) = self.publication_bounds();
        let first_note = self.notes.iter().filter(|n| n.attributes.get("muted").and_then(Value::as_bool)!=Some(true))
            .filter_map(|n|n.onset.0.checked_add(self.content_offset.0)).min()?;

        let first_seconds = self.tempo.seconds_at(GlobalBlick(first_note));
        let context_start = ((first_seconds-0.5)/0.005/8.0).floor()*0.005*8.0;
        let start = if publication_begin.0 == 0 {
            GlobalBlick(self.tempo.position_at(context_start).0.saturating_sub(1).max(0))
        } else {publication_begin};
        let end = publication_end.unwrap_or_else(|| {
            let last_seconds = self.tempo.seconds_at(GlobalBlick(last_note));
            GlobalBlick(self.tempo.position_at(((last_seconds+0.5)/0.005).ceil()*0.005).0.saturating_add(1))
        });
        if end.0 <= start.0 {
            return None;
        }
        let start_sample = self.tempo.seconds_at(start) * self.sample_rate as f64;
        let end_sample = self.tempo.seconds_at(end) * self.sample_rate as f64;
        if !start_sample.is_finite()
            || !end_sample.is_finite()
            || start_sample < i64::MIN as f64
            || end_sample >= i64::MAX as f64
        {
            return None;
        }
        Some(Coverage {
            start,
            end,
            sample_start: SampleFrame(start_sample.floor() as i64),
            sample_end: SampleFrame(end_sample.ceil() as i64),
            sample_rate: self.sample_rate,
        })
    }
}

fn synthesis_extensions(extra: &Extra) -> Extra {
    extra
        .iter()
        .filter(|(key, _)| key.as_str() != "scriptData")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn group_extensions(extra: &Extra) -> Extra {
    let mut options = synthesis_extensions(extra);

    options.remove("musicalScale");
    options
}

fn reference_extensions(extra: &Extra) -> Extra {
    let mut options = synthesis_extensions(extra);
    options.remove("timestampLMR");
    options.remove("timestampLRSR");

    if options.get("voicePresetName").is_some_and(Value::is_string) {
        options.insert("voicePresetName".into(),Value::String(String::new()));
    }

    if options.get("isInstrumental").and_then(Value::as_bool) != Some(true)
        && options.get("audio").and_then(Value::as_object).is_some_and(|audio| {
            audio.iter().all(|(key,value)|match key.as_str() {
                "filename" => value.as_str()==Some(""),
                "duration" | "bpm" => value.as_f64()==Some(0.0),
                "alternativeBPMs" | "beatLocations" => value.as_array().is_some_and(Vec::is_empty),
                _ => false,
            })
        }) {options.remove("audio");}
    options
}

fn project_extensions(extra: &Extra) -> Extra {
    let mut options = synthesis_extensions(extra);

    for key in ["loopBegin", "loopEnd", "loopEnabled"] {options.remove(key);}

    if let Some(Value::Object(mixer)) = options.get_mut("projectMixer") {
        if mixer.get("linkRoomSettings").is_some_and(Value::is_boolean) {
            mixer.remove("linkRoomSettings");
        }
        if mixer.is_empty() {
            options.remove("projectMixer");
        }
    }
    options
}
