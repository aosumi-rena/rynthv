// SPDX-License-Identifier: MIT
#[path = "mod.rs"]
mod r2_std;
use r2_std::{
    ControlProvider, ControlSample, NativeRenderer, Note, Phrase, PitchMode, Resources, Tempo,
};
use std::{fs::OpenOptions, io::Write, path::PathBuf, sync::atomic::AtomicBool};
struct Neutral(PitchMode, [f64; 5]);
fn report_pitch(name: &str, values: &[f32], origin: f64, hop: f64) {
    let voiced = |value: f32| value.is_finite() && value > 0.0;
    let positive = values.iter().filter(|&&value| voiced(value)).count();
    let mut segments = Vec::new();
    let mut cursor = 0;
    while cursor < values.len() {
        if !voiced(values[cursor]) {
            cursor += 1;
            continue;
        }
        let begin = cursor;
        while cursor < values.len() && voiced(values[cursor]) {
            cursor += 1;
        }
        segments.push((begin, cursor));
    }
    println!("{name}-pitch frames={} positive={positive} zero={} nonfinite={} segments={}",
        values.len(), values.iter().filter(|&&v| v == 0.0).count(),
        values.iter().filter(|v| !v.is_finite()).count(), segments.len());
    for (begin, end) in segments {
        println!("{name}-pitch-segment frames={begin}..{end} seconds={}..{}",
            origin + begin as f64 * hop, origin + end as f64 * hop);
    }
}
impl ControlProvider for Neutral {
    fn pitch_mode(&self) -> PitchMode {
        self.0
    }
    fn sample(&self, _seconds: f64) -> r2_std::Result<ControlSample> {
        Ok(ControlSample {
            midi_pitch: None,
            pitch_deviation_semitones: 0.0,
            loudness_db: self.1[0],
            tension: self.1[1],
            breathiness: self.1[2],
            voicing: self.1[3],
            gender: self.1[4],
            tone_shift_cents: 0.0,
        })
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let mode = if args.first().is_some_and(|s| s == "--automatic") {
        args.remove(0);
        PitchMode::SharedAutomatic
    } else {
        PitchMode::NativeDefault
    };
    let mut timbre = [0.0, 0.0, 0.0, 1.0, 0.0];
    if args.first().is_some_and(|s| s == "--timbre") {
        if args.len() < 2 {
            return Err("--timbre loudness,tension,breathiness,voicing,gender".into());
        }
        let fields = args[1]
            .to_string_lossy()
            .split(',')
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()?;
        timbre = fields
            .try_into()
            .map_err(|_| "five timbre values required")?;
        args.drain(..2);
    }
    let rest_after = if args.first().is_some_and(|s| s == "--rest-after") {
        if args.len() < 2 {
            return Err("--rest-after requires a zero-based note index".into());
        }
        let index = args[1].to_string_lossy().parse::<usize>()?;
        args.drain(..2);
        Some(index)
    } else {
        None
    };
    if args.len() < 5 {
        return Err(
            "usage: render_phrase [--automatic] [--rest-after INDEX] BANK CLF_DIRECTORY SHARED_F0 OUTPUT.wav LYRIC ..."
                .into(),
        );
    }
    if rest_after.is_some_and(|index| index >= args.len() - 5) {
        return Err("--rest-after must identify a note before the final note".into());
    }
    let resources = Resources {
        bank_path: PathBuf::from(&args[0]),
        clf_directory: PathBuf::from(&args[1]),
        shared_f0_path: PathBuf::from(&args[2]),
    };
    let mut renderer = NativeRenderer::open(resources)?;
    let quarter = 705_600_000;
    let notes = args[4..]
        .iter()
        .enumerate()
        .map(|(i, lyric)| Note {
            local_tick: (i as i64 + i64::from(rest_after.is_some_and(|index| i > index))) * quarter,
            duration_ticks: quarter,
            midi_pitch: [60, 64, 67, 62, 65, 69, 63, 60][i % 8],
            lyric: lyric.to_string_lossy().into_owned(),
            pronunciation: None,
        })
        .collect();
    let phrase = Phrase {
        notes,
        tempos: vec![Tempo {
            tick: 0,
            bpm: 120.0,
        }],
        anchor_tick: quarter,
    };
    let result =
        renderer.render_source_preview(&phrase, &Neutral(mode, timbre), &AtomicBool::new(false))?;
    let size = result.samples.len().checked_mul(4).ok_or("WAV extent")?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(PathBuf::from(&args[3]))?;
    output.write_all(b"RIFF")?;
    output.write_all(&u32::try_from(size + 36)?.to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16u32.to_le_bytes())?;
    output.write_all(&3u16.to_le_bytes())?;
    output.write_all(&1u16.to_le_bytes())?;
    output.write_all(&result.sample_rate.to_le_bytes())?;
    output.write_all(&(result.sample_rate * 4).to_le_bytes())?;
    output.write_all(&4u16.to_le_bytes())?;
    output.write_all(&32u16.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&u32::try_from(size)?.to_le_bytes())?;
    let mut peak = 0.0f32;
    let mut squares = 0.0;
    for &sample in &result.samples {
        output.write_all(&sample.to_le_bytes())?;
        peak = peak.max(sample.abs());
        squares += f64::from(sample) * f64::from(sample);
    }
    output.sync_all()?;
    println!(
        "source-preview frames={} rate={} origin={} peak={} rms={}",
        result.samples.len(),
        result.sample_rate,
        result.origin_seconds,
        peak,
        (squares / result.samples.len() as f64).sqrt()
    );
    report_pitch("generated", &result.generated_pitch_hz, result.pitch_origin_seconds,
        result.pitch_hop_seconds);
    report_pitch("physical", &result.pitch_hz, result.pitch_origin_seconds,
        result.pitch_hop_seconds);
    for (i, note) in phrase.notes.iter().enumerate() {
        let seconds =
            phrase.seconds_at(phrase.anchor_tick + note.local_tick + note.duration_ticks / 2)?;
        let frame =
            ((seconds - result.pitch_origin_seconds) / result.pitch_hop_seconds).round() as usize;
        println!(
            "note-center {i} seconds={seconds} hz={}",
            result.pitch_hz[frame.min(result.pitch_hz.len() - 1)]
        );
    }
    for note in result.accuracy_notes {
        println!("LIMIT: {note}");
    }
    for phone in result.phones {
        println!(
            "phone {:?}:{:?} {} {}..{}",
            phone.note_index,
            phone.token_index,
            phone.symbol,
            phone.start_seconds,
            phone.end_seconds
        );
    }
    Ok(())
}
