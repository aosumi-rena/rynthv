use crate::synthesis::{
    inference::Sequence,
    voice::{PreparedTree, Result, VoiceError},
};

use super::filter_kernel::DynamicFilter;

pub const SPECTRAL_FILTER_TYPE_ID: [u8; 16] = [
    0x8b, 0xc8, 0x4f, 0xda, 0x18, 0xb0, 0x19, 0xb5, 0x57, 0xef, 0x3c, 0x9c, 0x38, 0x28, 0x88, 0xf1,
];

#[derive(Clone, Copy, Debug)]
pub struct SpectralFilterConfig {

    pub window_frames: usize,

    pub serialized_dimensions: [usize; 2],
}

impl SpectralFilterConfig {
    pub fn prepare(tree: &PreparedTree, node: usize) -> Result<Self> {
        let module = tree.node(node)?;
        if module.type_id != SPECTRAL_FILTER_TYPE_ID || !module.children.is_empty() {
            return Err(VoiceError::new("Expected a leaf vocoder spectral filter"));
        }
        Self::parse(&tree.payload(node)?)
    }

    pub fn parse(payload: &[u8]) -> Result<Self> {
        if payload.len() != 13 || payload[4] & 1 != 0 {
            return Err(VoiceError::new(
                "Unsupported vocoder spectral filter configuration",
            ));
        }
        let read = |offset: usize| {
            i32::from_le_bytes(
                payload[offset..offset + 4]
                    .try_into()
                    .expect("checked payload"),
            )
        };
        let dimensions = [read(0), read(5), read(9)];
        if dimensions.iter().any(|&dimension| dimension <= 0) {
            return Err(VoiceError::new(
                "Vocoder spectral filter dimensions must be positive",
            ));
        }
        Ok(Self {
            window_frames: dimensions[0] as usize,
            serialized_dimensions: [dimensions[1] as usize, dimensions[2] as usize],
        })
    }

    pub fn window_samples(self, frame_samples: usize) -> Result<usize> {
        frame_samples
            .checked_mul(self.window_frames)
            .filter(|&samples| samples > 1 && samples <= i32::MAX as usize)
            .ok_or_else(|| VoiceError::new("Vocoder analysis window size is invalid"))
    }

    pub fn fft_size(self, frame_samples: usize) -> Result<usize> {
        (self.window_samples(frame_samples)? - 1)
            .checked_next_power_of_two()
            .and_then(|samples| samples.checked_mul(2))
            .filter(|&samples| samples <= i32::MAX as usize)
            .ok_or_else(|| VoiceError::new("Vocoder FFT size overflows"))
    }
}

#[derive(Debug)]
pub struct SpectralFilterStream {
    filter: DynamicFilter,
}

impl SpectralFilterStream {
    pub(super) fn with_geometry(frame_samples: usize, window_samples: usize, fft_size: usize) -> Result<Self> {
        Ok(Self { filter: DynamicFilter::new(frame_samples, window_samples, fft_size)? })
    }

    pub(crate) fn process_independent(&mut self,parameters:&Sequence,excitation:&Sequence,end:bool)->Result<Sequence> {
        self.filter.process_independent(parameters,excitation,end)
    }
    pub(crate) fn set_noise(&mut self) {self.filter.set_noise();}
    pub(crate) fn push_effects(&mut self,frames:&[Option<super::effects::FrameEffect>]) {self.filter.push_effects(frames);}
    pub fn new(config: SpectralFilterConfig, frame_samples: usize) -> Result<Self> {
        Ok(Self {
            filter: DynamicFilter::new(
                frame_samples,
                config.window_samples(frame_samples)?,
                config.fft_size(frame_samples)?,
            )?,
        })
    }

    pub fn process(
        &mut self,
        parameters: &Sequence,
        excitation: &Sequence,
        final_batch: bool,
    ) -> Result<Sequence> {
        self.filter.process(parameters, excitation, final_batch)
    }
}
