mod archive;
mod archive_cache;
mod catalog;
mod archive_file;
mod auxiliary;
pub(super) mod rewrite;
mod attention;
mod stable;
mod microphone;
mod wave_stream;
mod wave_replacement;
mod ensemble;
mod conditional;
mod convolution;
mod fourier;
mod kernels;
mod metadata;
mod manifest;
mod padding;
mod program;
mod program_lifetime;
mod random;
mod render;
pub(super) mod score;
mod tensor;

pub(super) use catalog::scan;
pub(super) use wave_replacement::open_waveform;
pub(super) use ensemble::{prepare, EnsembleStream};
pub(super) use microphone::Microphones;
type Result<T> = std::result::Result<T, String>;
fn fail<T>(message: &str) -> Result<T> {
    Err(message.into())
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn id(s: &str) -> [u8; 16] {
    hex(s).try_into().unwrap()
}
fn u16at(b: &[u8], a: usize) -> u16 {
    u16::from_le_bytes(b[a..a + 2].try_into().unwrap())
}
fn u32at(b: &[u8], a: usize) -> u32 {
    u32::from_le_bytes(b[a..a + 4].try_into().unwrap())
}
fn u64at(b: &[u8], a: usize) -> u64 {
    u64::from_le_bytes(b[a..a + 8].try_into().unwrap())
}
