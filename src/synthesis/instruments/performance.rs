use crate::model::{Automation, Group, GroupRef, TimeAxis};
use serde::Serialize;

pub(super) fn model_gain(gain_db: f32) -> f32 {
    if gain_db > -100. { 10f32.powf(gain_db * 0.05) } else { 0. }
}

pub fn transpose(reference:&GroupRef)->Result<f64,String> {
    let voice=reference.extra.get("voice");
    let read=|key:&str|->Result<f64,String> {
        voice.and_then(|v|v.get(key)).map_or(Ok(0.),|v|v.as_f64()
            .filter(|v|v.is_finite()&&v.abs()<=100_000.)
            .ok_or_else(||format!("Invalid instrument tuning: {key}")))
    };
    Ok(read("transposeSemitones")?+read("transposeCents")?/100.)
}

#[derive(Clone, Serialize)]
pub(crate) struct Performance {
    loudness: f32,
    noise: f32,
    curves: [Option<Automation>; 2],
    time: TimeAxis,
    offset: i64,
    model_gain: f32,
}

impl Performance {
    pub fn read(time: &TimeAxis, group: &Group, reference: &GroupRef, gain_db: f32) -> Result<Self, String> {
        let read = |key: &str| -> Result<f32, String> {
            reference.extra.get("voice").and_then(|v| v.get(key)).map_or(Ok(0.), |v| {
                v.as_f64().filter(|v| v.is_finite() && v.abs() <= 1_000_000.)
                    .map(|v| v as f32).ok_or_else(|| format!("Invalid instrument parameter {key}"))
            })
        };
        let curves = ["loudness", "breathiness"].map(|key| group.parameters.get(key)
            .filter(|c| !c.points.is_empty()).cloned());
        for curve in curves.iter().flatten() { curve.validate()?; }
        Ok(Self { loudness: read("paramLoudness")?, noise: read("paramBreathiness")?,
            curves, time: time.clone(), offset: reference.blick_offset,
            model_gain: model_gain(gain_db) })
    }

    fn gains(&self, frame: usize, origin: i64) -> [f32; 2] {
        let position = self.time.position_at((origin as f64 + frame as f64 * 240.) / 48000.)
            .saturating_sub(self.offset);
        let loudness = self.loudness + self.curves[0].as_ref().map_or(0., |c| c.sample(position, 0.));
        let noise = self.noise + self.curves[1].as_ref().map_or(0., |c| c.sample(position, 0.));
        [10f32.powf((f64::from(loudness - noise.max(0.) * 5.5) / 20.) as f32),
         10f32.powf((f64::from(loudness + noise * 7.5) / 20.) as f32)]
    }

    pub fn add(&self, origin: i64, control_frames: usize, players: usize, first: usize,
        input: &[f32], output: &mut [[f32; 4]]) {
        self.add_selected(origin,control_frames,players,first,input,output,None);
    }
    pub fn add_member(&self, origin:i64,control_frames:usize,players:usize,first:usize,
        input:&[f32],output:&mut[[f32;4]],member:usize) {
        self.add_selected(origin,control_frames,players,first,input,output,Some(member));
    }
    fn add_selected(&self, origin:i64,control_frames:usize,players:usize,first:usize,
        input:&[f32],output:&mut[[f32;4]],selected:Option<usize>) {
        let mut previous = usize::MAX;
        let mut low = [1.; 2];
        let mut high = low;
        let preamp = (1f32/players as f32).sqrt()*10f32.powf(0.45000001788139343);
        for (i, (members, destination)) in input.chunks_exact(8*players).zip(output).enumerate() {
            let sample = first + i;
            let frame = sample / 240;
            if frame != previous {
                low = self.gains(frame.min(control_frames.saturating_sub(1)), origin);
                high = self.gains((frame + 1).min(control_frames.saturating_sub(1)), origin);
                previous = frame;
            }
            let fraction = (sample % 240) as f32 / 240.;
            let primary = low[0] * (1. - fraction) + high[0] * fraction;
            let noise = low[1] * (1. - fraction) + high[1] * fraction;
            for (index,source) in members.chunks_exact(8).enumerate() {
                if selected.is_some_and(|v|v!=index) {continue;}
                for c in 0..4 {
                    let primary = ((source[c] * primary) * preamp) * self.model_gain;
                    let noise = ((source[4 + c] * noise) * preamp) * self.model_gain;
                    destination[c] += primary + noise;
                }
            }
        }
    }
}
