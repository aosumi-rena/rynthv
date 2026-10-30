use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(skip, default = "new_id")]
    pub id: EntityId,
    #[serde(default = "fresh_note_uuid", deserialize_with = "read_note_uuid")]
    pub uuid: String,
    pub onset: i64,
    pub duration: i64,
    pub pitch: i32,
    pub lyrics: String,
    #[serde(default)]
    pub phonemes: String,
    #[serde(default = "singing")]
    pub musical_type: String,
    #[serde(default)]
    pub attributes: Extra,
    #[serde(flatten)]
    pub extra: Extra,
}
impl Note {
    pub fn new(onset: i64, duration: i64, pitch: i32) -> Self {
        Self {
            id: new_id(),
            uuid: fresh_note_uuid(),
            onset,
            duration,
            pitch,
            lyrics: "la".into(),
            phonemes: String::new(),
            musical_type: "singing".into(),
            attributes: BTreeMap::from([
                ("evenSyllableDuration".into(), json!(true)),
                ("muted".into(), json!(false)),
            ]),
            extra: BTreeMap::from([
                ("accent".into(), json!("")),
                ("detune".into(), json!(0)),
                ("takes".into(), TakeSet::default().to_value()),
            ]),
        }
    }
    pub fn renew_identity(&mut self) {
        self.id = new_id();
        self.uuid = fresh_note_uuid();
    }
    pub fn muted(&self) -> bool {
        self.attributes
            .get("muted")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

fn fresh_note_uuid() -> String {
    format!("{:016x}", uuid::Uuid::new_v4().as_u64_pair().0)
}

fn read_note_uuid<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<String,D::Error> {
    let value=String::deserialize(deserializer)?;

    Ok(if value.is_empty(){fresh_note_uuid()}else{value})
}

fn fresh_uuid() -> String { uuid::Uuid::new_v4().to_string() }
fn singing() -> String { "singing".into() }
fn unbounded() -> i64 { -1 }
fn empty_database() -> Value { json!({}) }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub uuid: String,
    pub notes: Vec<Note>,
    pub parameters: BTreeMap<String, Automation>,
    #[serde(default)]
    pub vocal_modes: BTreeMap<String, Automation>,
    #[serde(default)]
    pub pitch_controls: Vec<PitchControl>,
    #[serde(flatten)]
    pub extra: Extra,
}
impl Group {
    pub fn automation(&self, parameter: &Parameter) -> Option<&Automation> {
        match parameter {
            Parameter::VocalMode(_) => self.vocal_modes.get(parameter.label()),
            _ => self.parameters.get(parameter.key()),
        }
    }
    pub fn automation_mut(&mut self, parameter: &Parameter) -> &mut Automation {
        match parameter {
            Parameter::VocalMode(_) => self.vocal_modes.entry(parameter.label().into()).or_default(),
            _ => self.parameters.entry(parameter.key().into()).or_default(),
        }
    }
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            uuid: uuid::Uuid::new_v4().to_string(),
            notes: Vec::new(),
            parameters: Parameter::ALL
                .into_iter()
                .map(|p| (p.key().into(), Automation::default()))
                .collect(),
            vocal_modes: BTreeMap::new(),
            pitch_controls: Vec::new(),
            extra: BTreeMap::from([
                ("musicalScale".into(), json!({"type": "Major", "root": "C"})),
            ]),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRef {
    #[serde(skip, default = "new_id")]
    pub id: EntityId,
    #[serde(default = "fresh_uuid")]
    pub uuid: String,
    #[serde(rename = "groupID")]
    pub group_id: String,
    #[serde(default)]
    pub blick_absolute_begin: i64,
    #[serde(default = "unbounded")]
    pub blick_absolute_end: i64,
    #[serde(default)]
    pub blick_offset: i64,
    #[serde(default)]
    pub pitch_offset: i32,
    #[serde(default)]
    pub mute: bool,
    #[serde(default = "empty_database")]
    pub database: Value,
    #[serde(flatten)]
    pub extra: Extra,
}
impl GroupRef {
    pub fn renew_identity(&mut self) {
        self.id = new_id();
        self.uuid = uuid::Uuid::new_v4().to_string();
    }
    pub fn visible_end(&self) -> i64 {
        if self.blick_absolute_end < 0 {
            i64::MAX
        } else {
            self.blick_absolute_end
        }
    }
    pub fn contains(&self, global: i64) -> bool {
        global >= self.blick_absolute_begin && global < self.visible_end()
    }
    pub fn note_range(&self, note: &Note) -> Option<(i64, i64)> {
        let start = (note.onset + self.blick_offset).max(self.blick_absolute_begin);
        let end = (note.onset + note.duration + self.blick_offset).min(self.visible_end());
        (start < end).then_some((start, end))
    }
    pub fn new(group: &Group, end: i64) -> Self {
        Self {
            id: new_id(),
            uuid: uuid::Uuid::new_v4().to_string(),
            group_id: group.uuid.clone(),
            blick_absolute_begin: 0,
            blick_absolute_end: end,
            blick_offset: 0,
            pitch_offset: 0,
            mute: false,
            database: json!({"name":"","language":"","phoneset":"","languageOverride":"","phonesetOverride":"","backendType":"","version":"-2"}),
            extra: BTreeMap::from([
                ("isInstrumental".into(), json!(false)),
                ("dictionary".into(), json!("")),
                (
                    "voice".into(),
                    json!({"vocalModeInherited":true,"vocalModePreset":"","vocalModeParams":{}}),
                ),
                ("voicePresetName".into(), json!("")),
                ("takes".into(), TakeSet::default().to_value()),
            ]),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    #[serde(skip, default = "new_id")]
    pub id: EntityId,
    pub name: String,
    pub disp_color: TrackColor,
    pub disp_order: i32,
    pub main_group: Group,
    pub main_ref: GroupRef,
    pub groups: Vec<GroupRef>,
    pub mixer: Extra,

    #[serde(skip)]
    pub instrumental: Option<InstrumentalTrack>,
    #[serde(flatten)]
    pub extra: Extra,
}
impl Track {
    pub fn new(index: usize) -> Self {
        let main_group = Group::new("main");
        Self {
            id: new_id(),
            name: format!("未命名音轨 {}", index + 1),
            disp_color: TrackColor::default(),
            disp_order: index as i32,
            main_ref: GroupRef::new(&main_group, -1),
            main_group,
            groups: Vec::new(),
            mixer: BTreeMap::from([
                ("gainDecibel".into(), json!(0.0)),
                ("pan".into(), json!(0.0)),
                ("mute".into(), json!(false)),
                ("solo".into(), json!(false)),
                ("display".into(), json!(true)),
            ]),
            extra: BTreeMap::from([("renderEnabled".into(), json!(false))]),
            instrumental: None,
        }
    }
    pub fn references(&self) -> impl Iterator<Item = &GroupRef> {
        std::iter::once(&self.main_ref).chain(&self.groups)
    }
    pub fn is_instrumental(&self) -> bool {
        self.instrumental.is_some()
    }
    pub fn audio_clip(&self) -> Option<&AudioClip> {
        self.instrumental.as_ref()?.clip.as_ref()
    }

    pub fn audio_window(&self) -> Option<(f64, f64)> { Some(self.audio_clip()?.window()) }
    pub fn visible_references(&self) -> impl Iterator<Item = &GroupRef> {
        std::iter::once(&self.main_ref)
            .filter(|_| !self.main_group.notes.is_empty())
            .chain(&self.groups)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub version: i32,
    #[serde(default = "fresh_uuid")]
    pub uuid: String,
    pub time: TimeAxis,
    #[serde(default)]
    pub library: Vec<Group>,
    pub tracks: Vec<Track>,
    pub render_config: Extra,
    #[serde(
        default,
        rename = "rynthvDictionaries",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub dictionaries: BTreeMap<String, PronunciationDictionary>,
    #[serde(flatten)]
    pub extra: Extra,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            version: SVP_VERSION,
            uuid: uuid::Uuid::new_v4().to_string(),
            time: TimeAxis::default(),
            library: Vec::new(),
            tracks: vec![Track::new(0)],
            render_config: BTreeMap::from([
                ("destination".into(), json!("./")),
                ("audioFormat".into(), json!("wav")),
                ("filename".into(), json!("未命名")),
                ("numChannels".into(), json!(1)),
                ("aspirationFormat".into(), json!("noAspiration")),
                ("bitDepth".into(), json!(16)),
                ("sampleRate".into(), json!(44100)),
                ("exportMixDown".into(), json!(true)),
                ("exportPitch".into(), json!(false)),
            ]),
            extra: Extra::new(),
            dictionaries: BTreeMap::new(),
        }
    }
}
impl Project {
    pub fn group(&self, id: &str) -> Option<&Group> {
        self.library
            .iter()
            .chain(self.tracks.iter().map(|t| &t.main_group))
            .find(|g| g.uuid == id)
    }
    pub fn group_mut(&mut self, id: &str) -> Option<&mut Group> {
        self.library
            .iter_mut()
            .chain(self.tracks.iter_mut().map(|t| &mut t.main_group))
            .find(|g| g.uuid == id)
    }
    pub fn end(&self) -> i64 {
        self.tracks
            .iter()
            .flat_map(Track::visible_references)
            .map(|r| {
                if r.blick_absolute_end >= 0 {
                    r.blick_absolute_end
                } else {
                    self.group(&r.group_id)
                        .and_then(|g| {
                            g.notes
                                .iter()
                                .filter_map(|n| r.note_range(n).map(|range| range.1))
                                .max()
                        })
                        .unwrap_or(0)
                }
            })
            .chain(self.tracks.iter().filter_map(|t| {
                t.audio_window().map(|(_,end)|self.time.position_at(end))
            }))
            .max()
            .unwrap_or(0)
    }
}
