use super::{
    condition::{CHANNELS, VocoderCondition},
    configuration::{COMPACT_CHILDREN, Configuration, Modules, ROOT_TYPE, SpectralFilter},
    filterbank::{ANALYSIS_TYPE_ID, BANDS, SYNTHESIS_TYPE_ID, TAPS},
    sampling::POSITIONAL_GRU_TYPE_ID,
    spectral_heads::SPECTRAL_HEADS_TYPE_ID,
};
use crate::synthesis::{
    inference::ops::{SEQUENTIAL_TYPE_ID, conditional::CONDITIONAL_TYPE_ID},
    voice::{PreparedTree, Result, VoiceError},
};

pub(super) fn prepare(
    tree: &PreparedTree,
    root: usize,
) -> Result<(Configuration, Modules, VocoderCondition)> {
    let node = tree.node(root)?;
    let payload = tree.payload(root)?;
    if node.type_id != ROOT_TYPE || node.children.len() != COMPACT_CHILDREN || payload.len() != 18 {
        return Err(VoiceError::new("Invalid compact vocoder root"));
    }
    let mut config = Configuration::parse(&payload)?;
    if config.bands != CHANNELS || payload[12] != 0 || payload[13] > 1 {
        return Err(VoiceError::new("Unsupported compact vocoder configuration"));
    }
    let at = |i| node.children[i];
    for (index, expected) in [
        (1, SEQUENTIAL_TYPE_ID),
        (2, POSITIONAL_GRU_TYPE_ID),
        (3, CONDITIONAL_TYPE_ID),
        (4, SPECTRAL_HEADS_TYPE_ID),
        (5, SEQUENTIAL_TYPE_ID),
        (7, CONDITIONAL_TYPE_ID),
        (8, SEQUENTIAL_TYPE_ID),
        (9, ANALYSIS_TYPE_ID),
        (10, SYNTHESIS_TYPE_ID),
    ] {
        if tree.node(at(index))?.type_id != expected {
            return Err(VoiceError::new("Invalid compact vocoder module layout"));
        }
    }
    let bytes = tree.payload(at(0))?;
    if bytes.len() != 52 {
        return Err(VoiceError::new("Invalid compact vocoder signal geometry"));
    }
    let words: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word) as usize)
        .collect();
    let [
        channels,
        rate,
        hop,
        cepstrum,
        fft,
        window,
        padding,
        center,
        ramp,
        bands,
        taps,
        subhop,
        flags,
    ] = words.as_slice()
    else {
        return Err(VoiceError::new("Invalid compact vocoder geometry length"));
    };
    if *channels != CHANNELS
        || *rate != 48_000
        || *hop != 240
        || *cepstrum != 512
        || *fft != 1024
        || *window != hop * 2
        || *padding != (fft - window) / 2
        || *center != fft / 2
        || *ramp != 16
        || *bands != BANDS
        || *taps != TAPS
        || *subhop != hop / bands
        || *flags != 0
    {
        return Err(VoiceError::new(
            "Unsupported compact vocoder signal geometry",
        ));
    }
    for index in [9, 10] {
        let bank = tree.node(at(index))?;
        if bank.children.len() != *bands || bank.payload_bytes != 0 {
            return Err(VoiceError::new(
                "Compact vocoder filter bank geometry differs",
            ));
        }
        for &child in &bank.children {
            if tree
                .tensor(child)?
                .vector()
                .is_none_or(|values| values.len() != *taps)
            {
                return Err(VoiceError::new("Compact vocoder filter tap count differs"));
            }
        }
    }
    let condition = VocoderCondition::prepare_compact(tree, at(0), *rate as u32, *hop)?;
    let filter = SpectralFilter::Window {
        samples: *window,
        fft_size: *fft,
    };
    config.bands = *bands;
    config.use_input_projection = true;
    config.gate_postfilter_noise = true;
    let modules = Modules {
        conditions: at(0),
        input_projection: Some(at(1)),
        recurrent_sampler: Some(at(2)),
        conditioned_network: at(3),
        spectral_heads: at(4),
        gain_network: at(5),
        coefficients: at(6),
        postfilter_network: at(7),
        postfilter_projection: at(8),
        analysis_filterbank: at(9),
        synthesis_filterbank: at(10),
        harmonic_filter: filter,
        noise_filter: filter,
        third_filter: filter,
        fast_postfilter: None,
    };
    Ok((config, modules, condition))
}
