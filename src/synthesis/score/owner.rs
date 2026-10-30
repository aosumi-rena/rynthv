use super::{
    ExplicitScore, FRAME_SECONDS, LyricFrontend, PreparedPhrase, ScoreNote, ScorePhone, options,
};
use crate::{
    model::QUARTER,
    synthesis::{
        plan::{GlobalBlick, OwnerInput, TempoMapping},
        voice::{PreparedTree, Result, VoiceError, VoiceFamily},
    },
};

pub fn prepare_owner(
    input: &OwnerInput,
    lexicon: &LyricFrontend,
    duration_tree: &PreparedTree,
    family: VoiceFamily,
) -> Result<PreparedPhrase> {
    let mut clean=input.clone();
    clean.automation.retain(|key,_|!key.starts_with("dds")&&!key.starts_with("vocalModeComponent_")&&!key.starts_with("voiceBlend_")&&!matches!(key.as_str(),"growl_frequency"|"growl_amplitude"));
    if family==VoiceFamily::Sv1 && !input.experiments.settings.enabled {clean.automation.retain(|key,_|!key.starts_with("vocalMode_"));}
    for options in [&mut clean.voice.instance_options,&mut clean.voice.track_default_options] {
        if let Some(voice)=options.get_mut("voice").and_then(serde_json::Value::as_object_mut) {
            if family!=VoiceFamily::Sv2 {
                if family==VoiceFamily::Sv1 {voice.remove("vocalModeParams");}
                if !input.experiments.settings.active(input.experiments.settings.dds_controls) {
                    for spec in crate::synthesis::parameters::VOICE_CONTROLS {if voice.contains_key(spec.key) {voice.insert(spec.key.into(),serde_json::json!(0.0));}}
                }
            }
            if let Some(modes)=voice.get_mut("vocalModeParams").and_then(serde_json::Value::as_object_mut) {
                for mode in modes.values_mut() {
                    if let Some(value)=mode.as_f64() {*mode=serde_json::json!(value.clamp(0.0,150.0));}
                    else if let Some(lanes)=mode.as_object_mut() {for value in lanes.values_mut() {
                        if let Some(number)=value.as_f64() {*value=serde_json::json!(number.clamp(0.0,150.0));}
                    }}
                }
            }
            if input.voice.instance_options.get("rynthvExperiments")
                .and_then(|v|v.get("extended_expression")).and_then(serde_json::Value::as_bool)==Some(true) {
                let factor=input.experiments.settings.expression_factor();
                for spec in crate::synthesis::parameters::VOICE_CONTROLS {
                    if let Some(number)=voice.get(spec.key).and_then(serde_json::Value::as_f64) {
                        let value=(number as f32).clamp(-spec.limit*factor,spec.limit*factor);
                        voice.insert(spec.key.into(),serde_json::json!(value));
                    }
                }
            }
        }
    }
    let mut phrase = ExplicitScore::from_owner(&clean, lexicon, duration_tree, family)?
        .prepare(duration_tree, family, lexicon.default_language_key())?;
    phrase.pitch_controls = super::PreparedPitchControls::new(input)?;
    if input.experiments.settings.active(input.experiments.settings.component_curves) {
        clean.automation=input.automation.clone();
        clean.automation.retain(|key,_|!key.starts_with("dds")&&!key.starts_with("voiceBlend_")&&!matches!(key.as_str(),"growl_frequency"|"growl_amplitude"));
    }
    if input.experiments.settings.active(input.experiments.settings.growl) {
        super::PreparedAutomation::add_growl(input,&mut clean.automation)?;
    }
    phrase.automation = super::PreparedAutomation::new(&clean);
    phrase.performance_controls = super::PreparedPerformanceControls::new(input)?;
    phrase.voice_parameters=crate::synthesis::parameters::VoiceParameters::from_input(&clean.voice)?;
    phrase.experimental_dds_expression=family!=VoiceFamily::Sv2 && input.experiments.settings.active(input.experiments.settings.dds_controls);
    if input.experiments.settings.enabled {
        let (low,high)=input.experiments.settings.style_range();
        phrase.experimental_style_range=Some((low as f32/100.0,high as f32/100.0));
        if let Some(raw)=input.voice.instance_options.get("voice").and_then(|v|v.get("vocalModeParams")) {
            let mut modes:crate::model::PublicModeMap=serde_json::from_value(raw.clone()).map_err(|e|VoiceError::new(e.to_string()))?;
            for control in modes.values_mut() {
                for value in [&mut control.pitch,&mut control.timbre,&mut control.pronunciation] {
                    if let Some(value)=value {*value=value.clamp(low,high);}
                }
            }
            phrase.vocal_modes=Some(modes);
        }
    }
    phrase.phoneme_controls=super::phonemes::from_owner(input)?;
    phrase.unison=crate::synthesis::unison::UnisonSettings::from_input(&input.voice)?;

    if family == VoiceFamily::Sv2Compatible { phrase.unison = Default::default(); }
    Ok(phrase)
}

