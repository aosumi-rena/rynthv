use std::sync::Arc;

use crate::synthesis::{
    stages::f0::{
        TimedPhone,
        features::{FeatureRows, FrameFeatureInputs},
        frontend::FrontendNote,
    },
    voice::{Result, VoiceError},
};

use super::{
    PreparedTimbreFeatures, PreparedTimbreFrames, TimbreNetwork, TimbreOutput,
    records::{
        AcousticNote, AcousticPhone, FrameControl, PhoneImpulse, RecordsConfig, RecordsInput,
    },
    stream::TimbreStream,
};

pub struct TimbreRequest<'a> {
    pub dds:Option<&'a crate::synthesis::experiments::DdsFrameControls>,

    pub mode_coefficients: Option<&'a crate::synthesis::vocal_modes::VocalModeCoefficients>,
    pub notes: &'a [FrontendNote],
    pub phones: &'a [TimedPhone],
    pub midi_pitch: &'a [f32],
    pub note_modulation: &'a [AcousticNote],

    pub phone_impulses: Option<&'a [Option<PhoneImpulse>]>,
    pub frame_controls: Option<&'a [FrameControl]>,
    pub requested_frame: usize,
}

impl<'tree> TimbreNetwork<'tree> {

    pub fn render(
        &self,
        request: &TimbreRequest<'_>,
        config: &RecordsConfig,
    ) -> Result<TimbreOutput> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("timbre.frontend_render");
        let features = self.prepare_features(request.notes, request.phones)?;
        self.render_prepared(request, config, &features)
    }

    pub fn render_prepared(
        &self,
        request: &TimbreRequest<'_>,
        config: &RecordsConfig,
        features: &PreparedTimbreFeatures,
    ) -> Result<TimbreOutput> {
        self.render_prepared_until(request, config, features, request.midi_pitch.len())
    }

    pub fn render_prepared_until(
        &self,
        request: &TimbreRequest<'_>,
        config: &RecordsConfig,
        features: &PreparedTimbreFeatures,
        end_frame: usize,
    ) -> Result<TimbreOutput> {
        #[cfg(feature = "synth-profile")]
        let _profile = crate::synthesis::profiling::span("timbre.render_prepared");
        let frames =
            Arc::new(self.prepare_frame_features_with_defaults(
                request.notes, request.phones, features, request.mode_coefficients,request.dds.and_then(|d|d.default_weights.as_deref()))?);
        self.start_prepared(request, config, frames)?
            .collect_until(end_frame)
    }

    pub fn prepare_frame_features(
        &self,
        notes: &[FrontendNote],
        phones: &[TimedPhone],
        features: &PreparedTimbreFeatures,
    ) -> Result<PreparedTimbreFrames> {
        self.prepare_frame_features_with_modes(notes, phones, features, None)
    }

    pub fn prepare_frame_features_with_modes(
        &self,
        notes: &[FrontendNote],
        phones: &[TimedPhone],
        features: &PreparedTimbreFeatures,
        modes: Option<&crate::synthesis::vocal_modes::VocalModeCoefficients>,
    ) -> Result<PreparedTimbreFrames> {
        self.prepare_frame_features_with_defaults(notes,phones,features,modes,None)
    }

    fn prepare_frame_features_with_defaults(
        &self,notes:&[FrontendNote],phones:&[TimedPhone],features:&PreparedTimbreFeatures,
        modes:Option<&crate::synthesis::vocal_modes::VocalModeCoefficients>,defaults:Option<&[f32]>,
    )->Result<PreparedTimbreFrames> {
        let input = self.feature_inputs(notes, phones)?;
        let feature_config = input.network.configuration();
        if feature_config.frame_channels != 128 {
            return Err(VoiceError::new(
                "Timbre frame feature width differs from its program",
            ));
        }
        if feature_config.condition_channels != 0
            && feature_config.condition_channels != self.styles.neutral.len()
        {
            return Err(VoiceError::new(
                "Timbre frame style differs from feature condition width",
            ));
        }
        if features.note_count != notes.len()
            || features.phone_count != phones.len()
            || features.note_channels != feature_config.note_channels
            || features.phone_channels != feature_config.phone_channels
            || features.note_count.checked_mul(features.note_channels) != Some(features.notes.len())
            || features.phone_count.checked_mul(features.phone_channels)
                != Some(features.phones.len())
        {
            return Err(VoiceError::new(
                "Prepared timbre feature dimensions differ from the request or model",
            ));
        }
        let convert = |values: &[usize]| -> Result<Vec<u32>> {
            values
                .iter()
                .map(|&value| {
                    u32::try_from(value)
                        .map_err(|_| VoiceError::new("Timbre frame index exceeds u32"))
                })
                .collect()
        };
        let note_durations = convert(&input.notes.frame_counts)?;
        let positions = convert(&input.notes.frame_positions)?;
        let style = self.feature_style(notes.len())?;
        let frame_count = input.notes.frame_counts.iter().sum();
        let mode_styles = self.mode_styles(modes,0..frame_count,defaults)?;
        let frame_style = (feature_config.condition_channels != 0).then(||
            mode_styles.as_ref().map_or_else(||self.styles.neutral.repeat(frame_count),
                |styles|styles.primary.values.clone()));
        let expanded = input.network.expand_encoding(
            &FrameFeatureInputs {
                notes: &input.normalized_notes,
                style: style.as_deref(),
                encoding: features,
                note_durations: &note_durations,
                note_positions: &positions,
                phone_durations: &input.phone_durations,
            },
            frame_style.as_ref().map(|values| FeatureRows {
                rows: frame_count,
                width: feature_config.condition_channels,
                values,
            }),
        )?;
        Ok(PreparedTimbreFrames::new(
            expanded,
            feature_config.frame_channels,
            input.notes.frame_counts,
        ))
    }

    pub fn start_prepared<'network>(
        &'network self,
        request: &TimbreRequest<'_>,
        config: &RecordsConfig,
        features: Arc<PreparedTimbreFrames>,
    ) -> Result<TimbreStream<'network, 'tree>> {
        if request.notes.len() != request.note_modulation.len()
            || request.notes.len() != features.note_frames.len()
            || request
                .phone_impulses
                .is_some_and(|values| values.len() != request.phones.len())
        {
            return Err(VoiceError::new(
                "Timbre modulation counts differ from notes/phones",
            ));
        }
        if request
            .note_modulation
            .iter()
            .zip(&features.note_frames)
            .any(|(note, &count)| note.frames != count)
            || features.len() != request.midi_pitch.len()
        {
            return Err(VoiceError::new(
                "Timbre notes, phones and F0 do not cover the same frames",
            ));
        }
        let acoustic_phones: Vec<_> = request
            .phones
            .iter()
            .enumerate()
            .map(|(index, phone)| AcousticPhone {
                token: &phone.token,
                frames: phone.frames,
                impulse: request.phone_impulses.and_then(|values| values[index]),
            })
            .collect();
        let config = RecordsConfig {
            retained_noise_only: self.retained_noise_only,
            stride: self.layout.stride,
            ppg_width: self.layout.ppg_noise_channels,
            timbre_width: self.layout.timbre_noise_channels,
            ..*config
        };
        let records = config.prepare(RecordsInput {
            phones: &acoustic_phones,
            notes: request.note_modulation,
            midi_pitch: request.midi_pitch,
            controls: request.frame_controls,
            requested_frame: request.requested_frame,
        })?;
        let styles = self.mode_styles(request.mode_coefficients,records.low_rate_positions.iter().copied(),request.dds.and_then(|d|d.default_weights.as_deref()))?;
        TimbreStream::from_prepared(self, features, &records, styles.as_ref())
    }

    fn mode_styles(
        &self,
        modes: Option<&crate::synthesis::vocal_modes::VocalModeCoefficients>,
        positions: impl IntoIterator<Item=usize>,
        defaults:Option<&[f32]>,
    ) -> Result<Option<super::styles::AcousticStyles>> {
        if modes.is_none() && defaults.is_none() {return Ok(None);}
        let style_node = crate::synthesis::voice::speakers::style(
            self.tree, self.layout.styles, self.speaker)?;
        let names = crate::synthesis::vocal_modes::ordered_style_names(
            self.tree, style_node, self.styles.variants.len())?;
        let mut primary=Vec::new();let mut secondary=Vec::new();

        let constant=modes.filter(|m|!m.animated()).map(|modes|Ok::<_,VoiceError>((
            self.styles.blend(&modes.weights(&names,true))?,
            self.styles.blend(&modes.weights(&names,false))?))).transpose()?;
        for frame in positions {
            let (mut a,mut b)=if let Some((a,b))=&constant {(a.clone(),b.clone())}
                else if let Some(modes)=modes {(self.styles.blend(&modes.weights_at(&names,true,frame))?,self.styles.blend(&modes.weights_at(&names,false,frame))?)}
                else {(self.styles.neutral.to_vec(),self.styles.neutral.to_vec())};
            if let Some(defaults)=defaults {
                let weight=*defaults.get(frame).filter(|v|v.is_finite()).ok_or_else(||VoiceError::new("Default acoustic style frames are not aligned"))?;
                if weight!=1.0 {for row in [&mut a,&mut b] {for (value,&neutral) in row.iter_mut().zip(self.styles.neutral.iter()) {*value+=neutral*(weight-1.0);}}}
            }
            primary.extend(a);secondary.extend(b);
        }
        Ok(Some(super::styles::AcousticStyles {
            primary:crate::synthesis::inference::Sequence::from_values(self.styles.neutral.len(),primary)?,
            secondary:crate::synthesis::inference::Sequence::from_values(self.styles.neutral.len(),secondary)?,
        }))
    }
}
