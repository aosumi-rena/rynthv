use super::{
    Result, fail, fourier,
    program::Node,
    tensor::{Tensor, affine},
    u32at,
};
use std::sync::atomic::{AtomicBool, Ordering};
pub use super::convolution::run as convolution;
pub fn gru(b: &[u8], x: &Tensor, cancel: &AtomicBool) -> Result<Tensor> {
    if b.len() < 8 {
        return fail("GRU header");
    }
    let (h, input) = (u32at(b, 0) as usize, u32at(b, 4) as usize);
    if h == 0
        || h > 32768
        || input == 0
        || input != *x.shape.last().unwrap()
        || b.len() != 8 + (3 * h * input + 3 * h * h + 6 * h) * 4
    {
        return fail("Instrument GRU dimensions");
    }
    let a: Vec<f32> = b[8..]
        .chunks_exact(4)
        .map(|v| f32::from_bits(u32at(v, 0)))
        .collect();
    let ni = 3 * h * input;
    let nh = 3 * h * h;
    let wi = &a[..ni];
    let wh = &a[ni..ni + nh];
    let bi = &a[ni + nh..ni + nh + 3 * h];
    let bh = &a[ni + nh + 3 * h..];
    let q = affine(
        &x.dense(),
        x.shape[0] * x.shape[1],
        input,
        wi,
        3 * h,
        Some(bi),
    );
    let mut out = vec![0.; x.shape[0] * x.shape[1] * h];
    for batch in 0..x.shape[0] {
        let mut hidden = vec![0.; h];
        for t in 0..x.shape[1] {
            if t % 64 == 0 && cancel.load(Ordering::Relaxed) {
                return fail("Instrument render cancelled");
            }
            let r = affine(&hidden, 1, h, wh, 3 * h, Some(bh));
            let off = (batch * x.shape[1] + t) * 3 * h;
            for c in 0..h {
                let reset = 1. / (1. + (-(q[off + c] + r[c])).exp());
                let update = 1. / (1. + (-(q[off + h + c] + r[h + c])).exp());
                let candidate = (q[off + 2 * h + c] + reset * r[2 * h + c]).tanh();
                hidden[c] = (1. - update) * candidate + update * hidden[c];
            }
            out[(batch * x.shape[1] + t) * h..(batch * x.shape[1] + t + 1) * h]
                .copy_from_slice(&hidden);
        }
    }
    Ok(Tensor::new(vec![x.shape[0], x.shape[1], h], out))
}
pub fn execute(n: &Node, a: &[Tensor]) -> Result<Tensor> {
    let x = &a[0];
    if (0x300..=0x303).contains(&n.opcode) && a.iter().any(|x| x.imag.is_some()) {
        return Tensor::complex_binary(x, &a[1], n.opcode);
    }
    let binary = |f: fn(f32, f32) -> f32| Tensor::binary(x, &a[1], f);
    Ok(match n.opcode {
        0x200 => super::padding::apply(n,x,a.get(1))?,
        0x300 => binary(|a, b| a + b)?,
        0x301 => binary(|a, b| a - b)?,
        0x302 => binary(|a, b| a * b)?,
        0x303 => binary(|a, b| a / b)?,
        0x304 => binary(f32::powf)?,
        0x401 => x.map(f32::sqrt),
        0x402 => x.map(f32::exp),
        0x403 => x.map(f32::ln),
        0x404 => x.map(f32::log2),
        0x405 => x.map(f32::log10),
        0x406 => x.map(f32::sin),
        0x407 => x.map(f32::cos),
        0x409 => x.map(f32::floor),
        0x500 => binary(|_, _| 0.).and_then(|mut out| {
            let data = (0..out.len())
                .map(|i| {
                    let c = Tensor::coords(&out.shape, i);
                    let v = x.broadcast(&c);
                    let b = a[1].broadcast(&c);
                    if v < b {
                        n.f(0)
                    } else if v == b {
                        n.f(1)
                    } else {
                        n.f(2)
                    }
                })
                .collect();
            out.data = std::sync::Arc::new(data);
            Ok(out)
        })?,
        0x501 => binary(f32::min)?,
        0x502 => binary(f32::max)?,
        0x503 => Tensor::binary(&Tensor::binary(x, &a[1], f32::max)?, &a[2], f32::min)?,
        0x600 => x.map(|v| 1. / (1. + (-v).exp())),
        0x601 => x.map(f32::tanh),
        0x602 => x.map(|v| v / (1. + (-v).exp())),
        0x701 => {
            let axis = n.p(0);
            if axis >= x.shape.len() {
                return fail("Instrument reduction axis");
            }
            let mut shape = x.shape.clone();
            let count = shape[axis];
            shape[axis] = 1;
            let data = (0..shape.iter().product())
                .map(|i| {
                    let mut c = Tensor::coords(&shape, i);
                    let mut sum = 0.;
                    for j in 0..count {
                        c[axis] = j;
                        sum += x.data[x.at(&c)];
                    }
                    sum / count as f32
                })
                .collect();
            Tensor::new(shape, data)
        }
        0x900 => fourier::rfft(x, n.p(0) != 0)?,
        0x902 => fourier::stft(x, &a[1], *n.shape.last().unwrap(), n.p(0))?,
        0x903 => fourier::istft(x, &a[1], n.p(0))?,
        0xb00 => fourier::harmonics(x, n.p(0), n.p(1))?,
        0xa00 => {
            if x.shape.len() != 4 || x.shape[3] % 2 != 0 {
                return fail("Rotary position shape");
            }
            let width = x.shape[3];
            let half = width / 2;
            let mut y = x.dense();
            for b in 0..x.shape[0] {
                for t in 0..x.shape[1] {
                    for h in 0..x.shape[2] {
                        for c in 0..half {
                            let frequency = n.f(0).powf(-(c as f32) * 2. / width as f32);
                            let angle = t as f32 * frequency;
                            let off = ((b * x.shape[1] + t) * x.shape[2] + h) * width;
                            let l = x.get(off + c);
                            let r = x.get(off + half + c);
                            y[off + c] = l * angle.cos() - r * angle.sin();
                            y[off + half + c] = l * angle.sin() + r * angle.cos();
                        }
                    }
                }
            }
            Tensor::new(x.shape.clone(), y)
        }
        0xa01 => super::attention::run(x, &a[1], &a[2], n.p(0), n.f(2))?,
        0xa02 => {
            if ![0, 2].contains(&n.p(0)) {
                return fail("Instrument recurrence initialization");
            }
            let shape = Tensor::broadcast_shape(&[x, &a[1]])?;
            if shape.len() != 3 {
                return fail("Instrument recurrence rank");
            }
            let mut out = vec![0.; shape.iter().product()];
            for b in 0..shape[0] {
                for c in 0..shape[2] {
                    let mut state = if n.p(0) == 2 {
                        x.broadcast(&[b, 0, c]) as f64
                    } else {
                        0.
                    };
                    for t in 0..shape[1] {
                        state = state * a[1].broadcast(&[b, t, c]) as f64
                            + x.broadcast(&[b, t, c]) as f64;
                        out[(b * shape[1] + t) * shape[2] + c] = state as f32;
                    }
                }
            }
            Tensor::new(shape, out)
        }
        _ => {
            return Err(format!(
                "Unsupported instrument model operation {:04x}",
                n.opcode
            ));
        }
    })
}
