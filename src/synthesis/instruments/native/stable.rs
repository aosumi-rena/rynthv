use super::{program::Node, tensor::Tensor};
pub fn frames(
    n: &Node,
    args: &[Tensor],
    ready: &[usize],
    module: Option<&Node>,
    conditional: Option<usize>,
    output: &Tensor,
) -> usize {
    if ready.iter().all(|r| *r == usize::MAX) {
        return usize::MAX;
    }
    let length = output.shape.get(1).copied().unwrap_or(1);
    let first = ready.first().copied().unwrap_or(usize::MAX);
    let both = ready.iter().copied().min().unwrap_or(usize::MAX);
    let safe = match n.opcode {
        2 if args[0].shape.get(1) == output.shape.get(1) => first,
        2 => 0,
        3 if n.p(0) != 1 => both,
        0x100 => first.saturating_mul(n.p(2) + 1),
        0x200 => first.saturating_add(n.p(2)),
        0x800 | 0x801 => {
            let Some(m) = module else { return 0 };
            let (stride, dilation) = (m.p(0), m.p(1));
            if stride == 0 || dilation == 0 {
                return 0;
            }
            if n.opcode == 0x801 {
                first.saturating_mul(stride).saturating_sub(n.p(0))
            } else {
                let kernel = (m.shape[0] - 1) * dilation + 1;
                first.saturating_sub(kernel) / stride + usize::from(first >= kernel)
            }
        }
        0x804..=0x806 => conditional.unwrap_or(0),
        0x807 => first,
        0x902 => {
            let size = 2 * (n.shape.last().copied().unwrap_or(1) - 1);
            if n.p(0) == 0 {
                return 0;
            }
            first.saturating_sub(size) / n.p(0) + usize::from(first >= size)
        }
        0x903 => first
            .saturating_mul(n.p(0))
            .saturating_sub(args[0].shape[2] - 1),
        0xb00 => first.saturating_sub(1).saturating_mul(n.p(1)),
        0xa01 if n.p(0) != 0 => both.saturating_sub(n.p(0) / 2),
        0xa01 => 0,
        0x701 if n.p(0) == 1 => 0,
        0x300..=0x304
        | 0x401..=0x409
        | 0x500..=0x503
        | 0x600..=0x602
        | 0x701
        | 0x900
        | 0xa00
        | 0xa02 => both,
        _ => 0,
    };
    safe.min(length)
}
