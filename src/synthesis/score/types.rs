use crate::model::{NoteMode, NotePerformance, RetakeSeeds};
use serde::{Deserialize, Serialize};

use super::{FRAME_SECONDS, HOP_SAMPLES, SAMPLE_RATE};
use crate::synthesis::{
    stages::{
        duration::{DurationNote, DurationRecord, ExplicitPhone},
        f0::frontend::{FrontendNote, frame_count},
    },
    voice::{PreparedTree, Result, VoiceError, VoiceFamily},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScorePhone {
    pub language: String,
    pub token: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_token: Option<String>,
    #[serde(default)]
    pub onset_offset_seconds: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreNote {
    #[serde(default)]
    pub source_note_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_error: Option<String>,
    #[serde(default)]
    pub retake_seeds: Option<RetakeSeeds>,
    #[serde(default)]
    pub performance: NotePerformance,
    pub duration_seconds: f64,
    pub pitch: f32,
    pub phones: Vec<ScorePhone>,
    #[serde(default)]
    pub kind: u32,
    #[serde(default)]
    pub flag: bool,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default = "one")]
    pub parts: u32,
    #[serde(default)]
    pub part_index: u32,
}

fn one() -> u32 {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplicitScore {

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocal_modes: Option<crate::model::PublicModeMap>,

    pub start_seconds: f64,
    pub visible_begin_seconds: f64,
    #[serde(default)]
    pub visible_end_seconds: Option<f64>,

    pub notes: Vec<ScoreNote>,
}

#[derive(Clone, Debug)]
pub struct PreparedPhrase {
    pub experimental_dds_expression:bool,
    pub experimental_style_range: Option<(f32,f32)>,

    pub(crate) unison_notes: Vec<FrontendNote>,
    pub unison: crate::synthesis::unison::UnisonSettings,
    pub note_errors: std::collections::BTreeMap<u64, String>,

    pub phone_labels: std::collections::BTreeMap<PhoneSource,String>,
    pub automation: Option<super::PreparedAutomation>,
    pub voice_parameters:crate::synthesis::parameters::VoiceParameters,
    pub vocal_modes: Option<crate::model::PublicModeMap>,
    pub(crate) mode_coefficients: Option<crate::synthesis::vocal_modes::VocalModeCoefficients>,

    pub pitch_controls: Option<super::PreparedPitchControls>,

    pub performance_controls: Option<super::PreparedPerformanceControls>,

    pub records: Vec<DurationRecord>,
    pub notes: Vec<FrontendNote>,

    pub source_note_ids: Vec<Option<u64>>,

    pub phone_sources: Vec<Vec<Option<PhoneSource>>>,
    pub phoneme_controls:std::collections::BTreeMap<PhoneSource,super::phonemes::PhoneControl>,

    pub retake_seeds: Vec<Option<RetakeSeeds>>,
    pub start_seconds: f64,
    pub visible_begin_seconds: f64,
    pub visible_end_seconds: Option<f64>,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]
pub struct PhoneSource {
    pub note_id:u64,
    pub phone_index:usize,
}

impl ExplicitScore {
    pub fn prepare(
        &self,
        duration_tree: &PreparedTree,
        family: VoiceFamily,
        default_language: &str,
    ) -> Result<PreparedPhrase> {
        if let Some(modes) = &self.vocal_modes {
            if !modes.is_empty() && !matches!(family,VoiceFamily::Sv2|VoiceFamily::Sv2Compatible) {
                return Err(VoiceError::new("Acoustic vocal modes require R3"));
            }
            crate::synthesis::vocal_modes::validate_public(modes)?;
        }
        if !self.start_seconds.is_finite()
            || !self.visible_begin_seconds.is_finite()
            || self
                .visible_end_seconds
                .is_some_and(|end| !end.is_finite() || end <= self.visible_begin_seconds)
        {
            return Err(VoiceError::new("Invalid score placement or visible bounds"));
        }
        let super::context::ContextNotes {
            start_seconds,
            notes: source,
            has_alignment_prefix,
        } = super::context::padded(self, family, default_language);
        let mut records = Vec::with_capacity(source.len());
        let mut notes = Vec::with_capacity(source.len());
        let mut silent_variants=std::collections::BTreeSet::new();
        for (index, note) in source.iter().enumerate() {
            super::retakes::validate_seeds(note.retake_seeds.unwrap_or_default(), family)?;
            super::options::validate_performance(note.performance, family)?;
            if !note.duration_seconds.is_finite()
                || note.duration_seconds <= 0.0
                || !note.pitch.is_finite()
                || !(0.0..=127.0).contains(&note.pitch)
                || (note.phones.is_empty() && note.kind != 2)
                || note.parts == 0
                || note.part_index >= note.parts
            {
                return Err(VoiceError::new(format!(
                    "Score record {index} has invalid duration, pitch, syllable partition or empty phones"
                )));
            }
            for (phone_index,phone) in note.phones.iter().enumerate() {
                if phone.token.is_empty()
                    || phone.token.chars().any(char::is_whitespace)
                    || phone
                        .onset_offset_seconds
                        .is_some_and(|value| !value.is_finite())
                {
                    return Err(VoiceError::new(format!(
                        "Score record {index} contains an invalid explicit phone"
                    )));
                }
                let resolved = duration_tree.resolve_phone(&phone.language, &phone.token)?;
                let silent_variant=note.kind==1 && super::pronunciation::special::numbered_breath(&phone.token);
                if resolved.used_silence_fallback && silent_variant {silent_variants.insert((index,phone_index));}

                if resolved.used_silence_fallback && !silent_variant {
                    return Err(VoiceError::new(format!(
                        "Score record {index}: unsupported phone {:?} in language {:?}",
                        phone.token, phone.language
                    )));
                }
            }
            let frontend = FrontendNote {
                phones: note
                    .phones
                    .iter()
                    .map(|phone| phone.token.clone())
                    .collect(),
                pitch: note.pitch,
                pitch_offset: 0.0,
                duration_seconds: note.duration_seconds,
                frame_seconds: FRAME_SECONDS,
                kind: note.kind,
                flag: note.flag
                    || (family != VoiceFamily::Sv1 && note.performance.mode == NoteMode::Rap),
                tag: note.tag.clone(),
                parts: note.parts,
                part_index: note.part_index,
                appended_durations: Vec::new(),
                performance: note.performance,
            };

            if family != VoiceFamily::Sv1
                && frame_count(&frontend)? == 0
                && !(has_alignment_prefix && index == 0)
            {
                return Err(VoiceError::new(format!(
                    "Score record {index} is shorter than one model frame"
                )));
            }
            records.push(DurationRecord {
                phones: note
                    .phones
                    .iter()
                    .enumerate()
                    .map(|(phone_index,phone)| ExplicitPhone {
                        language: phone.language.clone(),
                        token: if silent_variants.contains(&(index,phone_index)) {"sil".into()}else{phone.token.clone()},
                        onset_offset_seconds: phone.onset_offset_seconds,
                    })
                    .collect(),
                note: DurationNote {
                    sample_rate: SAMPLE_RATE,
                    hop_samples: HOP_SAMPLES as f64,
                    duration_seconds: note.duration_seconds,
                    pitch: note.pitch,
                },
                linked_durations_seconds: Vec::new(),
                parts: note.parts,
                part_index: note.part_index,
                pitch_offset: 0.0,
            });
            notes.push(frontend);
        }
        let mut counters=std::collections::BTreeMap::<u64,usize>::new();
        let mut phone_sources:Vec<Vec<_>>=source.iter().map(|note|note.phones.iter().map(|_|{
            note.source_note_id.map(|note_id|{
                let next=counters.entry(note_id).or_default();let phone_index=*next;*next+=1;
                PhoneSource{note_id,phone_index}
            })
        }).collect()).collect();
        let mut phone_labels=std::collections::BTreeMap::new();
        for (record,note) in source.iter().enumerate() {
            for (phone,p) in note.phones.iter().enumerate() {
                if let Some((source,label))=phone_sources[record][phone].zip(p.display_token.clone()) {
                    phone_labels.insert(source,label);
                }
            }
        }
        phone_labels.extend(silent_variants.into_iter().filter_map(|(record,phone)|
            phone_sources[record][phone].map(|source_id|(source_id,source[record].phones[phone].token.clone()))));
        let unison_notes = notes.clone();
        super::records::link_sustains(&mut records, &notes)?;
        super::records::redistribute_onsets(duration_tree, &mut records, &mut notes,&mut phone_sources, family)?;
        Ok(PreparedPhrase {
            experimental_dds_expression:false,
            experimental_style_range: None,
            unison_notes,
            unison:Default::default(),
            note_errors: source.iter().filter_map(|n| n.source_note_id.zip(n.note_error.clone())).collect(),
            phone_labels,
            automation: None,
            voice_parameters:Default::default(),
            vocal_modes: self.vocal_modes.clone(),
            mode_coefficients: None,
            pitch_controls: None,
            performance_controls: None,
            records,
            notes,
            source_note_ids: source.iter().map(|note| note.source_note_id).collect(),
            phone_sources,
            phoneme_controls:Default::default(),
            retake_seeds: source.iter().map(|note| note.retake_seeds).collect(),
            start_seconds,
            visible_begin_seconds: self.visible_begin_seconds,
            visible_end_seconds: self.visible_end_seconds,
        })
    }
}
