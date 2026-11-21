#[path = "recurrent/tanh.rs"]
mod tanh;

use std::{fmt, sync::Arc};

use super::{
    InferenceError,
    activation::ActivationOperator,
    affine::Affine,
    matrix::{MatrixError, check_length},
};

pub const GRU_TYPE_ID: [u8; 16] = [
    0x06, 0xcb, 0x32, 0xfa, 0x71, 0xcd, 0x89, 0x69, 0x0b, 0x18, 0xa3, 0x8d, 0x38, 0xd7, 0xaa, 0x5d,
];

#[derive(Debug)]
pub enum RecurrentError {
    Matrix(MatrixError),
    Activation(InferenceError),
}

impl fmt::Display for RecurrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Matrix(error) => error.fmt(f),
            Self::Activation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RecurrentError {}

impl From<MatrixError> for RecurrentError {
    fn from(error: MatrixError) -> Self {
        Self::Matrix(error)
    }
}

impl From<InferenceError> for RecurrentError {
    fn from(error: InferenceError) -> Self {
        Self::Activation(error)
    }
}

#[derive(Clone)]
pub struct Gru {

    layers: [Affine; 6],
    legacy_numeric:bool,
}

#[derive(Clone)]
pub struct GruState {
    hidden: Vec<f32>,
    reset: Vec<f32>,
    update: Vec<f32>,
    candidate: Vec<f32>,
    scratch: Vec<f32>,
}

impl GruState {
    pub fn hidden(&self) -> &[f32] {
        &self.hidden
    }

    pub fn reset(&mut self) {
        self.hidden.fill(0.0);
        self.reset.fill(0.0);
        self.update.fill(0.0);
        self.candidate.fill(0.0);
        self.scratch.fill(0.0);
    }

    pub fn restore_hidden(&mut self, hidden: &[f32]) -> Result<(), MatrixError> {
        check_length("GRU checkpoint", self.hidden.len(), hidden.len())?;
        self.reset();
        self.hidden.copy_from_slice(hidden);
        Ok(())
    }
}

impl Gru {

    pub fn new(serialized_layers: [Affine; 6]) -> Result<Self, MatrixError> {
        Self::new_execution(serialized_layers,false)
    }

    pub(crate) fn new_execution(serialized_layers:[Affine;6],legacy_numeric:bool)->Result<Self,MatrixError> {
        let inputs = serialized_layers[0].input_channels();
        let hidden = serialized_layers[0].output_channels();
        for (index, layer) in serialized_layers.iter().enumerate() {
            if layer.output_channels() != hidden
                || layer.input_channels() != if index < 3 { inputs } else { hidden }
                || layer.bias().is_none()
            {
                return Err(MatrixError::InvalidShape);
            }
        }
        let merge = |first: usize, second: usize| -> Arc<[f32]> {
            serialized_layers[first]
                .bias()
                .expect("bias shapes validated")
                .iter()
                .zip(
                    serialized_layers[second]
                        .bias()
                        .expect("bias shapes validated"),
                )
                .map(|(left, right)| left + right)
                .collect()
        };
        let reset_bias = merge(0, 3);
        let update_bias = merge(1, 4);
        let [
            input_reset,
            input_update,
            input_candidate,
            hidden_reset,
            hidden_update,
            hidden_candidate,
        ] = serialized_layers;
        Ok(Self {
            legacy_numeric,
            layers: [
                Affine::new(input_reset.matrix().clone(), Some(reset_bias))?,
                Affine::new(input_update.matrix().clone(), Some(update_bias))?,
                input_candidate,
                Affine::new(hidden_reset.matrix().clone(), None)?,
                Affine::new(hidden_update.matrix().clone(), None)?,
                hidden_candidate,
            ],
        })
    }

    pub fn input_channels(&self) -> usize {
        self.layers[0].input_channels()
    }

    pub fn output_channels(&self) -> usize {
        self.layers[0].output_channels()
    }

