use crate::synthesis::{
    inference::{matrix::DenseMatrix, ops::embedding::Embedding},
    voice::{PreparedModule, PreparedTree, Result, TensorStorage, VoiceError},
};

pub(super) fn prepare(tree: &PreparedTree, index: usize) -> Result<Embedding> {
    let node = tree.node(index)?;
    let PreparedModule::Embedding(existing) = &node.module else {
        return Err(VoiceError::new("Language precision requires an embedding"));
    };
    let [tensor] = node.children.as_slice() else {
        return Err(VoiceError::new("Language embedding has an invalid tensor count"));
    };
    let TensorStorage::FloatMatrix { rows, columns, values: raw, .. } = tree.tensor(*tensor)? else {
        return Err(VoiceError::new("Language precision requires a floating-point matrix"));
    };
    if *rows != existing.features() || *columns != existing.entries() {
        return Err(VoiceError::new("Language embedding dimensions disagree"));
    }
    let mut values = Vec::with_capacity(raw.len());
    for &value in raw.iter() {
        let stored = encode(value.to_bits());
        if !value.is_finite() || stored & 0x7fff >= 0x7c00 {
            return Err(VoiceError::new("Language embedding exceeds finite half precision"));
        }
        values.push(f32::from_bits(decode(stored)));
    }
    let owned = DenseMatrix::from_float(*rows, *columns, values.into())
        .map_err(|error| VoiceError::new(error.to_string()))?;
    Ok(Embedding::new(&owned))
}

fn encode(bits: u32) -> u16 {
    let sign = (bits >> 16) & 0x8000;
    let exponent = bits & 0x7f80_0000;
    let fraction = bits & 0x007f_ffff;
    if exponent >= 0x4780_0000 {
        if exponent == 0x7f80_0000 && fraction != 0 {
            return ((fraction >> 13).max(1) + 0x7c00 + sign) as u16;
        }
        return (0x7c00 + sign) as u16;
    }
    if exponent <= 0x3800_0000 {
        if exponent < 0x3300_0000 { return sign as u16; }
        let shift = 0x71 - (exponent >> 23);
        let shifted = (fraction | 0x0080_0000) >> shift;
        return (((shifted + 0x1000) >> 13) + sign) as u16;
    }
    let adjusted = exponent.wrapping_add(0xc800_0000) >> 13;
    (adjusted + ((fraction + 0x1000) >> 13) + sign) as u16
}

fn decode(half: u16) -> u32 {
    let half = u32::from(half);
    let sign = (half & 0x8000) << 16;
    let exponent = half & 0x7c00;
    let fraction = half & 0x03ff;
    if exponent != 0 {
        if exponent == 0x7c00 { return ((fraction + 0x3fc00) << 13) + sign; }
        return (((half & 0x7fff) + 0x1c000) << 13) + sign;
    }
    if fraction == 0 { return sign; }
    let mut shifted = fraction << 1;
    let mut count = 0;
    while shifted & 0x0400 == 0 { shifted <<= 1; count += 1; }
    ((shifted & 0x03ff) + ((0x70 - count) << 10)) << 13 | sign
}
