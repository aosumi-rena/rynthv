use crate::synthesis::{
    engine::{PreparedRenderAcoustics, models},
    stages::{
        sv1_timbre::{PreparedSv1TimbreFrames, Sv1TimbreNetwork, Sv1TimbreStream},
        timbre::{
            PreparedTimbreFrames, TimbreNetwork, TimbreOutput, TimbreRequest, TimbreStream,
            records::RecordsConfig,
        },
    },
    voice::{PreparedVoice, Result, VoiceError, VoiceFamily},
};
use std::sync::Arc;

pub enum Network<'tree> {
    Choir,
    Dds(Box<Sv1TimbreNetwork<'tree>>),
    Sv2(Box<TimbreNetwork<'tree>>),
}

pub(super) enum PreparedFrames {
    Dds(Arc<PreparedSv1TimbreFrames>),
    Sv2(Arc<PreparedTimbreFrames>),
}

pub(super) enum Stream<'network, 'tree> {
    Dds(Box<Sv1TimbreStream<'network, 'tree>>),
    Sv2(Box<TimbreStream<'network, 'tree>>),
}

impl<'tree> Network<'tree> {
    pub fn prepare(voice: &'tree PreparedVoice) -> Result<Self> {
        if !voice.speakers.is_empty() {
            return Ok(Self::Choir);
        }
        let tree = voice.model("timbre")?;
        match voice.family {
            VoiceFamily::Sv1 | VoiceFamily::Sv2Compatible => {
                Ok(Self::Dds(Box::new(models::dds_timbre(voice)?)))
            }
            VoiceFamily::Sv2 => Ok(Self::Sv2(Box::new(TimbreNetwork::prepare(
                tree,
                voice.speaker,
                voice.retained_acoustic_noise_only,
            )?))),
        }
    }

    pub(super) fn prepare_frames(
        &self,
        acoustic: &PreparedRenderAcoustics,
    ) -> Result<PreparedFrames> {
        let prepared = &acoustic.prepared;
        match self {
            Self::Choir => Err(VoiceError::new(
                "Choir PCM is rendered by its member ensemble program",
            )),
            Self::Dds(network) => {
                let modulation = acoustic.modulation();
                let acoustic_pitch = acoustic.acoustic_pitch()?;
                let frames = network.prepare_frame_features(
                    &TimbreRequest { dds:None,
                        mode_coefficients: prepared.phrase.mode_coefficients.as_ref(),
                        notes: &prepared.phrase.notes,
                        phones: &prepared.phones,
                        midi_pitch: &acoustic_pitch,
                        note_modulation: &modulation,
                        phone_impulses: None,
                        frame_controls: None,
                        requested_frame: 0,
                    },
                    &acoustic.pitch,
                )?;
                Ok(PreparedFrames::Dds(Arc::new(frames)))
            }
            Self::Sv2(network) => network
                .prepare_frame_features_with_modes(
                    &prepared.phrase.notes,
                    &prepared.phones,
                    prepared.sv2_features()?.1,
                    prepared.phrase.mode_coefficients.as_ref(),
                )
                .map(|frames| PreparedFrames::Sv2(Arc::new(frames))),
        }
    }

    pub(super) fn start<'network>(
        &'network self,
        acoustic: &PreparedRenderAcoustics,
        frames: &PreparedFrames,
        requested_frame: usize,
    ) -> Result<Stream<'network, 'tree>> {
        let prepared = &acoustic.prepared;
        let modulation = acoustic.modulation();
        let impulses = acoustic.impulses();
        let frame_controls=acoustic.frame_controls();
        let acoustic_pitch=acoustic.acoustic_pitch()?;
        let request = TimbreRequest { dds:None,
            mode_coefficients: prepared.phrase.mode_coefficients.as_ref(),
            notes: &prepared.phrase.notes,
            phones: &prepared.phones,
            midi_pitch: &acoustic_pitch,
            note_modulation: &modulation,
            phone_impulses: None,
            frame_controls: frame_controls.as_deref(),
            requested_frame,
        };
        match (self, frames) {
            (Self::Dds(network), PreparedFrames::Dds(frames)) => network
                .start_prepared(request, Arc::clone(frames))
                .map(|stream| Stream::Dds(Box::new(stream))),
            (Self::Sv2(network), PreparedFrames::Sv2(frames)) => network
                .start_prepared(
                    &TimbreRequest { dds:None,
                        phone_impulses: Some(&impulses),
                        ..request
                    },
                    &RecordsConfig::default(),
                    Arc::clone(frames),
                )
                .map(|stream| Stream::Sv2(Box::new(stream))),
            _ => Err(VoiceError::new(
                "Progressive frame conditions belong to another model family",
            )),
        }
    }
}

impl Stream<'_, '_> {
    pub fn first_frame(&self) -> usize {
        match self {
            Self::Dds(stream) => stream.first_frame(),
            Self::Sv2(stream) => stream.first_frame(),
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            Self::Dds(stream) => stream.is_finished(),
            Self::Sv2(stream) => stream.is_finished(),
        }
    }

    pub fn next(&mut self, max_frames: usize) -> Result<TimbreOutput> {
        match self {
            Self::Dds(stream) => stream.next(max_frames),
            Self::Sv2(stream) => stream.next(max_frames),
        }
    }
}
