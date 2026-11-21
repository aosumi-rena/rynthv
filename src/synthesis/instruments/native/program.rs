use super::{
    Result, archive::Archive, conditional::Conditional, fail, kernels, tensor::Tensor, u16at,
    u32at, u64at,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
mod waveform;
#[derive(Clone)]
pub struct Node {
    pub kind: u8,
    pub opcode: u16,
    pub dtype: u8,
    pub shape: Vec<usize>,
    pub strides: Vec<isize>,
    pub offset: usize,
    pub flags: u32,
    pub ports: [u16; 16],
    pub params: [u8; 64],
    pub resources: HashMap<u8, [u8; 16]>,
    pub tag: u32,
}
impl Node {
    pub fn p(&self, i: usize) -> usize {
        u32at(&self.params, i * 4) as usize
    }
    pub fn f(&self, i: usize) -> f32 {
        f32::from_bits(u32at(&self.params, i * 4))
    }
}
fn bytecode(b: &[u8]) -> Result<(usize, Vec<u8>)> {
    if b.len() < 16 || b[0] != 255 || b[3] != 252 {
        return fail("Instrument programme header");
    }
    let nodes = u16at(b, 1) as usize;
    let nonce = u64at(b, 4);
    let count = u32at(b, 12) as usize;
    let samples = (count * 8).div_ceil(6);
    if b.len() != 16 + samples * 4 {
        return fail("Instrument programme carrier count");
    }
    let mut position = ((nonce % 997) as i32 - 478).rem_euclid(997);
    let mut pending = 0u32;
    let mut bits = 0;
    let mut out = Vec::new();
    for i in 0..samples {
        let x = f32::from_bits(u32at(b, 16 + 4 * i)) as f64;
        let phase = (x.clamp(-1., 1.).acos() * 64. / std::f64::consts::PI)
            .floor()
            .clamp(0., 63.) as u32;
        let odd = phase * 2 + 1;
        if ((odd as f64 * std::f64::consts::PI / 128.).cos() - x).abs() > 0.0006 {
            return fail("Instrument programme carrier bounds");
        }
        let fraction = (position as f64 * 0.6180339887498949).fract();
        let mul = 4 * (fraction * 32.).floor().min(31.) as u32 + 3;
        let inv = (1..256).step_by(2).find(|v| v * mul % 256 == 1).unwrap();
        let unfolded = odd * inv & 255;
        let code = unfolded.min(256 - unfolded) >> 1;
        pending = ((pending & 255) << 6) | code;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            if out.len() < count {
                out.push((pending >> bits) as u8);
            } else if ((pending >> bits) & 255) != 0 {
                return fail("Programme padding");
            }
        }
        position = (position + 1) % 997;
    }
    if out.len() != count || (bits != 0 && (pending & ((1 << bits) - 1)) != 0) {
        return fail("Programme decoded extent");
    }
    Ok((nodes, out))
}
fn parse(b: &[u8]) -> Result<Vec<Node>> {
    let (count, b) = bytecode(b)?;
    let mut at = 0;
    let mut nodes: Vec<Node> = Vec::new();
    let mut ids = HashMap::new();
    while at < b.len() {
        let cmd = b[at];
        at += 1;
        let n = match cmd {
            0..=2 => 16,
            3 => 52,
            4 | 254 => 0,
            5 | 10 | 15 => 4,
            6 => 2,
            7 | 11 => 8 - 4 * (cmd == 7) as usize,
            8 | 12 => {
                if cmd == 8 {
                    8
                } else {
                    16
                }
            }
            9 => 32,
            13 | 19 => 64,
            14 => 5,
            18 => {
                if at + 6 > b.len() {
                    return fail("Programme stream header");
                }
                if u32at(&b, at) == 0 || u16at(&b, at + 4) as usize != nodes.len() {
                    return fail("Programme stream clock");
                }
                6 + nodes.len() * 16
            }
            _ => return fail("Unknown instrument programme command"),
        };
        if at + n > b.len() {
            return fail("Truncated programme command");
        }
        let p = &b[at..at + n];
        at += n;
        match cmd {
            3 | 19 => {
                let wide = cmd == 19;
                let port = if wide { 56 } else { 44 };
                let rank = p[port + 2] as usize;
                if rank > 6 {
                    return fail("Instrument tensor rank");
                }
                let mut node = Node {
                    kind: p[port + 3],
                    opcode: u16at(p, port + 6),
                    dtype: p[port + 4],
                    shape: (0..rank)
                        .map(|i| {
                            if wide {
                                u32at(p, i * 4) as usize
                            } else {
                                u16at(p, i * 2) as usize
                            }
                        })
                        .collect(),
                    strides: (0..rank)
                        .map(|i| u32at(p, if wide { 24 } else { 12 } + i * 4) as i32 as isize)
                        .collect(),
                    offset: u32at(p, if wide { 48 } else { 36 }) as usize,
                    flags: u32at(p, if wide { 52 } else { 40 }),
                    ports: [65535; 16],
                    params: [0; 64],
                    resources: HashMap::new(),
                    tag: 0,
                };
                node.ports[0] = u16at(p, port);
                nodes.push(node);
                ids.clear();
            }
            0..=2 => {
                ids.insert(cmd, p.try_into().unwrap());
            }
            4 => {
                let node = nodes.last_mut().ok_or("Programme resource without node")?;
                if ![1, 2].contains(&node.kind) {
                    return fail("Programme resource kind");
                }
                node.resources = std::mem::take(&mut ids);
            }
            5 => nodes.last_mut().ok_or("Programme tag without node")?.tag = u32at(p, 0),
            6..=9 => {
                let node = nodes.last_mut().ok_or("Programme ports without node")?;
                for (i, p) in p.chunks_exact(2).enumerate() {
                    node.ports[i] = u16at(p, 0);
                }
            }
            10..=13 => nodes
                .last_mut()
                .ok_or("Programme params without node")?
                .params[..n]
                .copy_from_slice(p),
            14 => {
                let slot = p[0] as usize;
                let tag = u32at(p, 1);
                let matches: Vec<usize> = nodes[..nodes.len().saturating_sub(1)]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, n)| (n.tag == tag).then_some(i))
                    .collect();
                if slot >= 16 || tag == 0 || matches.len() != 1 {
                    return fail("Programme named port");
                }
                nodes.last_mut().unwrap().ports[slot] = matches[0] as u16;
            }
            15 => return fail("Unsupported special programme port"),
            254 => {
                if at != b.len() {
                    return fail("Programme terminator suffix");
                }
            }
            18 => {}
            _ => unreachable!(),
        }
    }
    if nodes.len() != count {
        return fail("Instrument programme node count");
    }
    Ok(nodes)
}
pub struct Program {
    archive: Arc<Archive>,
    nodes: Vec<Node>,
    values: Vec<Option<Tensor>>,
    uses: Vec<usize>,
    tensors: HashMap<([u8; 16], u8), Tensor>,
    conditional: HashMap<usize, Conditional>,
    inputs: HashMap<u32, Tensor>,
    bound: Vec<u32>,
    cancel: Arc<AtomicBool>,
    stable: Option<Vec<usize>>,
    replacements: HashMap<usize, (Tensor, usize)>,
}
impl Program {
    pub(super) fn set_cancel(&mut self,cancel:Arc<AtomicBool>){self.cancel=cancel;}
    pub(super) fn release_inputs(&mut self){self.inputs.clear();self.values.fill(None);}
    pub fn new(archive: Arc<Archive>, id: [u8; 16], cancel: Arc<AtomicBool>) -> Result<Self> {
        let nodes = parse(&archive.read(&id)?)?;
        let values = vec![None; nodes.len()];
        let uses=vec![0;nodes.len()];
        Ok(Self {
            archive,
            nodes,
            values,
            uses,
            tensors: HashMap::new(),
            conditional: HashMap::new(),
            inputs: HashMap::new(),
            bound: Vec::new(),
            cancel,
            stable: None,
            replacements: HashMap::new(),
        })
    }
    pub fn run_stable(&mut self, inputs: Vec<(u32,Tensor)>)->Result<(HashMap<u32,Tensor>,usize)> {
        self.stable=Some(vec![0;self.nodes.len()]);
        let out=self.run(inputs,&[])?;
        let safe=self.nodes.iter().enumerate().filter(|(_,n)|n.flags&2!=0)
            .map(|(i,_)|self.stable.as_ref().unwrap()[i]).min().unwrap_or(0);
        self.stable=None;
        Ok((out,safe))
    }
    pub fn run(
        &mut self,
        inputs: Vec<(u32, Tensor)>,
        bound: &[u32],
    ) -> Result<HashMap<u32, Tensor>> {
        let outputs = self.outputs();
        self.run_outputs(inputs, bound, &outputs)
    }
    fn outputs(&self) -> Vec<(usize, u32)> {
        self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (n.flags & 2 != 0).then_some((i, n.tag)))
            .collect()
    }
    fn run_outputs(
        &mut self,
        inputs: Vec<(u32, Tensor)>,
        bound: &[u32],
        outputs: &[(usize, u32)],
    ) -> Result<HashMap<u32, Tensor>> {
        self.inputs = inputs.into_iter().collect();
        self.bound = bound.to_vec();
        self.values.fill(None);
        self.uses=super::program_lifetime::counts(&self.nodes,outputs,&self.bound,
            |index| self.replacements.contains_key(&index))?;
        let mut out = HashMap::new();
        for &(i, tag) in outputs {
            out.insert(tag, self.eval(i)?);
        }
        Ok(out)
    }
    fn tensor(&mut self, id: [u8; 16], dtype: u8) -> Result<Tensor> {
        if let Some(t) = self.tensors.get(&(id, dtype)) {
            return Ok(t.clone());
        }
        let b = self.archive.read(&id)?;
        let size = if dtype == 1 { 2 } else { 4 };
        if ![1, 2, 242].contains(&dtype) || b.len() % size != 0 {
            return fail("Instrument tensor format");
        }
        let data: Vec<f32> = b
            .chunks_exact(size)
            .map(|v| {
                if dtype == 1 {
                    half::f16::from_bits(u16at(v, 0)).to_f32()
                } else {
                    f32::from_bits(u32at(v, 0))
                }
            })
            .collect();
        let t = Tensor::new(vec![data.len()], data);
        self.tensors.insert((id, dtype), t.clone());
        Ok(t)
    }
    fn eval(&mut self, index: usize) -> Result<Tensor> {
        if self.cancel.load(Ordering::Relaxed) {
            return fail("Instrument render cancelled");
        }
        let count=self.uses.get_mut(index).ok_or("Instrument programme lifetime bounds")?;
        *count=count.checked_sub(1).ok_or("Instrument programme consumer count")?;
        if *count==0 {
            if let Some(v)=self.values[index].take(){return Ok(v);}
        } else if let Some(v)=&self.values[index] {return Ok(v.clone());}
        let n = self
            .nodes
            .get(index)
            .ok_or("Instrument programme port bounds")?
            .clone();
        if n.kind == 2 {
            return fail("Instrument module used as tensor");
        }
        let mut ready=usize::MAX;
        let y = if let Some((tensor, stable)) = self.replacements.get(&index) {
            ready = *stable;
            tensor.clone()
        } else if self.bound.contains(&n.tag) || (n.opcode == 0 && n.kind == 3) {
            let t = self
                .inputs
                .get(&n.tag)
                .ok_or_else(|| format!("Missing instrument input {}", n.tag))?
                .clone();
            if n.shape.len() != t.shape.len()
                || n.shape.iter().zip(&t.shape).any(|(a, b)| *a != 0 && a != b)
            {
                return fail("Instrument model input shape");
            }
            ready=t.shape.get(1).copied().unwrap_or(1);
            t
        } else if n.opcode == 0 && n.kind == 1 {
            let t = self.tensor(*n.resources.get(&0).ok_or("Constant resource")?, n.dtype)?;
            t.view(n.shape.clone(), n.strides.clone(), n.offset)?
        } else {
            let mut args = Vec::new();
            let mut arg_ready=Vec::new();
            let mut module = None;
            for p in n.ports.into_iter().filter(|p| *p != 65535) {
                let p = p as usize;
                if self.nodes.get(p).is_some_and(|n| n.kind == 2) {
                    module = Some((p, self.nodes[p].clone()));
                } else {
                    args.push(self.eval(p)?);
                    arg_ready.push(self.stable.as_ref().map_or(usize::MAX,|s|s[p]));
                }
            }
            let module_clock=module.clone();
            let result=match n.opcode {
                2 => {
                    let x = &args[0];
                    let shape = n
                        .shape
                        .iter()
                        .enumerate()
                        .map(|(i, d)| if *d == 0 && i < 2 { x.shape[i] } else { *d })
                        .collect();
                    x.view(shape, n.strides.clone(), n.offset)?
                }
                3 => Tensor::concat(&args.iter().collect::<Vec<_>>(), n.p(0))?,
                0x100 => super::random::normals(
                    &args[0],
                    *n.shape.last().unwrap(),
                    n.f(0),
                    n.f(1),
                    n.p(2) + 1,
                )?,
                0x800 | 0x801 => {
                    let (_, m) = module.ok_or("Convolution module")?;
                    let w = self
                        .tensor(*m.resources.get(&1).ok_or("Convolution weights")?, m.dtype)?
                        .dense();
                    let b = if let Some(id) = m.resources.get(&2) {
                        Some(self.tensor(*id, 2)?.dense())
                    } else {
                        None
                    };
                    kernels::convolution(
                        &args[0],
                        &w,
                        b.as_deref(),
                        &m.shape,
                        m.p(0),
                        m.p(1),
                        m.p(2),
                        n.opcode == 0x801,
                        n.p(0),
                        n.p(1),
                    )?
                }
                0x804..=0x806 => {
                    let (mi, m) = module.ok_or("Conditional module")?;
                    if !self.conditional.contains_key(&mi) {
                        let b = self
                            .archive
                            .read(m.resources.get(&1).ok_or("Conditional weights")?)?;
                        self.conditional.insert(
                            mi,
                            Conditional::parse(
                                &b,
                                m.dtype,
                                n.opcode == 0x805,
                                n.opcode == 0x806,
                                m.p(0) == 1,
                            )?,
                        );
                    }
                    self.conditional[&mi].run(&args[0], &args[1], &self.cancel)?
                }
                0x807 => {
                    let (_, m) = module.ok_or("GRU module")?;
                    let b = self
                        .archive
                        .read(m.resources.get(&1).ok_or("GRU weights")?)?;
                    kernels::gru(&b, &args[0], &self.cancel)?
                }
                _ => kernels::execute(&n, &args)?,
            };
            if self.stable.is_some() {
                let conditional=module_clock.as_ref().and_then(|(i,_)|self.conditional.get(i))
                    .map(|c|c.stable_frames(arg_ready[0],arg_ready.get(1).copied().unwrap_or(usize::MAX)));
                ready=super::stable::frames(&n,&args,&arg_ready,module_clock.as_ref().map(|(_,n)|n),conditional,&result);
            }
            result
        };
        if let Some(stable)=&mut self.stable {stable[index]=ready;}
        if n.dtype != 242 && !y.finite()
        {
            return Err(format!(
                "Nonfinite instrument node {index} op {:04x}",
                n.opcode
            ));
        }
        if self.uses[index]>0 {self.values[index] = Some(y.clone());}
        Ok(y)
    }
}
