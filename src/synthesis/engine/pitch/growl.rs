use crate::synthesis::{score::PreparedPhrase,stages::f0::PitchResult,voice::{Result,VoiceError}};

pub(in crate::synthesis::engine) fn apply(phrase:&PreparedPhrase,pitch:&mut PitchResult)->Result<()> {
    let Some(curves)=phrase.automation.as_ref().filter(|c|c.has("growl_amplitude")) else{return Ok(())};
    let nominal=crate::synthesis::score::FRAME_SECONDS;
    if pitch.frame_seconds!=nominal && pitch.frame_seconds!=f64::from(nominal as f32) {
        return Err(VoiceError::new("Growl requires the 200 Hz pitch clock"));
    }
    let mut cycles=0.0_f64;
    for (frame,value) in pitch.pitch.iter_mut().enumerate() {
        let time=phrase.start_seconds+frame as f64*pitch.frame_seconds;
        let amplitude=curves.value("growl_amplitude",time,0.0).clamp(0.0,1.0);
        let frequency=curves.value("growl_frequency",time,60.0).clamp(30.0,90.0);
        if !amplitude.is_finite() || !frequency.is_finite() {return Err(VoiceError::new("Invalid growl samples"));}
        if amplitude!=0.0 && value.is_finite() {
            *value=(f64::from(*value)+0.5*f64::from(amplitude)*(std::f64::consts::TAU*cycles).sin()) as f32;
        }
        cycles+=f64::from(frequency)/200.0;
    }
    Ok(())
}
