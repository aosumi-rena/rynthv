use super::{
    Contract, Sv1TimbreNetwork,
    frontend::ContextInput,
    pitch,
    records::{self, Records},
};
use crate::synthesis::{
    inference::Sequence,
    stages::{f0::PitchResult, timbre::TimbreRequest},
    voice::{Result, VoiceError},
};

pub struct PreparedSv1TimbreFrames {
    pub(super) contract: Contract,
    pub(super) expanded: Sequence,
    pub(super) records: Records,
    pub(super) phone_frames: Vec<usize>,
    pub(super) noise_ids: Vec<i32>,
    pub(super) spectral_silence:Option<Vec<bool>>,
}

impl PreparedSv1TimbreFrames {
    pub(super) fn prepare(
        network: &Sv1TimbreNetwork<'_>,
        request: &TimbreRequest<'_>,
        pitch: &PitchResult,
    ) -> Result<Self> {
        let phone_frames: Vec<_> = request.phones.iter().map(|phone| phone.frames).collect();
        let frames = phone_frames
            .iter()
            .try_fold(0_usize, |total, &count| total.checked_add(count))
            .ok_or_else(|| VoiceError::new("DDS acoustic phone timeline overflows"))?;
        let log_hz = pitch::sample_log_hz(
            pitch,
            request.midi_pitch,
            frames,
            network.frontend.frame_seconds,
            network.contract,
        )?;
        let records = records::prepare(request, network.contract, log_hz)?;
        let linguistic = network.frontend.linguistic(request.phones)?;
        let normalized_pitch = network.frontend.normalize_pitch(&records.log_hz)?;
        let mut style_rows=if let Some(modes)=request.mode_coefficients {
            let names=crate::synthesis::vocal_modes::ordered_style_names(network.tree,network.style_node,network.styles.variants.len())?;
            let constant=(!modes.animated()).then(||network.styles.blend(&modes.weights(&names,false))).transpose()?;
            let mut values=Vec::with_capacity(frames*network.styles.neutral.len());
            for frame in 0..frames {
                if let Some(row)=&constant {values.extend_from_slice(row);}
                else {values.extend(network.styles.blend(&modes.weights_at(&names,false,frame))?);}
            }
            Some(Sequence::from_values(network.styles.neutral.len(),values)?)
        }else{None};
        if let Some(weights)=request.dds.and_then(|d|d.default_weights.as_ref()) {
            if weights.len()!=frames || weights.iter().any(|v|!v.is_finite()) {return Err(VoiceError::new("Default style weights are not aligned"));}
            let rows=style_rows.get_or_insert(Sequence::from_values(network.styles.neutral.len(),network.styles.neutral.repeat(frames))?);
            for (row,&weight) in rows.values.chunks_exact_mut(network.styles.neutral.len()).zip(weights) {
                if weight!=1.0 {for (value,&neutral) in row.iter_mut().zip(network.styles.neutral.iter()) {*value+=neutral*(weight-1.0);}}
            }
        }
        let expanded = network.context.prepare_frames(&ContextInput {
            phones: request.phones,
            linguistic: &linguistic,
            normalized_pitch: &normalized_pitch,
            bank: &network.bank,
            bank_rows:request.dds.and_then(|d|d.bank_rows.as_ref()),
            style: &network.styles.neutral,
            style_rows:style_rows.as_ref(),
            phone_factors: None,
        })?;
        let spectral_silence=Some(super::silence::flags(&linguistic.silence,&phone_frames)?);
        expanded.validate()?;
        #[cfg(feature="acoustic-diagnostic")]
        for (label,value) in [("compatible_acoustic_linguistic",&linguistic.normalized),("compatible_acoustic_expanded",&expanded)] {
            crate::synthesis::stages::f0::diagnostic::capture(label.into(),value)?;
        }
        if expanded.frames != frames
            || records.log_hz.len() != frames
            || linguistic.noise_ids.len() != phone_frames.len()
        {
            return Err(VoiceError::new(
                "Compatible prepared frame and phone timelines differ",
            ));
        }
        Ok(Self {
            contract: network.contract,
            expanded,
            records,
            phone_frames,
            noise_ids: linguistic.noise_ids,
            spectral_silence,
        })
    }

    pub fn len(&self) -> usize {
        self.expanded.frames
    }

    pub fn is_empty(&self) -> bool {
        self.expanded.frames == 0
    }

    pub(super) fn first_phone(&self, requested_frame: usize) -> Result<(usize, usize)> {
        if requested_frame > self.len() {
            return Err(VoiceError::new(
                "Compatible acoustic seek is outside the prepared timeline",
            ));
        }
        let mut remaining = requested_frame.saturating_sub(512);
        let mut first_phone = 0;
        let mut first_frame = 0;
        while remaining > 0 {
            let &count = self
                .phone_frames
                .get(first_phone)
                .ok_or_else(|| VoiceError::new("Compatible acoustic seek has no matching phone"))?;
            if count >= remaining {
                break;
            }
            remaining -= count;
            first_frame += count;
            first_phone += 1;
        }
        Ok((first_phone, first_frame))
    }
}
