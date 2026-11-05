use crate::synthesis::{
    inference::ops::attention_relative::{AttentionOptions, RelativeAttention},
    voice::{PreparedTree, Result, VoiceError},
};

const TYPE: [u8; 16] = [
    0x06, 0x29, 0xd0, 0x1b, 0xcd, 0xd4, 0x69, 0x27, 0xef, 0xd5, 0xaa, 0x5c, 0xa4, 0x4f, 0xb0, 0xe8,
];

pub(super) fn execute(
    tree: &PreparedTree,
    index: usize,
    input: &[f32],
    width: usize,
) -> Result<Vec<f32>> {
    let node = tree.node(index)?;
    let [query, key, value, output] = node.children.as_slice() else {
        return Err(VoiceError::new(
            "Phonetic self attention requires four projections",
        ));
    };
    let payload = tree.payload(index)?;
    if node.type_id != TYPE || payload.len() != 21 || payload[20] != 0 {
        return Err(VoiceError::new("Unsupported phonetic attention schema"));
    }
    let word = |offset| u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap());
    let window = word(0) as usize;
    let channels = word(4) as usize;
    let heads = word(8) as usize;
    let distance_penalty = f32::from_bits(word(16));
    if channels != width
        || heads == 0
        || !channels.is_multiple_of(heads)
        || !input.len().is_multiple_of(width)
    {
        return Err(VoiceError::new("Phonetic attention dimensions disagree"));
    }
    let head_width = channels / heads;
    if head_width == 0 || !head_width.is_multiple_of(2) {
        return Err(VoiceError::new(
            "Unsupported odd phonetic rotary head width",
        ));
    }
    let mut q = tree.execute_node(*query, input)?;
    let mut k = tree.execute_node(*key, input)?;
    let v = tree.execute_node(*value, input)?;
    if q.len() != input.len() || k.len() != input.len() || v.len() != input.len() {
        return Err(VoiceError::new(
            "Phonetic attention projections changed width",
        ));
    }
    rotate(&mut q, width, head_width);
    rotate(&mut k, width, head_width);
    let attended = RelativeAttention::new(None, None)
        .map_err(error)?
        .execute(
            &q,
            &k,
            &v,
            heads,
            head_width,
            head_width,
            AttentionOptions {
                window: Some(window),
                distance_penalty,
                ..Default::default()
            },
        )
        .map_err(error)?;
    let mut result = tree.execute_node(*output, &attended)?;
    super::add(&mut result, input)?;
    Ok(result)
}

fn rotate(values: &mut [f32], width: usize, head_width: usize) {
    let half = head_width / 2;

    for (position, row) in values.chunks_exact_mut(width).enumerate() {
        for head in row.chunks_exact_mut(head_width) {
            for i in 0..half {
                let exponent = -((2 * i) as f32) / head_width as f32;
                let phase = 10000.0_f32.powf(exponent) * position as f32;
                let cosine = phase.cos();
                let sine = phase.sin();
                let a = head[i];
                let b = head[i + half];
                head[i] = a * cosine - b * sine;
                head[i + half] = a * sine + b * cosine;
            }
        }
    }
}

fn error(e: impl std::fmt::Display) -> VoiceError {
    VoiceError::new(e.to_string())
}
