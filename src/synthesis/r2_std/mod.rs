// SPDX-License-Identifier: MIT

mod api;
pub(crate) use api::RenderProgress;
pub(crate) mod automatic_f0;
mod archive;
mod bank;
mod codec;
mod decoder;
mod directional;
mod directional_model;
mod frames;
mod japanese_dictionary;
mod lf;
mod lyric_assignment;
mod noise_filter_tables;
mod noise_tables;
mod noise_synthesis;
mod overlap;
mod overlap_correction;
mod overlap_fade;
mod phonetics;
mod p3;
mod pitch_transform;
mod pitch_baseline;
mod preparation;
mod preview;
mod residual;
mod retaining;
mod release_modifier;
mod release_spectrum;
mod score;
mod selection;
mod selection_refinement;
mod source_adaptation;
mod source_coordinates;
mod source_frames;
mod source_phase;
mod spectrum;
mod stored_vt;
mod target_rules;
mod thumbnail;
mod timing;
mod transforms;
mod unit_cost;
mod warp;
mod wave_placement;

pub(crate) use api::{
    ControlProvider, ControlSample, NativeRenderer, PitchMode, RealizedPhone, RenderedPhrase,
    Resources, NoteFailure,
};
pub(crate) use bank::{BankProfile, SourceUnit, StdBank, TimelinePhone};
pub(crate) use decoder::{DecodedFrame, SourceDecoder};
pub(crate) use frames::{AcousticFrame, StoredFrame, StoredFrames};
pub(crate) use score::{Note, Phrase, Tempo};
pub(crate) use unit_cost::UnitCostModel;

#[derive(Debug)]
pub(crate) enum Error {
    Io(std::io::Error),
    Format(&'static str),
    Unsupported(&'static str),
    Cancelled,
    Text(String),
    Archive(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "R2 STD: {error}"),
            Self::Format(reason) => write!(f, "R2 STD invalid input: {reason}"),
            Self::Unsupported(reason) => write!(f, "R2 STD unsupported: {reason}"),
            Self::Cancelled => f.write_str("R2 STD synthesis cancelled"),
            Self::Text(reason) => write!(f, "R2 STD text preparation: {reason}"),
            Self::Archive(reason) => write!(f, "R2 STD archive: {reason}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
pub(crate) type Result<T> = std::result::Result<T, Error>;
