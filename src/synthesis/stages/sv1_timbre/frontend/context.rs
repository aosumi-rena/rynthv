use super::PhoneInput;
use crate::synthesis::{
    inference::{ModelRuntime, Sequence},
    stages::f0::TimedPhone,
    voice::{
        PreparedModule, PreparedTree, Result, VoiceError, embedded::types::TIMBRE_CONTEXT_TYPE_ID,
    },
};

pub struct Context<'a> {
    tree: &'a PreparedTree,
    native: bool,
    phone_projection: usize,
    pointwise: usize,
    convolution: usize,
    language: Option<usize>,
    language_override: Option<crate::synthesis::inference::ops::embedding::Embedding>,
}

pub struct ContextInput<'a> {
    pub phones: &'a [TimedPhone],
    pub linguistic: &'a PhoneInput,
    pub normalized_pitch: &'a [f32],
    pub bank: &'a [f32],
    pub bank_rows:Option<&'a Sequence>,
    pub style: &'a [f32],
    pub style_rows:Option<&'a Sequence>,
    pub phone_factors: Option<&'a [f32]>,
}

pub struct ContextOutput {
    pub convolution: Sequence,
    pub pointwise: Sequence,
}

impl<'a> Context<'a> {
    pub fn prepare(tree: &'a PreparedTree, index: usize, native: bool, half_language: bool) -> Result<Self> {
        let node = tree.node(index)?;
        if node.type_id != TIMBRE_CONTEXT_TYPE_ID
            || !matches!(node.children.len(), 3 | 4)
            || !tree.payload(index)?.is_empty()
        {
            return Err(VoiceError::new(
                "SV1 timbre context requires its typed three/four children",
            ));
        }
        let language = node.children.get(3).copied();
        if let Some(index) = language
            && !matches!(tree.node(index)?.module, PreparedModule::Embedding(_))
        {
            return Err(VoiceError::new(
                "SV1 acoustic language node is not an embedding",
            ));
        }
        if half_language && !native {
            return Err(VoiceError::new("Language precision applies only to native DDS voices"));
        }
        let language_override = if half_language {
            language.map(|index| super::language::prepare(tree, index)).transpose()?
        } else { None };
        Ok(Self {
            tree,
            native,
            phone_projection: node.children[0],
            pointwise: node.children[1],
            convolution: node.children[2],
            language,
            language_override,
        })
    }

