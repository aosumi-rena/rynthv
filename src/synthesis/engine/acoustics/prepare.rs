use super::{PreparedRenderAcoustics, StageTimings};
use crate::synthesis::{
    engine::{PreparedAcoustics, PreparedModelFeatures, RenderStage, models},
    score::PreparedPhrase,
    stages::{
        compatible_duration::CompatibleDurationStage,
        duration::DurationStage,
        f0::{F0Stage, PitchInputControls, TimedPhone},
        sv1_duration::Sv1DurationStage,
        timbre::TimbreNetwork,
    },
    voice::{PreparedVoice, Result, VoiceError, VoiceFamily},
};
use std::time::Instant;

pub fn prepare_acoustics(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    checkpoint: impl FnMut(RenderStage) -> Result<()>,
 ) -> Result<PreparedRenderAcoustics> {
    prepare_acoustics_with_host(voice, duration_identity, phrase, checkpoint, None)
}

pub(super) fn prepare_acoustics_with_host(
    voice: &PreparedVoice,
    duration_identity: Option<u64>,
    phrase: &PreparedPhrase,
    mut checkpoint: impl FnMut(RenderStage) -> Result<()>,
    host_pitch: Option<&mut crate::synthesis::engine::pitch::HostPitchSampler<'_>>,
) -> Result<PreparedRenderAcoustics> {
    if phrase.records.is_empty()
        || phrase.records.len() != phrase.notes.len()
        || phrase.retake_seeds.len() != phrase.notes.len()
    {
        return Err(VoiceError::new(
            "Synthesis requires matching nonempty note records",
        ));
    }
    let mut prepared_phrase = phrase.clone();
    prepared_phrase.mode_coefficients = crate::synthesis::vocal_modes::for_phrase(voice,phrase)?;
    let duration_seeds: Vec<_> = phrase
        .retake_seeds
        .iter()
        .map(|seeds| seeds.unwrap_or_default().duration)
        .collect();
    let pitch_seeds: Vec<_> = phrase
        .retake_seeds
        .iter()
        .map(|seeds| seeds.map(|seeds| seeds.pitch))
        .collect();
    let dds_pitch_seeds: Vec<_> = pitch_seeds.iter().map(|seed| seed.unwrap_or(0)).collect();
    checkpoint(RenderStage::Duration)?;
    let started = Instant::now();
    #[cfg(feature = "synth-profile")]
    let profile = crate::synthesis::profiling::span("stage.duration");
    let mut duration = match voice.family {
        VoiceFamily::Sv1 => {
            if duration_identity.is_some() {
                return Err(VoiceError::new(
                    "SV1 duration does not accept an identity condition",
                ));
            }
            let native = Sv1DurationStage::prepare(voice.model("duration")?)?
                .predict_explicit(&phrase.records)?;
            if native.record_seconds.len() != prepared_phrase.notes.len() {
                return Err(VoiceError::new(
                    "SV1 derived note durations differ from the phrase",
                ));
            }

            for (note, seconds) in prepared_phrase.notes.iter_mut().zip(native.record_seconds) {
                note.duration_seconds = seconds;
                note.parts = 1;
                note.part_index = 0;
                note.appended_durations.clear();
            }
            native.result
        }
        VoiceFamily::Sv2Compatible => CompatibleDurationStage::prepare(
            voice.model("duration")?,
            models::required_identity(duration_identity)?,
        )?
        .predict_explicit(&phrase.records)?,
        VoiceFamily::Sv2 => DurationStage::prepare(
            voice.model("duration")?,
            models::required_identity(duration_identity)?,
            voice.speaker,
        )?
        .predict_explicit_seeded(&phrase.records, &duration_seeds)?,
    };
    if voice.family==VoiceFamily::Sv2 {
        if prepared_phrase.voice_parameters.consonant_duration!=0.0 {
            duration.durations=crate::synthesis::stages::duration::frontend::postprocess_controlled(
                voice.model("duration")?,&prepared_phrase.records,&duration.prediction,
                prepared_phrase.voice_parameters.consonant_duration)?;
        }
        crate::synthesis::score::phonemes::apply(&prepared_phrase,&mut duration)?;
    }
    if duration
        .prediction
        .phones
        .iter()
        .any(|phone| phone.used_silence_fallback)
    {
        return Err(VoiceError::new(
            "A requested phoneme is absent from the duration model",
        ));
    }
    let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
    #[cfg(feature = "synth-profile")]
    drop(profile);
    let phones: Vec<_> = duration
        .durations
        .iter()
        .map(|phone| {
            let record = &phrase.records[phone.record_index];
            let source = &record.phones[phone.phone_index];
            TimedPhone {
                note_index: phone.record_index,
                language: source.language.clone(),
                token: source.token.clone(),
                frames: phone.frames,
            }
        })
        .collect();
    let mut note_frames = vec![0_usize; phrase.notes.len()];
    for phone in &phones {
        note_frames[phone.note_index] = note_frames[phone.note_index]
            .checked_add(phone.frames)
            .ok_or_else(|| VoiceError::new("Synthesis frame count overflow"))?;
    }
    if note_frames.iter().sum::<usize>() < 4 {
        return Err(VoiceError::new(
            "A synthesis phrase must contain at least four 5 ms frames",
        ));
    }
    checkpoint(RenderStage::Pitch)?;
    let started = Instant::now();
    #[cfg(feature = "synth-profile")]
    let profile = crate::synthesis::profiling::span("stage.pitch");
    let (mut pitch, pitch_features) = match voice.family {
        VoiceFamily::Sv1 => {
            let seconds = duration
                .durations
                .iter()
                .map(|phone| phone.duration_seconds)
                .collect::<Vec<_>>();
            let stage = models::dds_pitch(voice)?;
            let expressiveness = crate::synthesis::engine::pitch::expressiveness(
                &prepared_phrase,
                stage.frame_seconds(),
                stage.note_frames(&prepared_phrase.notes)?.iter().sum(),
            )?;
            (
                stage.predict_native(
                    &prepared_phrase.notes,
                    &phones,
                    &seconds,
                    &dds_pitch_seeds,
                    expressiveness.as_deref(),
                )?,
                None,
            )
        }
        VoiceFamily::Sv2Compatible => {
            let stage = models::dds_pitch(voice)?;
            let expressiveness = crate::synthesis::engine::pitch::expressiveness(
                phrase,
                stage.frame_seconds(),
                stage.note_frames(&phrase.notes)?.iter().sum(),
            )?;
            (
                stage.predict_singing(
                    &phrase.notes,
                    &phones,
                    &dds_pitch_seeds,
                    expressiveness.as_deref(),
                )?,
                None,
            )
        }
        VoiceFamily::Sv2 => {
            let stage = F0Stage::prepare(voice.model("f0")?, voice.speaker)?;
            let features = stage.prepare_features(&phrase.notes, &phones)?;
            let controls = crate::synthesis::engine::pitch::sv2_controls(phrase)?;
            let expressiveness = crate::synthesis::engine::pitch::expressiveness(
                phrase,
                crate::synthesis::score::FRAME_SECONDS,
                note_frames.iter().sum(),
            )?;
            let pitch = stage.predict_singing_prepared(
                &phrase.notes,
                &phones,
                &features,
                PitchInputControls {
                    vocal_modes: prepared_phrase.mode_coefficients.as_ref(),
                    pitch_seeds: Some(&pitch_seeds),
                    absolute_pitch: controls.as_deref(),
                    expressiveness: expressiveness.as_deref(),
                },
                None,
            )?;
            (pitch, Some(features))
        }
    };
    if voice.family != VoiceFamily::Sv2 {
        crate::synthesis::engine::pitch::apply_dds_curves(&prepared_phrase, &mut pitch)?;
    }
    if let Some(sampler) = host_pitch {
        crate::synthesis::engine::pitch::apply_host_dds_pitch(&prepared_phrase, &mut pitch, sampler)?;
    }
    crate::synthesis::engine::pitch::apply_deviation(&prepared_phrase,&mut pitch);
    crate::synthesis::engine::pitch::apply_growl(&prepared_phrase,&mut pitch)?;
    let pitch_ms = started.elapsed().as_secs_f64() * 1000.0;
    #[cfg(feature = "synth-profile")]
    drop(profile);

    checkpoint(RenderStage::Timbre)?;
    let started = Instant::now();
    #[cfg(feature = "synth-profile")]
    let _profile = crate::synthesis::profiling::span("stage.timbre");
    let features = match pitch_features {
        Some(pitch) => PreparedModelFeatures::Sv2 {
            pitch,
            timbre: TimbreNetwork::prepare(voice.model("timbre")?, voice.speaker, voice.retained_acoustic_noise_only)?
                .prepare_features(&phrase.notes, &phones)?,
        },
        None => PreparedModelFeatures::Sv1Local,
    };
    Ok(PreparedRenderAcoustics {
        family: voice.family,
        duration,
        pitch,
        timings: StageTimings {
            duration_ms,
            pitch_ms,
            timbre_ms: started.elapsed().as_secs_f64() * 1000.0,
            ..StageTimings::default()
        },
        prepared: PreparedAcoustics {
            phrase: prepared_phrase,
            phones,
            features,
        },
    })
}
