// SPDX-License-Identifier: MIT
use super::{Error, Result, SourceDecoder, StdBank, UnitCostModel};
use std::path::PathBuf;

pub(crate) struct Resources {
    pub bank_path: PathBuf,
    pub clf_directory: PathBuf,
    pub shared_f0_path: PathBuf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PitchMode {
    NativeDefault,
    SharedAutomatic,
    HostComplete,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ControlSample {
    pub midi_pitch: Option<f64>,
    pub pitch_deviation_semitones: f64,
    pub loudness_db: f64,
    pub tension: f64,
    pub breathiness: f64,
    pub voicing: f64,
    pub gender: f64,
    pub tone_shift_cents: f64,
}
impl ControlSample {
    pub fn validate(&self, mode: PitchMode) -> Result<()> {
        if [
            self.pitch_deviation_semitones,
            self.loudness_db,
            self.tension,
            self.breathiness,
            self.voicing,
            self.gender,
            self.tone_shift_cents,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || self.midi_pitch.is_some_and(|v| !v.is_finite() || v <= 0.0)
            || mode == PitchMode::HostComplete && self.midi_pitch.is_none()
        {
            return Err(Error::Format(
                "finite controls and explicit complete-host MIDI required",
            ));
        }
        if !(-48.0..=24.0).contains(&self.loudness_db)
            || !(-2.0..=2.0).contains(&self.tension)
            || !(-2.0..=2.0).contains(&self.breathiness)
            || !(0.0..=1.0).contains(&self.voicing)
            || !(-2.0..=2.0).contains(&self.gender)
            || !(-800.0..=800.0).contains(&self.tone_shift_cents)
        {
            return Err(Error::Unsupported("native control range"));
        }
        Ok(())
    }
}
pub(crate) trait ControlProvider: Send + Sync {
    fn pitch_mode(&self) -> PitchMode;
    fn sample(&self, global_seconds: f64) -> Result<ControlSample>;
}
pub(crate) struct RealizedPhone {
    pub note_index: Option<usize>,
    pub token_index: Option<usize>,
    pub symbol: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}
pub(crate) enum RenderProgress<'a> {
    Stage(u8),
    Prepared {origin:f64,hop:f64,phones:&'a [RealizedPhone],failures:&'a [NoteFailure]},
    Pcm {origin:f64,samples:&'a [f32],pitch_origin:f64,hop:f64,pitch:&'a [f32]},
}
#[derive(Clone, Debug)]
pub(crate) struct NoteFailure {
    pub note_index: usize,
    pub message: String,
}
pub(crate) struct RenderedPhrase {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub origin_seconds: f64,
    pub pitch_origin_seconds: f64,
    pub pitch_hop_seconds: f64,
    pub generated_pitch_hz: Vec<f32>,
    pub pitch_hz: Vec<f32>,
    pub phones: Vec<RealizedPhone>,
    pub note_failures: Vec<NoteFailure>,
    pub accuracy_notes: Vec<&'static str>,
}

pub(crate) struct NativeRenderer {
    pub(super) bank: StdBank,
    pub(super) unit_cost: UnitCostModel,
    pub(super) decoder: SourceDecoder,
    pub(super) resources: Resources,
    pub(super) selector: super::selection::Selector,
    pub(super) phonetics: super::phonetics::Phonetics,
    pub(super) random_state: u32,
    pub(super) pitch_transform: super::pitch_transform::PitchTransform,
    pub(super) residual: super::residual::PsdResidual,
    pub(super) directions: Option<super::directional::Directions>,
    pub(super) noise: Option<super::noise_tables::NoiseTables>,
    pub(super) automatic: Option<super::automatic_f0::AutomaticF0>,
    pub(super) release: super::release_modifier::ReleaseModifier,
}
impl NativeRenderer {
    pub fn open(resources: Resources) -> Result<Self> {
        if [
            &resources.bank_path,
            &resources.clf_directory,
            &resources.shared_f0_path,
        ]
        .iter()
        .any(|path| !path.is_absolute())
        {
            return Err(Error::Format(
                "explicit absolute STD resource paths required",
            ));
        }
        let bank = StdBank::open(&resources.bank_path)?;
        let unit_cost = UnitCostModel::open(&bank, &resources.clf_directory)?;
        let decoder = SourceDecoder::new(&bank)?;
        let selector = super::selection::Selector::new(&bank)?;
        let phonetics =
            super::phonetics::Phonetics::open(&resources.clf_directory, bank.profile.encoder_only)?;
        let pitch_transform = super::pitch_transform::PitchTransform::open(&bank)?;
        let residual = super::residual::PsdResidual::open(&bank)?;
        let directions = super::directional::Directions::open(&bank)?;
        let release = super::release_modifier::ReleaseModifier::open(&bank, decoder.nyquist)?;
        Ok(Self {
            bank,
            unit_cost,
            decoder,
            resources,
            selector,
            phonetics,
            random_state: 1,
            pitch_transform,
            residual,
            directions,
            noise: None,
            automatic: None,
            release,
        })
    }
    pub fn profile(&self) -> &super::BankProfile {
        &self.bank.profile
    }
}
