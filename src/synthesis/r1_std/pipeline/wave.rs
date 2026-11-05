use super::{Result, ensure};

fn sine(value: f32) -> f32 {
    let v = f32::from_bits;
    let turns = (value * v(0x3e22f983)) as i32;
    let reduced = (turns as f32 + if value < 0. { -0.5 } else { 0.5 }) * v(0x40c90fdb);
    let x = reduced - value;
    let sign = x.to_bits() & 0x80000000;
    let q = (v(0x3fa2f983) - x.abs() * v(0x3ecf817b)) * x;
    let square = q * q;
    let polynomial = ((v(0xbb533217 ^ sign) * square + v(0x3c77ce9a | sign)) * square
        + v(0x3e5086d7 | sign)) * square;
    q * v(0x3f48d161) + polynomial
}

fn coefficient(distance: f32) -> f32 {
    if distance == 0. { return 1.; }
    if distance.abs() > 4. { return 0.; }
    let angle = distance as f64 * std::f64::consts::PI;
    let product = (sine(angle as f32) * 4.) * sine((angle * 0.25) as f32);
    ((((product as f64 / std::f64::consts::PI) / std::f64::consts::PI)
        / distance as f64) / distance as f64) as f32
}

fn prefilter(input: &[f32], ratio: f32) -> Vec<f32> {
    let mut frequency = [0f32; 32];
    let cutoff = ratio * 16.;
    let floor = cutoff as usize;
    frequency[..=floor].fill(1.);
    frequency[floor + usize::from(cutoff > floor as f32)] = cutoff - floor as f32;
    frequency[16] = frequency[15];
    for i in 17..32 { frequency[i] = frequency[32-i]; }
    let mut impulse = [0f32; 32];
    for (i, value) in impulse.iter_mut().enumerate() {
        let step = i as f32 * 0.19634954631328583;
        let mut lanes = [0f32; 4];
        for block in 0..8 { for lane in 0..4 {
            let k = block * 4 + lane;
            let product = (step * k as f32).cos() * frequency[k];
            lanes[lane] = if block == 0 { product } else { lanes[lane] + product };
        }}
        *value = ((lanes[0] + lanes[2]) + (lanes[1] + lanes[3])) * 0.03125;
    }
    let mut kernel = [0f32; 32];
    for i in 0..32 {
        let window = (0.54 - (i as f64 * std::f64::consts::TAU / 31.).cos() * 0.46) as f32;
        kernel[i] = impulse[(i+16)%32] * window;
    }
    let mut result = vec![0.; input.len()];
    for (j, value) in result.iter_mut().enumerate() {
        let mut lanes = [0f32; 4];
        for lane in 0..4 {
            let product = |block: usize| {
                let k = block * 4 + lane;
                let index = j as isize + 16 - k as isize;
                kernel[k] * if index >= 0 { input.get(index as usize).copied().unwrap_or(0.) } else { 0. }
            };
            lanes[lane] = ((product(0)+product(1)) + (product(2)+product(3)))
                + ((product(4)+product(5)) + (product(6)+product(7)));
        }
        *value = (lanes[0]+lanes[2]) + (lanes[1]+lanes[3]);
    }
    result
}

pub(super) fn resample(input: &[f32], ratio: f32) -> Result<Vec<f32>> {
    ensure(ratio.is_finite() && (0.0625..=16.).contains(&ratio)
        && input.len() <= 262144 && input.iter().all(|v| v.is_finite()),
        "Invalid R1 stored waveform resampling operands")?;
    if ratio == 1. { return Ok(input.to_vec()); }
    let count = (input.len() as f32 * ratio).round() as usize;
    ensure(count <= 262144, "R1 resampled waveform exceeds capacity")?;
    let filtered;
    let input = if ratio < 1. { filtered = prefilter(input, ratio); &filtered[..] } else { input };
    let mut output = vec![0.; count];
    for (i, value) in output.iter_mut().enumerate() {
        let position = i as f32 / ratio;
        let center = position as isize;
        let fraction = position - center as f32;
        for k in -3..=4 {
            let index = center+k;
            if index <= 0 || index >= input.len() as isize { continue; }
            *value += coefficient(fraction-k as f32) * input[index as usize];
        }
    }
    ensure(output.iter().all(|v| v.is_finite()), "R1 resampled waveform is nonfinite")?;
    Ok(output)
}