    pub fn new_state(&self) -> GruState {
        let hidden = self.output_channels();
        GruState {
            hidden: vec![0.0; hidden],
            reset: vec![0.0; hidden],
            update: vec![0.0; hidden],
            candidate: vec![0.0; hidden],
            scratch: vec![0.0; hidden],
        }
    }

    pub fn step(&self, input: &[f32], state: &mut GruState) -> Result<(), RecurrentError> {
        check_length("GRU input", self.input_channels(), input.len())?;
        self.check_state(state)?;
        self.step_validated(input, state, crate::synthesis::execution::zero_gru_weights())
    }

    pub fn execute_sequence(
        &self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
        state: &mut GruState,
    ) -> Result<(), RecurrentError> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("op.gru.sequence");
        let inputs = frames
            .checked_mul(self.input_channels())
            .ok_or(MatrixError::InvalidShape)?;
        let outputs = frames
            .checked_mul(self.output_channels())
            .ok_or(MatrixError::InvalidShape)?;
        check_length("GRU sequence input", inputs, input.len())?;
        check_length("GRU sequence output", outputs, output.len())?;
        self.check_state(state)?;
        let zero = crate::synthesis::execution::zero_gru_weights();
        for (input, output) in input
            .chunks_exact(self.input_channels())
            .zip(output.chunks_exact_mut(self.output_channels()))
        {
            self.step_validated(input, state, zero)?;
            output.copy_from_slice(&state.hidden);
        }
        Ok(())
    }

    fn check_state(&self, state: &GruState) -> Result<(), MatrixError> {
        check_length(
            "GRU hidden state",
            self.output_channels(),
            state.hidden.len(),
        )
    }

    fn step_validated(&self, input: &[f32], state: &mut GruState, zero: bool) -> Result<(), RecurrentError> {
        let bias = |index: usize| self.layers[index].bias().expect("GRU bias");
        for (index, gate) in [&mut state.reset, &mut state.update].into_iter().enumerate() {
            if zero {
                state.scratch.copy_from_slice(bias(index));
            } else {
                self.layers[index].execute(input, &mut state.scratch)?;
                self.layers[index + 3].execute_accumulate(&state.hidden, &mut state.scratch)?;
            }
            self.sigmoid(&state.scratch, gate)?;
        }

        if zero {
            state.candidate.copy_from_slice(bias(5));
        } else {
            self.layers[5].execute(&state.hidden, &mut state.candidate)?;
        }
        for (candidate, reset) in state.candidate.iter_mut().zip(&state.reset) {
            *candidate *= reset;
        }
        if zero {
            for (candidate, bias) in state.candidate.iter_mut().zip(bias(2)) {
                *candidate += bias;
            }
        } else {
            self.layers[2].execute_accumulate(input, &mut state.candidate)?;
        }
        if self.legacy_numeric {super::legacy_activation::tanh(&state.candidate,&mut state.scratch);}
        else {tanh::apply(&state.candidate, &mut state.scratch);}

        for ((hidden, update), candidate) in state
            .hidden
            .iter_mut()
            .zip(&state.update)
            .zip(&state.scratch)
        {

            *hidden *= update;
            let remaining = 1.0 - update;
            let contribution = candidate * remaining;
            *hidden += contribution;
        }
        Ok(())
    }

    fn sigmoid(&self,input:&[f32],output:&mut[f32])->Result<(),InferenceError> {
        if self.legacy_numeric {super::legacy_activation::sigmoid(input,output);Ok(())}
        else {sigmoid(input,output)}
    }
}

fn sigmoid(input: &[f32], output: &mut [f32]) -> Result<(), InferenceError> {
    #[cfg(target_arch = "x86_64")]
    {
        ActivationOperator::Sigmoid.execute_baseline(input, output)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ActivationOperator::Sigmoid.execute_reference(input, output)
    }
}
