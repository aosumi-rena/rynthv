const HI: [u32; 32] = [
    0x3f800000, 0x3f82c000, 0x3f858000, 0x3f888000, 0x3f8b8000, 0x3f8e8000, 0x3f91c000, 0x3f94c000,
    0x3f980000, 0x3f9b8000, 0x3f9ec000, 0x3fa24000, 0x3fa5c000, 0x3fa98000, 0x3fad4000, 0x3fb10000,
    0x3fb50000, 0x3fb8c000, 0x3fbd0000, 0x3fc10000, 0x3fc54000, 0x3fc98000, 0x3fce0000, 0x3fd28000,
    0x3fd74000, 0x3fdbc000, 0x3fe0c000, 0x3fe58000, 0x3feac000, 0x3fefc000, 0x3ff50000, 0x3ffa8000,
];

const LO: [u32; 32] = [
    0x00000000, 0x39d86988, 0x3aab0d9f, 0x3a407404, 0x3a2e0f1e, 0x3a90e62d, 0x38f4dce0, 0x3ad3bea3,
    0x3adfc146, 0x39d39b9c, 0x3ad4c982, 0x3ac10c0c, 0x3afb5aa6, 0x3a856ad3, 0x3a41f752, 0x3a8fd607,
    0x391e6678, 0x3aeebd1d, 0x398a39f4, 0x3ab13329, 0x3a9ca845, 0x3ae6f619, 0x3a923054, 0x3aa07647,
    0x391f9958, 0x3aeede5f, 0x39cdeec0, 0x3ae41b9d, 0x37c6e7c0, 0x3a92e66f, 0x3a95f454, 0x38ecb6d0,
];

pub(super) fn tanh(x: f32) -> f32 {
    if x.abs().to_bits() < 0x39000000 { return x; }
    let sign = if x.is_sign_negative() { -1.0 } else { 1.0 };
    let a = sign * x;
    if a > 10.0 { return sign; }
    if a <= 1.0 {
        let square = a * a;
        let cube = square * a;
        let (p, q) = if a < f32::from_bits(0x3f666666) {
            let p = square * f32::from_bits(0x384d2b7d) - f32::from_bits(0x3abfbc9e);
            (p * square - f32::from_bits(0x3e9058e0), square * f32::from_bits(0x3eaf769c) + f32::from_bits(0x3f588550))
        } else {
            let p = square * f32::from_bits(0x382089d8) - f32::from_bits(0x3aa18e00);
            (p * square - f32::from_bits(0x3e7679b1), square * f32::from_bits(0x3e95c65f) + f32::from_bits(0x3f38db60))
        };
        return ((p * cube) / q + a) * sign;
    }
    let twice = a + a;
    let scaled = twice * f32::from_bits(0x4238aa3b);
    let n = (if scaled > 0.0 { scaled + 0.5 } else { scaled - 0.5 }) as i32;
    let index = (n & 31) as usize;
    let exponent = (n & !31) / 32;
    let low = (exponent - if exponent < 0 { -1 } else { 0 }) >> 1;
    let high = exponent - low;
    let remainder = twice - n as f32 * f32::from_bits(0x3cb17000);
    let correction = -n as f32 * f32::from_bits(0x3585fdf4);
    let g = correction + remainder;
    let square = g * g;
    let mut p = (g * f32::from_bits(0x3e2aaaab) + 0.5) * square;
    let lo = f32::from_bits(LO[index]);
    let hi = f32::from_bits(HI[index]);
    p = ((p + correction) + remainder) * (lo + hi);
    p = (p + lo) + hi;
    p *= f32::from_bits(((low + 127) as u32) << 23);
    p *= f32::from_bits(((high + 127) as u32) << 23);
    (1.0 - 2.0 / (p + 1.0)) * sign
}

fn inverse(length: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    let seed = unsafe {
        use std::arch::x86_64::*;
        _mm_cvtss_f32(_mm_rcp_ss(_mm_set_ss(length)))
    };
    #[cfg(not(target_arch = "x86_64"))]
    let seed = 1.0 / length;
    let square = seed * seed;
    let twice = seed + seed;
    twice - square * length
}

pub(super) fn add_curve(spectrum: &mut [f32], adjustment: f32) {
    let knots = [0_usize, 5, 15, 64];
    let values = [adjustment, adjustment, 0.0, 0.0];
    for segment in 0..3 {
        let left = knots[segment];
        let length = knots[segment + 1] - left;
        let bulk = length / 8 * 8;
        let start = values[segment];
        let delta = values[segment + 1] - start;
        let reciprocal = if bulk == 0 { 0.0 } else { inverse(length as f32) };
        for i in 0..length {
            let ratio = if i < bulk { i as f32 * reciprocal } else { i as f32 / length as f32 };
            spectrum[left + i] = (ratio * delta + start) + spectrum[left + i];
        }
    }
}
