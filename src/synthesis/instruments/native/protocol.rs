use super::{Result, archive::Archive, id, render::PROGRAMS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Protocol {
    Initial,
    ExplicitVibrato,
}

impl Protocol {
    pub fn from_main(archive: &Archive) -> Result<Self> {
        let nodes = archive.program(id(PROGRAMS[4]))?;
        let width = nodes.iter().find(|node| node.tag == 200)
            .and_then(|node| node.shape.last()).copied()
            .ok_or("Instrument pitch programme has no input 200")?;
        match width {
            522 => Ok(Self::Initial),
            523 => Ok(Self::ExplicitVibrato),
            _ => Err(format!("Unsupported instrument pitch input width: {width}")),
        }
    }

    pub fn explicit_vibrato(self) -> bool { self == Self::ExplicitVibrato }
    pub fn waveform(archive: &Archive) -> Result<Self> {
        let nodes = archive.program(id(super::render::WAVE_PROGRAM))?;
        let width = nodes.iter().find(|node| node.tag == 900)
            .and_then(|node| node.shape.last()).copied()
            .ok_or("Instrument waveform programme has no input 900")?;
        match width {
            83 => Ok(Self::Initial), 148 => Ok(Self::ExplicitVibrato),
            _ => Err(format!("Unsupported instrument waveform input width: {width}")),
        }
    }
}