impl ExplicitScore {
    pub fn from_owner(
        input: &OwnerInput,
        lexicon: &LyricFrontend,
        duration_tree: &PreparedTree,
        family: VoiceFamily,
    ) -> Result<Self> {
        options::validate_owner(input, family)?;
        validate_tempo(&input.tempo)?;
        let language = lexicon.language(input)?;
        let transpose=crate::synthesis::parameters::transpose(&input.voice)?;
        let mut sources = Vec::new();
        for note in &input.notes {
            if options::muted(&note.attributes)? {
                continue;
            }
            options::validate_note(note, family)?;
            if note.duration <= 0 {
                return Err(VoiceError::new(format!(
                    "Note {} has non-positive duration",
                    note.id
                )));
            }
            let begin = note
                .onset
                .0
                .checked_add(input.content_offset.0)
                .ok_or_else(|| VoiceError::new("Global note onset overflows"))?;
            let end = begin
                .checked_add(note.duration)
                .ok_or_else(|| VoiceError::new("Global note end overflows"))?;
            sources.push((begin, end, note));
        }
        sources.sort_by_key(|(begin, _, note)| (*begin, note.id));
        if sources.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(VoiceError::new(
                "Overlapping notes within one synthesis owner are not supported",
            ));
        }
        let pronunciations = super::pronunciation::resolve_owner(
            &sources.iter().map(|(_,_,note)| *note).collect::<Vec<_>>(), language,
            input, lexicon, duration_tree, family)?;
        let (publication_begin, publication_end) = input.publication_bounds();
        let visible_begin_seconds = seconds_at(&input.tempo, publication_begin)?;
        let visible_end_seconds = publication_end
            .map(|end| seconds_at(&input.tempo, end))
            .transpose()?;
        let start_seconds = sources
            .first()
            .map(|(begin, _, _)| seconds_at(&input.tempo, GlobalBlick(*begin)))
            .transpose()?
            .unwrap_or(visible_begin_seconds);
        let mut score = Self {
            vocal_modes: crate::synthesis::vocal_modes::from_owner(input, family)?,
            start_seconds,
            visible_begin_seconds,
            visible_end_seconds,
            notes: Vec::new(),
        };
        let mut previous_time = if family == VoiceFamily::Sv1 {
            start_seconds
        } else {
            quantized_seconds(start_seconds)?
        };
        let mut previous_end_seconds = start_seconds;
        let mut previous_pitch = 60.0;
        for ((begin, end, note), pronunciation) in sources.into_iter().zip(pronunciations) {
            let super::pronunciation::ResolvedNote { language: note_language, placeholder_language, syllables, kind, error } = pronunciation;
            let begin_seconds = seconds_at(&input.tempo, GlobalBlick(begin))?;
            let end_seconds = seconds_at(&input.tempo, GlobalBlick(end))?;
            let begin_time = if family == VoiceFamily::Sv1 {begin_seconds} else {quantized_seconds(begin_seconds)?};
            let end_time = if family == VoiceFamily::Sv1 {end_seconds} else {quantized_seconds(end_seconds)?};
            let (duration_seconds, gap_seconds) = if family == VoiceFamily::Sv1 {
                (
                    end_seconds - begin_seconds,
                    begin_seconds - previous_end_seconds,
                )
            } else {
                (
                    end_time - begin_time,
                    begin_time - previous_time,
                )
            };
            if end_time <= begin_time {
                return Err(VoiceError::new(format!(
                    "Note {} is shorter than one model frame",
                    note.id
                )));
            }
            let pitch = note
                .pitch
                .checked_add(input.pitch_offset)
                .filter(|pitch| (0..=127).contains(pitch))
                .ok_or_else(|| {
                    VoiceError::new(format!("Note {} pitch is outside MIDI range", note.id))
                })? as f32 + transpose + note.extensions.get("detune").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32/100.0;
            if !pitch.is_finite() || !(0.0..=127.0).contains(&pitch) {
                return Err(VoiceError::new(format!("Note {} transposed pitch is outside MIDI range",note.id)));
            }
            let performance = crate::model::NotePerformance::from_parts(
                &note.musical_type,
                &note.attributes,
            ).map_err(VoiceError::new)?;
            if gap_seconds > 0.0 {

                score.notes.push(ScoreNote {
                    source_note_id: None,
                    note_error: None,
                    retake_seeds: Default::default(),

                    performance: crate::model::NotePerformance {
                        mode: if family == VoiceFamily::Sv2 {performance.mode} else {Default::default()},
                        choir_pitch: performance.choir_pitch,
                        ..Default::default()
                    },
                    duration_seconds: gap_seconds,

                    pitch: if family != VoiceFamily::Sv1 {pitch} else {previous_pitch},
                    phones: vec![ScorePhone {
                        language: language.id().to_owned(),
                        token: "sil".to_owned(),
                        display_token: None,
                        onset_offset_seconds: None,
                    }],
                    kind: 1,
                    flag: family == VoiceFamily::Sv1,
                    tag: None,
                    parts: 1,
                    part_index: 0,
                });
            }
            let mut syllables = super::model_phones::materialize(syllables, note_language, lexicon, duration_tree,
                note.attributes.get("evenSyllableDuration").and_then(serde_json::Value::as_bool) != Some(false))?;
            if placeholder_language.is_some() {
                for phone in syllables.iter_mut().flatten() {
                    phone.language = lexicon.default_language_key().to_owned();
                }
            }
            let parts = u32::try_from(syllables.len())
                .map_err(|_| VoiceError::new("Too many syllables in one note"))?;
            if parts == 0 || (kind != 2 && syllables.iter().any(Vec::is_empty)) {
                return Err(VoiceError::new(format!("Note {} has no phonemes", note.id)));
            }
            for (part_index, tokens) in syllables.into_iter().enumerate() {
                score.notes.push(ScoreNote {
                    source_note_id: Some(note.id),
                    note_error: error.clone(),
                    retake_seeds: Some(super::retakes::note_seeds(note, family)?),
                    performance,
                    duration_seconds,
                    pitch,
                    phones: tokens,
                    kind,

                    flag: family == VoiceFamily::Sv1
                        && note
                            .attributes
                            .get("evenSyllableDuration")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true),
                    tag: note
                        .extensions
                        .get("accent")
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                    parts,
                    part_index: part_index as u32,
                });
            }
            previous_time = end_time;
            previous_end_seconds = end_seconds;
            previous_pitch = pitch;
        }

        Ok(score)
    }
}

