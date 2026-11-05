// SPDX-License-Identifier: MIT
use super::{
    ControlProvider, Error, NativeRenderer, Phrase, PitchMode, RealizedPhone, RenderedPhrase,
    Result, source_frames::SourceFrames,
};
use std::sync::atomic::{AtomicBool, Ordering};
const RATE: usize = 44100;
impl NativeRenderer {
    pub fn render_source_preview(
        &mut self,
        phrase: &Phrase,
        controls: &dyn ControlProvider,
        cancel: &AtomicBool,
    ) -> Result<RenderedPhrase> {
        self.render_source_stream(phrase,controls,cancel,&mut |_|Ok(()))
    }
    pub fn render_source_stream(&mut self,phrase:&Phrase,controls:&dyn ControlProvider,cancel:&AtomicBool,
        emit:&mut dyn FnMut(super::RenderProgress<'_>)->Result<()>)->Result<RenderedPhrase> {
        phrase.validate()?;
        let first = &phrase.notes[0];
        let last = phrase.notes.last().expect("validated phrase");
        let span = phrase.seconds_at(phrase.anchor_tick + last.local_tick + last.duration_ticks)?
            - phrase.seconds_at(phrase.anchor_tick + first.local_tick)? + 1.6;
        if !span.is_finite() || span > 180.0 {
            return Err(Error::Unsupported("source preview phrase duration 0..180 seconds"));
        }
        emit(super::RenderProgress::Stage(0))?;
        let prepared = self.prepare(phrase, cancel)?;
        let mut phones = Vec::new();
        let mut clock = prepared.origin;
        for (phone, timing) in prepared.phones.iter().zip(&prepared.timing) {
            phones.push(RealizedPhone {
                note_index: phone.note,
                token_index: phone.token,
                symbol: phone.symbol.clone(),
                start_seconds: clock,
                end_seconds: clock + timing.duration,
            });
            clock += timing.duration;
        }
        let configured_hop=self.bank.configuration.numbers("voc_thop_output")?;
        if configured_hop.len()!=1 || !configured_hop[0].is_finite() || configured_hop[0]<=0.0 {
            return Err(Error::Format("output hop"));
        }
        let prepared_hop=configured_hop[0];
        emit(super::RenderProgress::Prepared {origin:prepared.origin,hop:prepared_hop,phones:&phones,failures:&prepared.note_failures})?;
        emit(super::RenderProgress::Stage(1))?;
        let baseline = if controls.pitch_mode() == PitchMode::HostComplete {
            None
        } else {
            Some(super::pitch_baseline::PitchBaseline::new(phrase)?)
        };
        let automatic = if controls.pitch_mode() == PitchMode::SharedAutomatic {
            self.bank.require_shared_f0()?;
            if self.automatic.is_none() {
                self.automatic = Some(super::automatic_f0::AutomaticF0::open(
                    &self.resources.shared_f0_path,
                )?);
            }
            Some(self.automatic.as_ref().unwrap().predict(
                phrase,
                &prepared.note_phones,
                &self.bank.profile.language,
                cancel,
            )?)
        } else {
            None
        };
        let at = |global: f64| -> Result<(super::ControlSample, f32)> {
            let control = controls.sample(global)?;
            control.validate(controls.pitch_mode())?;
            let mut midi = baseline.as_ref().map(|p| p.sample(global)).unwrap_or(0.0);
            if let Some(curve) = &automatic {
                if midi != 0.0 {
                    midi += f64::from(curve.sample(global));
                }
            }
            midi = control.midi_pitch.unwrap_or(midi) + control.pitch_deviation_semitones;
            Ok((control, (440.0 * 2.0f64.powf((midi - 69.0) / 12.0)) as f32))
        };
        let hop = self.bank.configuration.numbers("voc_thop_output")?;
        if hop.len() != 1 || !hop[0].is_finite() || hop[0] <= 0.0 {
            return Err(Error::Format("output hop"));
        }
        let hop = hop[0];
        let duration: f64 = prepared.timing.iter().map(|p| p.duration).sum();
        if !duration.is_finite() || duration <= 0.0 || duration > 180.0 {
            return Err(Error::Unsupported(
                "source preview phrase duration 0..180 seconds",
            ));
        }
        let frames = (duration / hop).ceil() as usize;
        let length = (duration * (RATE as f64)).ceil() as usize;
        let mut samples = Vec::with_capacity(length);
        let mut pitch_hz = Vec::with_capacity(frames);
        let mut generated_pitch_hz = Vec::with_capacity(frames);
        let note_intervals = phrase.notes.iter().map(|note| {
            Ok((phrase.seconds_at(phrase.anchor_tick + note.local_tick)?,
                phrase.seconds_at(phrase.anchor_tick + note.local_tick + note.duration_ticks)?))
        }).collect::<Result<Vec<_>>>()?;
        let active_end = note_intervals.iter().zip(&prepared.active_notes)
            .filter_map(|(&(_, end), &active)| active.then_some(end))
            .fold(f64::NEG_INFINITY, f64::max);
        let mut note_cursor = 0;
        emit(super::RenderProgress::Stage(2))?;
        let mut source = SourceFrames::new();
        self.noise = Some(super::noise_tables::NoiseTables::construct(
            &self.bank, &mut self.random_state,
        )?);
        let mut renderer = super::p3::Renderer::new(hop,self.noise.as_ref().unwrap())?;
        let targets = super::selection::Target::from_labels(
            &prepared
                .phones
                .iter()
                .map(|p| p.symbol.clone())
                .collect::<Vec<_>>(),
        );
        source.prepare(
            &self.bank,
            &prepared.selection.handles,
            &targets,
            &prepared.timing,
            &self.unit_cost,
            hop,
            &mut self.directions,
            &mut self.random_state,
            cancel,
        )?;
        let mut rows = super::retaining::Retaining::new(source, super::retaining::Request {
            bank: &self.bank, decoder: &self.decoder, model: &self.unit_cost,
            transform: &self.pitch_transform, handles: &prepared.selection.handles,
            targets: &targets, timing: &prepared.timing, origin: prepared.origin, hop,
        });
        emit(super::RenderProgress::Stage(3))?;
        let mut sent_samples=0;let mut sent_pitch=0;
        let mut terminal = false;
        for frame in 0..frames+super::retaining::MAX_ROWS {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let time = (frame as f64) * hop;
            let global = prepared.origin + time;
            let (control, desired) = at(global)?;
            while note_cursor < note_intervals.len() && global >= note_intervals[note_cursor].1 {
                note_cursor += 1;
            }
            let in_note = note_intervals.get(note_cursor).is_some_and(|&(begin, end)|
                global >= begin && global < end) && prepared.active_notes.get(note_cursor) == Some(&true);
            let Some((mut physical, sp)) = rows.next(|time| Ok(at(time)?.1), cancel)? else {
                if global < active_end {
                    return Err(Error::Format("retained source ends before note coverage"));
                }
                terminal = true;
                break;
            };
            generated_pitch_hz.push(if in_note { desired } else { 0.0 });
            if !physical.source_pitch.is_finite() || physical.source_pitch>700.0 {
                return Err(Error::Format("release stored pitch wrapper"));
            }
            let acoustic_pitch=(f64::from(desired)*physical.source_pitch.exp()) as f32;
            if !acoustic_pitch.is_finite() {
                return Err(Error::Format("release finite source pitch"));
            }
            self.release.apply(&mut physical, acoustic_pitch, control)?;
            if sp.iter().any(|&x| x >= -6.0) {
                let residual = self
                    .residual
                    .sample(&physical.noise_psd_db, &mut self.random_state)?;
                for i in 0..256 {
                    physical.noise_psd_db[i] =
                        (f64::from(residual[i]) - f64::from_bits(0x3ffa0ec3627ff0bd)
                            + f64::from(physical.noise_psd_db[i])) as f32;
                }
            }
            let f0 = physical.f0_hz;
            pitch_hz.push(f0);
            let output = renderer.feed(physical,self.noise.as_ref().unwrap(),self.decoder.nyquist,hop)?;
            samples.extend(output);
            if samples.len()-sent_samples>=4096 {
                emit(super::RenderProgress::Pcm {origin:prepared.origin+sent_samples as f64/RATE as f64,
                    samples:&samples[sent_samples..],pitch_origin:prepared.origin+sent_pitch as f64*hop,
                    hop,pitch:&generated_pitch_hz[sent_pitch..]})?;
                sent_samples=samples.len();sent_pitch=generated_pitch_hz.len();
            }
        }
        if !terminal {
            return Err(Error::Unsupported("P3 terminal source-row budget"));
        }
        if prepared.origin+samples.len() as f64/(RATE as f64)<active_end {
            return Err(Error::Format("P3 output ends before active note coverage"));
        }
        if samples.iter().any(|x| !x.is_finite()) {
            return Err(Error::Format("nonfinite source preview audio"));
        }
        if sent_samples<samples.len() {
            emit(super::RenderProgress::Pcm {origin:prepared.origin+sent_samples as f64/RATE as f64,
                samples:&samples[sent_samples..],pitch_origin:prepared.origin+sent_pitch as f64*hop,
                hop,pitch:&generated_pitch_hz[sent_pitch..]})?;
        }
        Ok(RenderedPhrase {
            samples,
            sample_rate: RATE as u32,
            origin_seconds: prepared.origin,
            pitch_origin_seconds: prepared.origin,
            pitch_hop_seconds: hop,
            generated_pitch_hz,
            pitch_hz,
            phones,
            note_failures: prepared.note_failures,
            accuracy_notes: vec![
                "Native default baseline plus real optional automatic F0; waveform port is not legacy bit parity",
                "Stored source pitch contour plus release pitch/formant/Rd/expression and bank EQ; generated pitch excludes unit contour",
                "DP plus reverse/forward duration refinement, learned bidirectional P1, source warp, pair phase repair and source-selected cross-unit fades",
                "Cumulative source/release clocks, pair front/tail trim and retained row splice; long-rest resets remain pending",
                "Source frame carry/lookahead, current-NM power rings, previous-NM PSD, output lags and adapter debt; embedded noise-wave playback and source direct/chirp-Z classic cost selector",
                "Platform FFT/trig/exp differ from legacy CRT kernels",
            ],
        })
    }
}
