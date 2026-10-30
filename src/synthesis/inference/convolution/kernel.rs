mod parallel;
#[cfg(target_arch = "x86_64")]
mod sse2;

#[cfg(not(target_arch = "x86_64"))]
#[path = "kernel/portable.rs"]
mod sse2;

use super::{
    ConvolutionConfig, ConvolutionError,
    packing::{OUTPUT_BLOCK, PackedWeights},
};
use crate::synthesis::execution::{self, Cancellation};
use sse2::{Output, accumulate_four, accumulate_one};

impl PackedWeights {
    pub(super) fn execute_pending(
        &self,
        config: ConvolutionConfig,
        pending: &[f32],
        output: &mut [f32],
        bias: Option<&[f32]>,
    ) -> Result<(), ConvolutionError> {
        let cancellation = execution::current();
        let macs_per_frame = self
            .input_channels()
            .saturating_mul(self.output_channels())
            .saturating_mul(config.kernel);
        parallel::for_frames(
            output,
            self.output_channels(),
            macs_per_frame,
            |first, target| {
                self.execute_frames(config, pending, first, target, bias, cancellation.as_ref())
            },
        )
    }

    fn execute_frames(
        &self,
        config: ConvolutionConfig,
        pending: &[f32],
        first_frame: usize,
        output: &mut [f32],
        bias: Option<&[f32]>,
        cancellation: Option<&Cancellation>,
    ) -> Result<(), ConvolutionError> {
        let inputs = self.input_channels();
        let outputs = self.output_channels();
        let frames = output.len() / outputs;
        if let Some(taps)=&self.integer_taps {
            for (frame,target) in output.chunks_exact_mut(outputs).enumerate() {
                check_cancelled(cancellation)?;target.fill(0.0);
                for (tap,matrix) in taps.iter().enumerate() {
                    if let Some(source)=source_row(pending,first_frame+frame,tap,inputs,config) {
                        if !matrix.integer_frame(source,target,true) {
                            for (channel,value) in target.iter_mut().enumerate() {
                                for (&weight,&input) in matrix.row(channel).iter().zip(source) {*value+=weight*input;}
                            }
                        }
                    }
                }
                if let Some(bias)=bias {for (value,bias) in target.iter_mut().zip(bias) {*value+=bias;}}
            }
            return Ok(());
        }
        for (block, weights) in self.blocks().enumerate() {
            check_cancelled(cancellation)?;
            let channel = block * OUTPUT_BLOCK;
            let width = (outputs - channel).min(OUTPUT_BLOCK);
            let bias = bias.map(|values| Output::load(&values[channel..channel + width]));
            let mut frame = 0;
            while frames - frame >= 4 {
                if frame.is_multiple_of(32) {
                    check_cancelled(cancellation)?;
                }
                let mut sums = [Output::zero(); 4];
                for (tap, weights) in weights.chunks_exact(inputs * OUTPUT_BLOCK).enumerate() {
                    let rows: [_; 4] = std::array::from_fn(|row| {
                        source_row(pending, first_frame + frame + row, tap, inputs, config)
                    });
                    if let [Some(a), Some(b), Some(c), Some(d)] = rows {
                        accumulate_four(weights, [a, b, c, d], &mut sums);
                    } else {

                        for (sum, row) in sums.iter_mut().zip(rows) {
                            if let Some(row) = row {
                                accumulate_one(weights, row, sum);
                            }
                        }
                    }
                }
                for (row, mut sum) in sums.into_iter().enumerate() {
                    if let Some(bias) = bias {
                        sum.add(bias);
                    }
                    let start = (frame + row) * outputs + channel;
                    sum.store(&mut output[start..start + width]);
                }
                frame += 4;
            }
            for frame in frame..frames {
                let mut sum = Output::zero();
                for (tap, weights) in weights.chunks_exact(inputs * OUTPUT_BLOCK).enumerate() {
                    if let Some(row) = source_row(pending, first_frame + frame, tap, inputs, config)
                    {
                        accumulate_one(weights, row, &mut sum);
                    }
                }
                if let Some(bias) = bias {
                    sum.add(bias);
                }
                let start = frame * outputs + channel;
                sum.store(&mut output[start..start + width]);
            }
        }
        check_cancelled(cancellation)
    }

    pub(super) fn accumulate(&self, tap: usize, source: &[f32], target: &mut [f32]) {
        if let Some(taps)=&self.integer_taps {
            let matrix=&taps[tap];
            if matrix.integer_frame(source,target,true) {return;}
            for (channel,value) in target.iter_mut().enumerate() {
                for (&weight,&input) in matrix.row(channel).iter().zip(source) {*value+=weight*input;}
            }
            return;
        }
        let tap_start = tap * self.input_channels() * OUTPUT_BLOCK;
        let tap_end = tap_start + self.input_channels() * OUTPUT_BLOCK;
        for (weights, target) in self.blocks().zip(target.chunks_mut(OUTPUT_BLOCK)) {
            let mut sum = Output::load(target);
            accumulate_one(&weights[tap_start..tap_end], source, &mut sum);
            sum.store(target);
        }
    }
}

#[inline]
fn check_cancelled(cancellation: Option<&Cancellation>) -> Result<(), ConvolutionError> {
    if cancellation.is_some_and(Cancellation::is_cancelled) {
        Err(ConvolutionError::Cancelled)
    } else {
        Ok(())
    }
}

#[inline]
fn source_row(
    pending: &[f32],
    frame: usize,
    tap: usize,
    inputs: usize,
    config: ConvolutionConfig,
) -> Option<&[f32]> {

    let start = (frame * config.stride + tap * config.dilation) * inputs;
    pending.get(start..start + inputs)
}