fn quantized_seconds(seconds: f64) -> Result<f64> {
    let frames = seconds / FRAME_SECONDS;
    if !frames.is_finite() || frames.abs() >= u32::MAX as f64 {
        return Err(VoiceError::new(
            "Score time exceeds the supported model frame range",
        ));
    }

    Ok(frames.round() * FRAME_SECONDS)
}

fn validate_tempo(mapping: &TempoMapping) -> Result<()> {
    if !mapping.origin_seconds.is_finite()
        || mapping
            .points
            .first()
            .is_none_or(|point| point.position.0 != 0)
        || mapping
            .points
            .iter()
            .any(|point| !point.bpm.is_finite() || point.bpm <= 0.0)
        || mapping
            .points
            .windows(2)
            .any(|pair| pair[0].position.0 >= pair[1].position.0)
    {
        return Err(VoiceError::new("Invalid project tempo mapping"));
    }
    Ok(())
}

fn seconds_at(mapping: &TempoMapping, position: GlobalBlick) -> Result<f64> {
    let mut seconds = mapping.origin_seconds;
    if position.0 < 0 {
        seconds += position.0 as f64 / QUARTER as f64 * 60.0 / mapping.points[0].bpm;
    } else {
        for (index, point) in mapping.points.iter().enumerate() {
            if position.0 <= point.position.0 {
                break;
            }
            let end = mapping
                .points
                .get(index + 1)
                .map_or(position.0, |next| next.position.0.min(position.0));
            let distance = i128::from(end) - i128::from(point.position.0);
            seconds += distance as f64 / QUARTER as f64 * 60.0 / point.bpm;
        }
    }
    if !seconds.is_finite() {
        return Err(VoiceError::new("Project time mapping is non-finite"));
    }
    Ok(seconds)
}
