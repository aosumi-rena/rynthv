use super::{Runtime, mixer};
use crate::{model::Project, synthesis::audio::PlaybackSource};
use std::sync::Arc;

impl Runtime {
    pub(super) fn prepare_recordings(&mut self, project: &Project) {
        match mixer::recordings(project, &self.instrumentals, &self.solo_tracks) {
            Ok(mut mix) => {
                if let Err(error)=mixer::append_instruments(&mut mix,project,&self.instruments,&self.solo_tracks){self.recording_error=Some(error);return;}
                self.recordings = mix;
                self.recording_error = None;
            }
            Err(error) => {
                self.recordings = mixer::RecordingMix::default();
                self.recording_error = Some(error);
            }
        }
    }

    pub(super) fn refresh_playback(&self) {
        let mut pending = self.recordings.pending.clone();
        let mut end = self.recordings.end;
        let plan = self.planner.plan();
        for (id, request) in &plan.requests {
            let coverage = request.coverage;
            let start = (coverage.sample_start.0.max(0) as f64 * 48_000.0
                / coverage.sample_rate as f64)
                .ceil() as usize;
            let last = (coverage.sample_end.0.max(0) as f64 * 48_000.0
                / coverage.sample_rate as f64)
                .ceil() as usize;
            end = end.max(last);
            let voice = self.routes.get(&id.reference()).and_then(|key| self.voices.get(key));
            if plan.output_controls[id].rocoflex || self.effects.contains(plan.output_controls[id].track_id) || voice.is_some_and(|v| v.live.is_some()) {
                continue;
            }

            if !self.routes.contains_key(&id.reference())
                && !self.binding_errors.contains_key(&id.reference())
                && !self.pending_voices.contains(&id.reference())
            {
                continue;
            }
            let output = &plan.output_controls[id];
            let mute = output
                .mixer
                .get("mute")
                .and_then(serde_json::Value::as_bool)
                == Some(true);
            if !mute
                && !output.reference_muted
                && (self.solo_tracks.is_empty() || self.solo_tracks.contains(&output.track_id))
            {
                pending.push(start..last);
            }
        }
        if (end == 0 && !self.metronome_enabled() && self.loop_range.is_none())
            || self.recording_error.is_some()
        {
            self.playback.clear();
        } else {
            let source = PlaybackSource::new(
                self.voices
                    .values()
                    .filter_map(|v| v.live.clone())
                    .collect(),
                self.recordings.layers.clone(),
                pending,
                end.max(self.metronome.end_frames),
            )
            .with_morphs(self.morph_playback().into_iter().chain(self.effects.playback()).collect())
            .with_metronome(Arc::clone(&self.metronome.track));
            self.playback
                .load_source(Arc::new(source), self.desired_seconds, self.desired_playing);
        }
    }
}
