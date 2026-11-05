use super::{ensure, Result, StdBank, StdError, StdNote, StdNoteFailure};
use std::collections::VecDeque;

#[derive(Clone)]
pub(super) struct Input {
    pub duration: f64,
    pub pitch: i32,
    pub text: String,
    pub owner: Option<usize>,
    pub silence: bool,
}
#[derive(Clone)]
pub(super) struct Record {
    pub input: usize,
    pub duration: f32,
    pub pitch: f32,
    pub phones: Vec<usize>,
}
pub(super) struct Score {
    pub inputs: Vec<Input>,
    pub records: Vec<Record>,
    pub phones: Vec<(usize, usize)>,
    pub origin: f64,
    pub note_start: f64,
    pub note_failures: Vec<StdNoteFailure>,
}

fn flush(first: bool, owner: &mut Option<usize>, pending: &mut VecDeque<Vec<usize>>,
    records: &mut Vec<Record>, inputs: &[Input]) -> Result<()> {
    if let Some(i) = owner.take() {
        let phones = if first {
            pending.pop_front().ok_or_else(|| StdError::Invalid("Empty R1 pending word".into()))?
        } else {
            pending.drain(..).flatten().collect()
        };
        ensure(!phones.is_empty(), "R1 word contains no phones")?;
        records.push(Record { input: i, duration: inputs[i].duration as f32,
            pitch: inputs[i].pitch as f32, phones });
    }
    Ok(())
}

fn silence(i: usize, inputs: &mut [Input], records: &mut Vec<Record>, phone: usize) {
    inputs[i].silence = true;
    inputs[i].pitch = 0;
    records.push(Record { input: i, duration: inputs[i].duration as f32,
        pitch: 0., phones: vec![phone] });
}

fn failure(i: usize, message: String, inputs: &mut [Input], records: &mut Vec<Record>,
    failures: &mut Vec<StdNoteFailure>, phone: usize) {
    if let Some(note_index) = inputs[i].owner {
        failures.push(StdNoteFailure { note_index, message });
    }
    silence(i, inputs, records, phone);
}