    pub fn prepare_frames(&self, input: &ContextInput<'_>) -> Result<Sequence> {
        let frames = input
            .phones
            .iter()
            .try_fold(0_usize, |sum, phone| sum.checked_add(phone.frames))
            .ok_or_else(|| VoiceError::new("SV1 acoustic frame count overflows"))?;
        if input.linguistic.normalized.frames != input.phones.len()
            || input.linguistic.language_ids.len() != input.phones.len()
            || input.normalized_pitch.len() != frames
            || input
                .phone_factors
                .is_some_and(|values| values.len() != input.phones.len())
            || input
                .normalized_pitch
                .iter()
                .chain(input.bank)
                .chain(input.style)
                .chain(input.phone_factors.into_iter().flatten())
                .any(|value| !value.is_finite())
        {
            return Err(VoiceError::new(
                "SV1 acoustic context inputs are not aligned",
            ));
        }
        let mut runtime = ModelRuntime::new(self.tree);
        let projected =
            runtime.execute(self.phone_projection, &input.linguistic.normalized, true)?;
        if projected.frames != input.phones.len() {
            return Err(VoiceError::new(
                "SV1 phone projection changed the record count",
            ));
        }
        let language = self
            .language
            .map(|index| {
                let ids = Sequence::from_values(
                    1,
                    input
                        .linguistic
                        .language_ids
                        .iter()
                        .map(|&id| id as f32)
                        .collect(),
                )?;
                if let Some(embedding) = &self.language_override {
                    let mut output = Sequence::zeros(ids.frames, embedding.features())?;
                    embedding.execute(&ids.values, &mut output.values)
                        .map_err(|error| VoiceError::new(error.to_string()))?;
                    Ok(output)
                } else {
                    runtime.execute(index, &ids, true)
                }
            })
            .transpose()?;
        let language_width = language.as_ref().map_or(0, |values| values.channels);
        let channels = projected
            .channels
            .checked_add(8)
            .and_then(|width| width.checked_add(input.bank.len()))
            .and_then(|width| width.checked_add(language_width))
            .and_then(|width| width.checked_add(input.style.len()))
            .ok_or_else(|| VoiceError::new("SV1 acoustic conditioning width overflows"))?;
        let mut expanded = Sequence::zeros(frames, channels)?;
        if let Some(rows)=input.bank_rows {
            rows.validate()?;
            if rows.frames!=frames || rows.channels!=input.bank.len() || rows.values.iter().any(|v|!v.is_finite()) {
                return Err(VoiceError::new("Speaker vector frames are not aligned"));
            }
        }
        if let Some(styles)=input.style_rows {
            styles.validate()?;
            if styles.frames!=frames || styles.channels!=input.style.len() || styles.values.iter().any(|v|!v.is_finite()) {
                return Err(VoiceError::new("Compatible style frames are not aligned"));
            }
        }
        let mut frame_index = 0;
        for (phone_index, phone) in input.phones.iter().enumerate() {
            let projection = projected.frame(phone_index)?;
            let factor = input
                .phone_factors
                .map_or(1.0, |values| values[phone_index]);
            let language = language
                .as_ref()
                .map(|values| values.frame(phone_index))
                .transpose()?
                .unwrap_or(&[]);
            for position in 0..phone.frames {
                let row =
                    &mut expanded.values[frame_index * channels..(frame_index + 1) * channels];
                for (target, &value) in row.iter_mut().zip(projection) {
                    *target = value * factor;
                }
                let fraction = if phone.frames == 1 {
                    0.5
                } else if self.native || self.tree.legacy_numeric() {
                    position as f32 / (phone.frames-1) as f32
                } else {
                    position as f32 * (1.0 / (phone.frames - 1) as f32)
                };
                let mut cursor = projected.channels;
                for scalar in [fraction, input.normalized_pitch[frame_index]] {
                    let bank = if self.native {
                        super::quadrature::bank(scalar)?
                    } else if self.tree.legacy_numeric() {
                        compatible_cosine_bank(scalar)
                    } else {
                        cosine_bank(scalar)
                    };
                    row[cursor..cursor + 4].copy_from_slice(&bank);
                    cursor += 4;
                }
                let style=input.style_rows.map(|rows|rows.frame(frame_index)).transpose()?.unwrap_or(input.style);
                let bank=input.bank_rows.map(|rows|rows.frame(frame_index)).transpose()?.unwrap_or(input.bank);
                for values in [bank, language, style] {
                    row[cursor..cursor + values.len()].copy_from_slice(values);
                    cursor += values.len();
                }
                frame_index += 1;
            }
        }
        Ok(expanded)
    }

    pub fn process(
        &self,
        runtime: &mut ModelRuntime<'_>,
        expanded: &Sequence,
        flush: bool,
    ) -> Result<ContextOutput> {

        let convolution = runtime.execute(self.convolution, expanded, flush)?;
        let pointwise = runtime.execute(self.pointwise, expanded, flush)?;
        Ok(ContextOutput {
            convolution,
            pointwise,
        })
    }
}

fn cosine_bank(value: f32) -> [f32; 4] {
    let angle = value * std::f32::consts::TAU;
    [
        std::f32::consts::FRAC_PI_2,
        0.0,
        -std::f32::consts::FRAC_PI_2,
        -std::f32::consts::PI,
    ]
    .map(|phase| (f64::from((angle + phase).cos()) * 0.5 + 0.5) as f32)
}

fn compatible_cosine_bank(value:f32)->[f32;4] {
    [0.25,0.0,-0.25,-0.5].map(|phase|{
        let angle=(value+phase)*std::f32::consts::TAU;
        (f64::from(angle.cos())*0.5+0.5) as f32
    })
}
