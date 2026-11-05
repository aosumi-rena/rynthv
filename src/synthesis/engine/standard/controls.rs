use crate::synthesis::{parameters::VoiceParameters,plan::OwnerInput,score::{PreparedAutomation,PreparedPitchControls},voice::{Result,VoiceError}};

pub(super) struct Controls {
    pub parameters:VoiceParameters,
    pub automation:Option<PreparedAutomation>,
    pub pitch:Option<PreparedPitchControls>,
    pub transpose:f32,
}
impl Controls {
    pub fn new(input:&OwnerInput)->Result<Self> {
        if input.pitch_controls.iter().any(|control|matches!(control,crate::model::pitch::PitchControl::Point(_))) {
            return Err(VoiceError::new("STD synthesis currently supports drawn pitch curves; neural pitch control points are unavailable"));
        }
        for curve in input.automation.values(){curve.validate().map_err(VoiceError::new)?;}
        Ok(Self {parameters:VoiceParameters::from_input(&input.voice)?,automation:PreparedAutomation::new(input),
            pitch:PreparedPitchControls::new(input)?,transpose:crate::synthesis::parameters::transpose(&input.voice)?})
    }
    pub fn value(&self,key:&str,seconds:f64,default:f32)->f32 {self.automation.as_ref().map_or(default,|curve|curve.value(key,seconds,default))}
    pub fn loudness(&self,seconds:f64)->f32 {(self.parameters.loudness+self.value("loudness",seconds,0.0)).clamp(-48.0,24.0)}
    // Bound the final native control, after the base value and interpolated
    // automation are combined. Cubic curves may overshoot their stored points.
    pub fn expression(&self,key:&str,seconds:f64,constant:f32,limit:f64)->f64 {
        (f64::from(constant)+f64::from(self.value(key,seconds,0.0))).clamp(-limit,limit)
    }
}
