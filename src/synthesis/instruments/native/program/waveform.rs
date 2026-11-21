use super::{Program, Result, Tensor};
use crate::synthesis::instruments::native::wave_replacement::{Gate, HOP};
use std::collections::HashMap;

const HARMONIC: usize = 7;
const NOISE: usize = 11;
const CONDITION: usize = 47;
const SPLIT: u16 = 73;

impl Program {
    pub(in crate::synthesis::instruments::native) fn run_waveform(
        &mut self,
        inputs: Vec<(u32, Tensor)>,
        gate: &Gate,
        active: &[bool],
    ) -> Result<(HashMap<u32, Tensor>, usize)> {
        let boundaries = self.waveform_boundaries()?;
        let input = inputs
            .iter()
            .find(|(tag, _)| *tag == 900)
            .ok_or("Missing waveform input")?
            .1
            .clone();
        let frames = input.shape[1];
        let source_nodes = [(HARMONIC, 0), (NOISE, 1)];
        self.stable = Some(vec![0; self.nodes.len()]);
        let sources = self.run_outputs(inputs.clone(), &[], &source_nodes);
        let stable = self.stable.take().unwrap();
        let sources = sources?;
        let [harmonic, noise] =
            gate.apply(&sources[&0], &sources[&1], &input, active, &self.cancel)?;
        let stable_frames = frames
            .min(stable[HARMONIC].saturating_sub(4) / HOP)
            .min(stable[NOISE] / (HOP / 8));
        let stable_pcm = (stable_frames * HOP).saturating_sub(12);
        self.replacements = HashMap::from([
            (boundaries[0], (harmonic, stable_pcm)),
            (boundaries[1], (noise, stable_pcm)),
            (
                CONDITION,
                (
                    Tensor::new(vec![1, frames, 192], vec![0.0; frames * 192]),
                    frames,
                ),
            ),
        ]);
        let result = self.run_stable(inputs);
        self.replacements.clear();
        self.stable = None;
        let (mut outputs, mut stable) = result?;
        if let Some(level) = &gate.level {
            stable = level.apply(&mut outputs, &input, active, stable, &self.cancel)?;
        }
        Ok((outputs, stable))
    }

    pub(in crate::synthesis::instruments::native) fn validate_waveform(
        &self,
        trained: bool,
    ) -> Result<()> {
        let outputs = self.outputs();
        if [931, 941]
            .iter()
            .any(|tag| outputs.iter().filter(|(_, t)| t == tag).count() != 1)
        {
            return Err("Incompatible instrument waveform outputs".into());
        }
        if trained {
            self.waveform_boundaries()?;
        }
        Ok(())
    }

    fn waveform_boundaries(&self) -> Result<[usize; 2]> {
        let matches = |i, opcode| {
            self.nodes
                .get(i)
                .is_some_and(|n: &super::Node| n.opcode == opcode)
        };
        if !matches(HARMONIC, 0xb00)
            || !matches(NOISE, 0x100)
            || !matches(CONDITION, 0x807)
            || self.nodes[CONDITION].ports[..2] != [15, 46]
            || self.nodes[5].tag != 900
            || self.nodes[6].offset != 76
        {
            return Err("Unsupported waveform program layout".into());
        }
        let mut result = [None, None];
        for (i, n) in self.nodes.iter().enumerate() {
            if n.opcode != 2 || n.ports[0] != SPLIT || n.shape.last() != Some(&8) {
                continue;
            }
            let branch = match n.offset {
                0 => 0,
                8 => 1,
                _ => continue,
            };
            for (j, output) in self.nodes.iter().enumerate() {
                if output.opcode != 0x801 || usize::from(output.ports[1]) != i {
                    continue;
                }
                let module = self
                    .nodes
                    .get(usize::from(output.ports[0]))
                    .ok_or("Invalid waveform filter module")?;
                if module.shape != [65, 1, 8]
                    || module.p(0) != 8
                    || output.p(0) != 32
                    || output.p(1) != 32
                    || result[branch].replace(j).is_some()
                {
                    return Err("Unsupported waveform filter layout".into());
                }
            }
        }
        match result {
            [Some(harmonic), Some(noise)] => Ok([harmonic, noise]),
            _ => Err("Missing waveform synthesis branches".into()),
        }
    }
}