impl Score {
    pub fn prepare(bank: &StdBank, notes: &[StdNote], request_origin: Option<f64>) -> Result<Self> {
        ensure(!notes.is_empty() && notes.len() <= 4094, "R1 score exceeds note capacity")?;
        let note_start = notes[0].start_seconds;
        let origin = request_origin.unwrap_or(note_start);
        ensure(note_start.is_finite() && origin.is_finite() && origin >= 0. && origin <= note_start
            && note_start - origin <= 3600.,
            "R1 request origin must be in its actual nonnegative leading window before the first note")?;
        let mut inputs = vec![Input { duration: 0.5, pitch: 60, text: "!sil".into(),
            owner: None, silence: true }; 2];
        let mut previous = note_start;
        for (i, n) in notes.iter().enumerate() {
            ensure(n.start_seconds.is_finite() && n.end_seconds.is_finite()
                && n.end_seconds > n.start_seconds && n.start_seconds >= previous && n.midi <= 127,
                "Invalid R1 note timing or pitch")?;
            if n.start_seconds > previous {
                inputs.push(Input { duration: n.start_seconds - previous, pitch: 0,
                    text: "!sil".into(), owner: None, silence: true });
            }
            inputs.push(Input { duration: n.end_seconds - n.start_seconds, pitch: n.midi as i32,
                text: if n.lyric.is_empty() { ".a".into() } else { n.lyric.clone() },
                owner: Some(i), silence: n.lyric == "!sil" });
            previous = n.end_seconds;
        }
        ensure(inputs.len() <= 4096, "R1 score including rests exceeds record capacity")?;
        let mut records: Vec<Record> = Vec::new();
        let mut pending: VecDeque<Vec<usize>> = VecDeque::new();
        let mut owner: Option<usize> = None;
        let mut note_failures = Vec::new();
        let mut chain_valid = false;
        let mut last_plus = None;
        for i in 0..inputs.len() {
            let input = inputs[i].clone();
            ensure(input.duration >= 0.01 && input.duration <= 40.,
                "R1 note duration exceeds bounded constructor domain")?;
            match input.text.as_str() {
                "-" | "!con" => {
                    flush(true, &mut owner, &mut pending, &mut records, &inputs)?;
                    if chain_valid {
                        let r = records.last_mut().ok_or_else(|| StdError::Invalid("Missing R1 continuation record".into()))?;
                        r.duration += input.duration as f32;
                    } else {
                        pending.clear();
                        failure(i, "No preceding syllable is available for continuation".into(),
                            &mut inputs, &mut records, &mut note_failures, bank.silence);
                    }
                }
                "+" => {
                    flush(true, &mut owner, &mut pending, &mut records, &inputs)?;
                    if !chain_valid {
                        pending.clear();
                        failure(i, "No syllable is available for +".into(),
                            &mut inputs, &mut records, &mut note_failures, bank.silence);
                    } else if let Some(phones) = pending.pop_front() {
                        chain_valid = phones.iter().any(|&p| p != bank.silence);
                        inputs[i].silence = !chain_valid;
                        records.push(Record { input: i, duration: input.duration as f32,
                            pitch: input.pitch as f32, phones });
                        last_plus = Some(i);
                        if !chain_valid { pending.clear(); last_plus = None; }
                    } else {
                        failure(i, "No syllable is available for +".into(),
                            &mut inputs, &mut records, &mut note_failures, bank.silence);
                        chain_valid = false;
                        last_plus = None;
                    }
                }
                _ => {
                    flush(false, &mut owner, &mut pending, &mut records, &inputs)?;
                    pending.clear();
                    last_plus = None;
                    if input.silence {
                        records.push(Record { input: i, duration: input.duration as f32,
                            pitch: input.pitch as f32, phones: vec![bank.silence] });
                        chain_valid = false;
                    } else {
                        match bank.language.resolve(&input.text, &bank.selectors, &bank.phones) {
                            Ok(syllables) => {
                                chain_valid = syllables.iter().flatten().any(|&p| p != bank.silence);
                                inputs[i].silence = !chain_valid;
                                pending.extend(syllables);
                                owner = Some(i);
                            }
                            Err(error @ (StdError::Invalid(_) | StdError::Unsupported(_))) => {
                                let message = match error {
                                    StdError::Unsupported(message) if message.starts_with("Unknown phoneme: ") => message,
                                    _ => format!("No pronunciation for: {}", input.text),
                                };
                                failure(i, message, &mut inputs, &mut records,
                                    &mut note_failures, bank.silence);
                                chain_valid = false;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
            }
        }
        flush(false, &mut owner, &mut pending, &mut records, &inputs)?;
        if !pending.is_empty() {
            if let Some(i) = last_plus {
                if let Some(record) = records.iter_mut().find(|r| r.input == i) {
                    record.phones = vec![bank.silence];
                    record.pitch = 0.;
                }
                for input in &mut inputs[i..] {
                    input.silence = true;
                    input.pitch = 0;
                    if let Some(note_index) = input.owner {
                        note_failures.push(StdNoteFailure { note_index,
                            message: "The + sequence needs more notes".into() });
                    }
                }
            }
        }
        let phones = records.iter().enumerate().flat_map(|(r, record)|
            (0..record.phones.len()).map(move |p| (r, p))).collect::<Vec<_>>();
        ensure(phones.len() <= 4096, "R1 score exceeds graph phone capacity")?;
        Ok(Self { inputs, records, phones, origin, note_start, note_failures })
    }
    pub fn phone(&self, i: usize) -> usize { let (r, p) = self.phones[i]; self.records[r].phones[p] }
    pub fn context(&self, i: usize, sil: usize) -> [usize; 3] {
        [if i == 0 { sil } else { self.phone(i - 1) }, self.phone(i),
            if i + 1 == self.phones.len() { sil } else { self.phone(i + 1) }]
    }
    pub fn duration(&self, i: usize) -> f32 { self.records[self.phones[i].0].duration }
}
