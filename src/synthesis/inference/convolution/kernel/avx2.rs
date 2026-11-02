//! AVX2 convolution with ordered accumulation.
use super::{
    ConvolutionConfig, ConvolutionError, OUTPUT_BLOCK, PackedWeights, check_cancelled, source_row,
};
use crate::synthesis::execution::Cancellation;
use std::arch::x86_64::*;

#[target_feature(enable = "avx2")]
unsafe fn load(values: &[f32]) -> __m256 {
    if values.len() == 8 {
        return unsafe { _mm256_loadu_ps(values.as_ptr()) };
    }
    let mut padded = [0.0; 8];
    padded[..values.len()].copy_from_slice(values);
    unsafe { _mm256_loadu_ps(padded.as_ptr()) }
}

#[target_feature(enable = "avx2")]
unsafe fn store(value: __m256, target: &mut [f32]) {
    if target.len() == 8 {
        unsafe { _mm256_storeu_ps(target.as_mut_ptr(), value) };
        return;
    }
    let mut padded = [0.0; 8];
    unsafe { _mm256_storeu_ps(padded.as_mut_ptr(), value) };
    target.copy_from_slice(&padded[..target.len()]);
}

#[target_feature(enable = "avx2")]
unsafe fn accumulate_one(weights: &[f32], source: &[f32], sum: &mut __m256) {
    for (weights, &sample) in weights.as_chunks::<8>().0.iter().zip(source) {
        let w = unsafe { _mm256_loadu_ps(weights.as_ptr()) };
        *sum = _mm256_add_ps(*sum, _mm256_mul_ps(w, _mm256_set1_ps(sample)));
    }
}

#[target_feature(enable = "avx2")]
unsafe fn accumulate_four(weights: &[f32], source: [&[f32]; 4], sums: &mut [__m256; 4]) {
    for (i, weights) in weights.as_chunks::<8>().0.iter().enumerate() {
        let w = unsafe { _mm256_loadu_ps(weights.as_ptr()) };
        sums[0] = _mm256_add_ps(sums[0], _mm256_mul_ps(w, _mm256_set1_ps(source[0][i])));
        sums[1] = _mm256_add_ps(sums[1], _mm256_mul_ps(w, _mm256_set1_ps(source[1][i])));
        sums[2] = _mm256_add_ps(sums[2], _mm256_mul_ps(w, _mm256_set1_ps(source[2][i])));
        sums[3] = _mm256_add_ps(sums[3], _mm256_mul_ps(w, _mm256_set1_ps(source[3][i])));
    }
}

#[target_feature(enable = "avx2")]
pub(super) unsafe fn execute_frames(
    packed: &PackedWeights,
    config: ConvolutionConfig,
    pending: &[f32],
    first_frame: usize,
    output: &mut [f32],
    bias: Option<&[f32]>,
    cancel: Option<&Cancellation>,
) -> Result<(), ConvolutionError> {
    let inputs = packed.input_channels();
    let outputs = packed.output_channels();
    let frames = output.len() / outputs;
    for (block, weights) in packed.blocks().enumerate() {
        check_cancelled(cancel)?;
        let channel = block * OUTPUT_BLOCK;
        let width = (outputs - channel).min(OUTPUT_BLOCK);
        let bias = bias.map(|b| unsafe { load(&b[channel..channel + width]) });
        let mut frame = 0;
        while frames - frame >= 4 {
            if frame.is_multiple_of(32) {
                check_cancelled(cancel)?;
            }
            let mut sums = [_mm256_setzero_ps(); 4];
            for (tap, w) in weights.chunks_exact(inputs * OUTPUT_BLOCK).enumerate() {
                let rows: [_; 4] = std::array::from_fn(|row| {
                    source_row(pending, first_frame + frame + row, tap, inputs, config)
                });
                if let [Some(a), Some(b), Some(c), Some(d)] = rows {
                    unsafe { accumulate_four(w, [a, b, c, d], &mut sums) };
                } else {
                    for (sum, row) in sums.iter_mut().zip(rows) {
                        if let Some(row) = row {
                            unsafe { accumulate_one(w, row, sum) };
                        }
                    }
                }
            }
            for (row, mut sum) in sums.into_iter().enumerate() {
                if let Some(bias) = bias {
                    sum = _mm256_add_ps(sum, bias);
                }
                let first = (frame + row) * outputs + channel;
                unsafe { store(sum, &mut output[first..first + width]) };
            }
            frame += 4;
        }
        for frame in frame..frames {
            let mut sum = _mm256_setzero_ps();
            for (tap, w) in weights.chunks_exact(inputs * OUTPUT_BLOCK).enumerate() {
                if let Some(row) = source_row(pending, first_frame + frame, tap, inputs, config) {
                    unsafe { accumulate_one(w, row, &mut sum) };
                }
            }
            if let Some(bias) = bias {
                sum = _mm256_add_ps(sum, bias);
            }
            let first = frame * outputs + channel;
            unsafe { store(sum, &mut output[first..first + width]) };
        }
    }
    check_cancelled(cancel)
}

#[target_feature(enable = "avx2")]
pub(super) unsafe fn accumulate(
    packed: &PackedWeights,
    tap: usize,
    source: &[f32],
    target: &mut [f32],
) {
    let first = tap * packed.input_channels() * OUTPUT_BLOCK;
    let end = first + packed.input_channels() * OUTPUT_BLOCK;
    for (weights, target) in packed.blocks().zip(target.chunks_mut(OUTPUT_BLOCK)) {
        let mut sum = unsafe { load(target) };
        unsafe {
            accumulate_one(&weights[first..end], source, &mut sum);
            store(sum, target)
        };
    }
}
