pub mod equalizer;
pub mod compressor;
pub mod reverb;
mod configuration;
mod floating_point;
pub mod voice;
pub mod room;
pub use configuration::Configuration;

pub struct Processor {
    equalizer: Vec<equalizer::Filter>,
    compressor: Option<compressor::Compressor>,
    reverb: Option<reverb::Reverb>,
}

impl Processor {
    pub fn new(configuration: &Configuration) -> Self {
        let mut filters = configuration.equalizer.map(Vec::from).unwrap_or_default();
        filters.sort_by(|a,b| (a.gain >= 1.0).cmp(&(b.gain >= 1.0))
            .then_with(|| a.quality().total_cmp(&b.quality())));
        Self { equalizer: filters.into_iter().map(|p| equalizer::Filter::new(p,48000)).collect(),
            compressor: configuration.compressor.map(|p| compressor::Compressor::new(p,48000)),
            reverb: configuration.reverb.map(reverb::Reverb::new) }
    }

    pub fn process(&mut self, frames: &mut [[f32;4]]) {
        for filter in &mut self.equalizer { filter.process(frames); }
        if let Some(compressor) = &mut self.compressor { compressor.process(frames); }
        if let Some(reverb) = &mut self.reverb { reverb.process(frames); }
    }

    fn input_level(&self) -> f32 {
        self.compressor.as_ref().map_or(-50.0, compressor::Compressor::input_level)
    }

    fn set_threshold(&mut self, threshold: f32) {
        if let Some(compressor) = &mut self.compressor { compressor.set_threshold(threshold); }
    }
}
