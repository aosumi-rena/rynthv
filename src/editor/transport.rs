use super::LoopRegion;
use crate::model::TimeAxis;
use std::time::Instant;

pub struct Transport {
    pub position: i64,
    pub playing: bool,
    pub looping: bool,
    pub loop_region: LoopRegion,
    pub return_on_pause: bool,
    start_position: i64,
    last: Option<Instant>,
}
impl Default for Transport {
    fn default() -> Self {
        Self {
            position: 0,
            playing: false,
            looping: false,
            loop_region: LoopRegion::default(),
            return_on_pause: true,
            start_position: 0,
            last: None,
        }
    }
}
impl Transport {
    pub fn toggle(&mut self) {
        if self.playing {
            self.playing = false;
            if self.return_on_pause {
                self.position = self.start_position;
            }
        } else {
            self.constrain_to_loop();
            self.start_position = self.position;
            self.playing = true;
        }
        self.last = Some(Instant::now());
    }
    pub fn stop(&mut self) {
        self.playing = false;
        self.position = 0;
        self.start_position = 0;
        self.last = None;
    }
    pub fn script_stop(&mut self) {
        self.playing=false;self.position=self.start_position;self.last=None;
    }

    pub fn constrain_to_loop(&mut self) {
        if self.looping && !self.loop_region.contains(self.position) {
            self.position = self.loop_region.start();
        }
    }

    pub fn loop_seconds(&self, time: &TimeAxis) -> Option<(f64, f64)> {
        self.looping.then(|| {
            (
                time.seconds_at(self.loop_region.start()).max(0.0),
                time.seconds_at(self.loop_region.end()).max(0.0),
            )
        })
    }

    pub fn update(&mut self, time: &TimeAxis, end: i64, audio_seconds: Option<f64>) -> bool {
        let now = Instant::now();
        let mut looped = false;
        if self.playing
            && let Some(last) = self.last
        {
            let seconds = audio_seconds.unwrap_or_else(|| {
                time.seconds_at(self.position) + now.duration_since(last).as_secs_f64()
            });
            self.position = time.position_at(seconds);
            if let Some((start_seconds, end_seconds)) = self.loop_seconds(time) {
                if audio_seconds.is_some() {

                    self.position = self
                        .position
                        .clamp(self.loop_region.start(), self.loop_region.end() - 1);
                } else if seconds >= end_seconds && end_seconds > start_seconds {
                    let wrapped = start_seconds
                        + (seconds - start_seconds).rem_euclid(end_seconds - start_seconds);
                    self.position = time
                        .position_at(wrapped)
                        .clamp(self.loop_region.start(), self.loop_region.end() - 1);
                    looped = true;
                }
            } else if self.position >= end {
                self.position = end;
                self.playing = false;
            }
        }
        self.last = Some(now);
        looped
    }
}
