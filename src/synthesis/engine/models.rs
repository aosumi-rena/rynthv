use crate::synthesis::{
    rendered::PitchTrack,
    score::PreparedPhrase,
    stages::{f0::PitchResult, sv1_f0::Sv1F0Stage, sv1_timbre::Sv1TimbreNetwork},
    voice::{PreparedVoice, Result, VoiceError, VoiceFamily},
};

pub(super) fn required_identity(identity: Option<u64>) -> Result<u64> {
    identity.ok_or_else(|| VoiceError::new("This voice requires its model identity condition"))
}

pub(super) fn dds_pitch(voice: &PreparedVoice) -> Result<Sv1F0Stage<'_>> {
    let tree = voice.model("f0")?;
    match voice.family {
        VoiceFamily::Sv1 => Sv1F0Stage::prepare_native(
            tree,
            voice.f0_enhancement.ok_or_else(|| {
                VoiceError::new("SV1 voice lacks its native F0 enhancement configuration")
            })?,
        ),
        VoiceFamily::Sv2Compatible => Sv1F0Stage::prepare(tree),
        VoiceFamily::Sv2 => Err(VoiceError::new(
            "SV2 voice does not use the DDS pitch program",
        )),
    }
}

pub(super) fn dds_timbre(voice: &PreparedVoice) -> Result<Sv1TimbreNetwork<'_>> {
    let tree = voice.model("timbre")?;
    match voice.family {
        VoiceFamily::Sv1 => Sv1TimbreNetwork::prepare_native_with_language_precision(
            tree, voice.dds_half_precision_language),
        VoiceFamily::Sv2Compatible => Sv1TimbreNetwork::prepare(tree),
        VoiceFamily::Sv2 => Err(VoiceError::new(
            "SV2 voice does not use the DDS acoustic program",
        )),
    }
}

pub(super) fn pitch_track(
    voice: &PreparedVoice,
    phrase: &PreparedPhrase,
    prediction: PitchResult,
) -> Result<PitchTrack> {
    match voice.family {
        VoiceFamily::Sv1 | VoiceFamily::Sv2Compatible => {
            let frames = dds_pitch(voice)?.note_frames(&phrase.notes)?;
            PitchTrack::from_phrase_with_frames(phrase, prediction, &frames)
        }
        VoiceFamily::Sv2 => PitchTrack::from_phrase(phrase, prediction),
    }
}
