//! Portable packed convolution kernel for targets without SSE2 (e.g. aarch64).
//!
//! Lane arithmetic is independent and the reduction walks inputs in the same
//! order as the SSE2 backend: for each input channel, `sum += weight * sample`
//! with no reassociation or FMA contraction. Results are therefore bit-identical
//! to the x86-64 path, which keeps generated audio reproducible across hosts.

#[derive(Clone, Copy)]
pub(super) struct Output {
    lanes: [f32; 8],
}

impl Output {
    #[inline(always)]
    pub(super) fn zero() -> Self {
        Self { lanes: [0.0; 8] }
    }

    #[inline(always)]
    pub(super) fn load(values: &[f32]) -> Self {
        debug_assert!(values.len() <= 8);
        let mut lanes = [0.0; 8];
        lanes[..values.len()].copy_from_slice(values);
        Self { lanes }
    }

    #[inline(always)]
    pub(super) fn store(self, values: &mut [f32]) {
        debug_assert!(values.len() <= 8);
        // Partial output blocks only expose the real lanes; padding stays here.
        values.copy_from_slice(&self.lanes[..values.len()]);
    }

    #[inline(always)]
    pub(super) fn add(&mut self, value: Self) {
        for (lane, addend) in self.lanes.iter_mut().zip(value.lanes) {
            *lane += addend;
        }
    }
}

#[inline(always)]
pub(super) fn accumulate_one(weights: &[f32], source: &[f32], sum: &mut Output) {
    debug_assert_eq!(weights.len() / 8, source.len());
    for (weights, &sample) in weights.as_chunks::<8>().0.iter().zip(source) {
        for (lane, &weight) in sum.lanes.iter_mut().zip(weights) {
            *lane += weight * sample;
        }
    }
}

#[inline(always)]
pub(super) fn accumulate_four(weights: &[f32], source: [&[f32]; 4], sums: &mut [Output; 4]) {
    debug_assert!(source.iter().all(|row| row.len() == weights.len() / 8));
    for (index, weights) in weights.as_chunks::<8>().0.iter().enumerate() {
        for (row, sum) in sums.iter_mut().enumerate() {
            let sample = source[row][index];
            for (lane, &weight) in sum.lanes.iter_mut().zip(weights) {
                *lane += weight * sample;
            }
        }
    }
}
