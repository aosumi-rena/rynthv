use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum VoiceFamily {
    R1Std,
    R2Std,
    Sv1,
    Sv2Compatible,
    Sv2,
}

impl VoiceFamily {
    pub fn model_version(self) -> u8 {
        match self {
            Self::R1Std => 1,
            Self::R2Std => 2,
            Self::Sv1 | Self::Sv2Compatible => 1,
            Self::Sv2 => 2,
        }
    }

    pub fn requires_identity(self) -> bool {
        matches!(self, Self::Sv2Compatible | Self::Sv2)
    }

    pub fn is_standard(self) -> bool { matches!(self, Self::R1Std | Self::R2Std) }
}
