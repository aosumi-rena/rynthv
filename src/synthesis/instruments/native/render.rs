use super::super::{Entry, InstrumentSettings};
use super::{
    Result,
    archive::Archive,
    fail, id,
    program::Program,
    score::{self, Note},
    tensor::Tensor,
    wave_stream::WaveStream,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::AtomicBool,
    },
};
pub(super) const WAVE_PROGRAM: &str = "b91417eb25d7f3796045f72ff65617cd";
pub(super) const PROGRAMS: [&str; 8] = [
    "6995abb671c2e5f5b08621023e47c736",
    "6d673c66a228e5d47c9a714619edef29",
    "23427d13c400cd9437f395c86a04446a",
    "304a86b7e7f09fa529dcd663d97bcd3c",
    "1ecb4be14e20de0bcb9a0e7651a12d71",
    "5eb9686f7ad39224759662e106fc3ecb",
    "df17b7b15e9945762a89f92323bde349",
    WAVE_PROGRAM,
];
fn take(mut outputs: HashMap<u32, Tensor>, tag: u32) -> Result<Tensor> {
    outputs
        .remove(&tag)
        .ok_or_else(|| format!("Instrument output {tag} absent"))
}
pub(super) fn prepare_member(
    settings: &InstrumentSettings,
    entry: &Entry,
    mut notes: Vec<Note>,
    ambient: u32,
    players: usize,
    cancel: Arc<AtomicBool>,
    main: &Arc<Archive>,
    aux: &Arc<Archive>,
    wave_archive: Arc<Archive>,
) -> Result<WaveStream> {
    let protocol = super::protocol::Protocol::from_main(main)?;
    let recs = score::records(&notes)?;
    let frames = notes.last().unwrap().start + notes.last().unwrap().length + 100;
    let stage = |i: usize,
                 a: &Arc<Archive>,
                 inputs: Vec<(u32, Tensor)>,
                 bound: &[u32]|
     -> Result<HashMap<u32, Tensor>> {
        Program::new(a.clone(), id(PROGRAMS[i]), cancel.clone())?
            .run(inputs, bound)
            .map_err(|e| format!("Instrument stage {i}: {e}"))
    };
    let embedded=super::auxiliary::embedding(settings,entry,aux,&notes,&recs,cancel.clone())?;
    let automatic:Vec<_>=notes.iter().map(|n|n.automatic_articulations).collect();
    if automatic.iter().any(|v|*v) {
        super::rewrite::articulate(main,aux,&mut notes,&recs,&embedded,&automatic,cancel.clone())?;
    }
    let timing_input=super::auxiliary::timing_input(&notes,&recs,&embedded)?;
    let mut timing = take(
        stage(
            1,
            aux,
            vec![
                (800, timing_input),
                (810, score::timing_seeds(&notes, &recs, protocol.explicit_vibrato())),
            ],
            &[],
        )?,
        820,
    )?;
    let timing_width = if protocol.explicit_vibrato() { 5 } else { 2 };
    if timing.shape!=[1,recs.len(),timing_width]{return fail("Instrument timing output shape");}
    let mut effective_controls:Vec<_>=notes.iter().map(|note|super::super::EffectiveNote {
        id:note.id,dynamic:note.dynamic,articulations:note.articulations.clone(),
        vibrato_hz: None, vibrato_phase: None,
    }).collect();
    for (row,record) in recs.iter().enumerate() {
        if let Some(index)=record.note && notes[index].automatic_dynamics {
            let value=timing.get(row*timing_width);
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {return fail("Invalid instrument dynamics prediction");}
            effective_controls[index].dynamic=value;
        }
    }
    if protocol.explicit_vibrato() {
        for (row, record) in recs.iter().enumerate() {
            if let Some(index) = record.note {
                if let Some((hz, phase)) = super::features::vibrato(&timing, row, &notes[index]) {
                    effective_controls[index].vibrato_hz = Some(hz);
                    effective_controls[index].vibrato_phase = Some(phase);
                }
            }
        }
    }
    if players > 1 {
        let values=Arc::make_mut(&mut timing.data);
        for (row,record) in recs.iter().enumerate() {
            if let Some(index)=record.note {
                let value=&mut values[row*timing_width+1];
                if value.is_nan(){*value=0.;}
                *value*=notes[index].time_dispersion;
            }
        }
    }
    let basic = score::basic(&recs, 29, &notes);
    let pitch_encoded = take(stage(2, main, vec![(100, basic.clone())], &[])?, 110)?;
    let latent_encoded = take(stage(3, main, vec![(300, basic)], &[])?, 310)?;
    let (pf, ps) = if protocol.explicit_vibrato() {
        super::features::pitch(&notes, &recs, &pitch_encoded, &timing, settings.trill_vibrato, players)
    } else {
        score::pitch_features(&notes, &recs, &pitch_encoded, &timing, settings.trill_vibrato, players)
    };
    let raw = take(stage(4, main, vec![(200, pf), (201, ps)], &[])?, 210)?;
    let pitch = score::assemble(&raw, &notes, frames);
    let (lf, ls, mut controls) =
        score::latent_features(&notes, &recs, &latent_encoded, &timing, &pitch, frames, players);
    if protocol.explicit_vibrato() {
        controls = Tensor::concat(&[&controls, &Tensor::new(vec![1, frames / 4, 1], vec![0.; frames / 4])], 2)?;
    }
    let mut latent = stage(
        5,
        main,
        vec![(400, lf), (411, ls), (410, controls)],
        &[410],
    )?;
    let pitch1 = pitch.clone();
    let latent420 = latent.remove(&420).ok_or("Latent 420")?;
    let physical_input = Tensor::concat(
        &[
            &latent420,
            &latent.remove(&430).ok_or("Latent 430")?,
            &pitch1,
        ],
        2,
    )?;
    let mut physical = stage(6, main, vec![(500, physical_input)], &[])?;
    let pitch2 = pitch.clone();
    let clock = Tensor::new(vec![1, frames, 1], (0..frames).map(|v| v as f32).collect());
    let physical520 = physical.remove(&520).ok_or("Physical 520")?;
    let physical540 = physical.remove(&540).ok_or("Physical 540")?;
    let physical530 = physical.remove(&530).ok_or("Physical 530")?;
    let wave_input = if super::protocol::Protocol::waveform(&wave_archive)?.explicit_vibrato() {
        let phase = super::features::phase(&pitch2, ambient);
        let resonance = Tensor::new(vec![1, frames, 64], vec![0.; frames * 64]);
        Tensor::concat(&[&physical520, &resonance, &physical540, &pitch2, &physical530, &clock, &phase], 2)?
    } else {
        Tensor::concat(&[&physical520, &physical540, &pitch2, &physical530, &clock], 2)?
    };
    let mut stream=WaveStream::new(wave_archive, wave_input,
        score::wave_seeds(&notes, frames, ambient), pitch.dense());
    stream.controls=effective_controls;
    Ok(stream)
}
