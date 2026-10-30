use super::Contract;
use crate::synthesis::{
    inference::Sequence,
    stages::sv1_noise::Normal,
    voice::{Result, VoiceError},
};
use std::collections::VecDeque;

struct SeedEntry {
    remaining: i32,
    seed: u32,
}

pub(super) struct Generator {
    normal: Normal,
    queue: VecDeque<SeedEntry>,
    remaining: usize,
    channels: usize,
    native_pending: VecDeque<SeedEntry>,
    native_phone_remaining: usize,
}

impl Generator {
    pub fn new(
        phone_ids: &[i32],
        phone_frames: &[usize],
        local_parameters: &[i32],
        channels: usize,
        contract: Contract,
    ) -> Result<Self> {
        if phone_ids.len() != phone_frames.len() || phone_ids.len() != local_parameters.len() {
            return Err(VoiceError::new(
                "SV1 acoustic noise phone inputs differ in length",
            ));
        }
        if channels == 0 {
            return Err(VoiceError::new("SV1 acoustic noise has no channels"));
        }
        let remaining = phone_frames.iter().try_fold(0_usize, |sum, &count| {
            sum.checked_add(count)
                .ok_or_else(|| VoiceError::new("SV1 acoustic noise frame count overflows"))
        })?;
        let mut queue = VecDeque::<SeedEntry>::new();
        let mut previous = String::new();
        let mut before_previous = String::new();
        for ((&id, &count), &local) in phone_ids.iter().zip(phone_frames).zip(local_parameters) {
            let id = id.to_string();
            let count = i32::try_from(count).map_err(|_| {
                VoiceError::new("SV1 acoustic noise phone duration exceeds signed range")
            })?;

            let seed = hash(format!("{previous}_{before_previous}_{id}_{local}0").as_bytes());
            before_previous = previous;
            previous = id;

            queue.push_back(SeedEntry {
                remaining: count,
                seed,
            });
        }
        let native_pending = if contract == Contract::Native {
            std::mem::take(&mut queue)
        } else {
            VecDeque::new()
        };
        if contract == Contract::Native {
            queue.push_back(SeedEntry { remaining: 0, seed: 5489 });
        }
        Ok(Self {
            normal: Normal::new(5489),
            queue,
            remaining,
            channels,
            native_pending,
            native_phone_remaining: 0,
        })
    }

    pub fn next(&mut self, frames: usize) -> Result<Sequence> {
        if frames > self.remaining {
            return Err(VoiceError::new(
                "SV1 acoustic noise request exceeds remaining frames",
            ));
        }
        let mut output = Sequence::zeros(frames, self.channels)?;
        for frame in output.values.chunks_exact_mut(self.channels) {

            while self.native_phone_remaining == 0 {
                let Some(mut entry) = self.native_pending.pop_front() else {
                    break;
                };
                self.native_phone_remaining = entry.remaining as usize;
                if let Some(previous) = self.queue.back() {
                    entry.remaining = entry.remaining.wrapping_add(previous.remaining);
                }
                self.queue.push_back(entry);
            }
            if self.queue.front().is_some_and(|entry| entry.remaining == 0) {

                self.queue.pop_front();
                if let Some(entry) = self.queue.front() {
                    self.normal.reseed(entry.seed);
                }
            }
            if let Some(entry) = self.queue.front_mut() {
                entry.remaining = entry.remaining.wrapping_sub(1);
            }
            for sample in frame {
                *sample = self.normal.next();
            }
            self.native_phone_remaining = self.native_phone_remaining.saturating_sub(1);
        }
        self.remaining -= frames;
        Ok(output)
    }
}

fn hash(bytes: &[u8]) -> u32 {
    let value = bytes.iter().fold(0x811c_9dc5_u32, |state, &byte| {
        (state ^ (byte as i8 as i32 as u32)).wrapping_mul(0x0100_0193)
    });
    if value == 0 {
        (0x811c_9dc5_u32 ^ u32::from(b' ')).wrapping_mul(0x0100_0193)
    } else {
        value
    }
}
