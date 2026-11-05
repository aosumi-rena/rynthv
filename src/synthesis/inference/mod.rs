pub mod activation;
pub(crate) mod legacy_activation;
pub mod affine;
pub mod convolution;
pub mod extent;
pub mod matrix;
pub mod noise;
pub mod ops;
mod parallel;
pub(crate) fn compute_pool()->Result<&'static rayon::ThreadPool,String> {parallel::pool()}
pub mod pooling;
pub mod recurrent;
pub mod runtime;
pub use runtime::ModelRuntime;
mod sequence;
pub use sequence::Sequence;

use std::fmt;

pub const RELU_TYPE_ID: [u8; 16] = [
    0x32, 0xe6, 0xe0, 0x5d, 0x60, 0x89, 0x2a, 0xf6, 0xc9, 0xf1, 0x12, 0x3a, 0xa6, 0x8e, 0xaf, 0x35,
];

pub const IDENTITY_TYPE_ID: [u8; 16] = [
    0x31, 0x67, 0xa3, 0x24, 0xe2, 0x56, 0xce, 0x2f, 0x32, 0x8e, 0xb4, 0x45, 0x2d, 0xc7, 0x24, 0x71,
];

pub const TANH_TYPE_ID: [u8; 16] = [
    0x7a, 0x2a, 0x9b, 0xcf, 0xa7, 0xeb, 0x1b, 0x04, 0x6b, 0x35, 0xfe, 0x05, 0x57, 0x27, 0x18, 0xb7,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
    Identity,
    Relu,
    Sigmoid,
    Silu,
    Tanh,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InferenceError {
    UnsupportedModule([u8; 16]),
    LengthMismatch { input: usize, output: usize },
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedModule(type_id) => {
                f.write_str("Unsupported synthesis module type: ")?;
                for byte in type_id {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
            Self::LengthMismatch { input, output } => write!(
                f,
                "Unary operator requires equal slice lengths: input {input}, output {output}"
            ),
        }
    }
}

impl std::error::Error for InferenceError {}

impl UnaryOperator {
    pub fn from_type_id(type_id: [u8; 16]) -> Result<Self, InferenceError> {
        match type_id {
            IDENTITY_TYPE_ID => Ok(Self::Identity),
            RELU_TYPE_ID => Ok(Self::Relu),
            TANH_TYPE_ID => Ok(Self::Tanh),
            activation::SIGMOID_TYPE_ID => Ok(Self::Sigmoid),
            activation::SILU_TYPE_ID => Ok(Self::Silu),
            _ => Err(InferenceError::UnsupportedModule(type_id)),
        }
    }

    pub fn execute(self, input: &[f32], output: &mut [f32]) -> Result<(), InferenceError> {
        if input.len() != output.len() {
            return Err(InferenceError::LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        match self {
            Self::Identity => output.copy_from_slice(input),
            Self::Sigmoid | Self::Silu => {
                let op = if self == Self::Sigmoid {
                    activation::ActivationOperator::Sigmoid
                } else {
                    activation::ActivationOperator::Silu
                };
                #[cfg(target_arch = "x86_64")]
                {
                    return op.execute_baseline(input, output);
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    return op.execute_reference(input, output);
                }
            }
            Self::Tanh => {
                for (&value, target) in input.iter().zip(output.iter_mut()) {
                    *target = value.tanh();
                }
            }
            Self::Relu => {
                for (&value, target) in input.iter().zip(output.iter_mut()) {

                    *target = if value > 0.0 { value } else { 0.0 };
                }
            }
        }
        Ok(())
    }
}
