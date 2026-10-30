use super::{PreparedTree, PreparedTreeSummary, Result, VoiceError, VoiceFamily};
use serde::Serialize;
use std::sync::{Arc, OnceLock};

pub struct PreparedVoice {
    files: Vec<PreparedFile>,
    pub family: VoiceFamily,
    pub f0_enhancement: Option<f32>,
    pub(crate) dds_half_precision_language: bool,
    pub(crate) retained_acoustic_noise_only: bool,
    pub(crate) vocal_modes: Option<Vec<String>>,
    pub speakers: Vec<super::Speaker>,
    pub member_vocal_modes: Arc<[super::MemberVocalModes]>,
    pub(crate) speaker: Option<usize>,
    summary: OnceLock<PreparationSummary>,
}

#[derive(Clone)]
pub(super) struct PreparedFile {
    pub name: String,
    pub trees: Vec<PreparedModel>,
}

#[derive(Clone)]
pub(super) struct PreparedModel {
    pub role: &'static str,
    pub tree: Arc<PreparedTree>,
}

#[derive(Serialize)]
pub struct PreparationSummary {
    pub files: Vec<PreparedFileSummary>,

    pub stage_programs_available: bool,
    pub stage_preparation_error: Option<String>,
}

#[derive(Serialize)]
pub struct PreparedFileSummary {
    pub name: String,
    pub trees: Vec<PreparedModelSummary>,
}

#[derive(Serialize)]
pub struct PreparedModelSummary {
    pub role: &'static str,
    pub modules: PreparedTreeSummary,
}

impl PreparedVoice {
    pub(super) fn new(
        files: Vec<PreparedFile>,
        family: VoiceFamily,
        f0_enhancement: Option<f32>,
        speakers: Vec<super::Speaker>,
    ) -> Self {
        let model = Self {
            files,
            family,
            f0_enhancement,
            dds_half_precision_language: false,
            retained_acoustic_noise_only: false,
            vocal_modes: None,
            speakers,
            member_vocal_modes: Arc::from([]),
            speaker: None,
            summary: OnceLock::new(),
        };

        model
    }

    pub fn summary(&self) -> &PreparationSummary {
        self.summary.get_or_init(|| {
            let stage_preparation_error =
                self.prepare_stages().err().map(|error| error.to_string());
            PreparationSummary {
                files: self
                    .files
                    .iter()
                    .map(|file| PreparedFileSummary {
                        name: file.name.clone(),
                        trees: file
                            .trees
                            .iter()
                            .map(|model| PreparedModelSummary {
                                role: model.role,
                                modules: model.tree.summary(),
                            })
                            .collect(),
                    })
                    .collect(),
                stage_programs_available: stage_preparation_error.is_none(),
                stage_preparation_error,
            }
        })
    }

    pub(super) fn with_vocal_modes(mut self, names: Option<Vec<String>>) -> Result<Self> {
        self.vocal_modes = names;
        self.member_vocal_modes = super::member_modes::prepare(&self)?.into();
        self.summary();
        Ok(self)
    }

    pub fn with_dds_half_precision_language(mut self, enabled: bool) -> Result<Self> {
        if self.family != VoiceFamily::Sv1 {
            return Err(VoiceError::new("Language precision requires a native DDS voice"));
        }
        self.dds_half_precision_language = enabled;
        self.summary = OnceLock::new();
        Ok(self)
    }

    pub(super) fn with_acoustic_version(mut self, version: &str) -> Self {
        self.retained_acoustic_noise_only = self.family == VoiceFamily::Sv2
            && super::noise_cadence::retained_only(version);
        self
    }

    fn prepare_stages(&self) -> Result<()> {
        use crate::synthesis::stages::{
            compatible_duration::CompatibleDurationStage, f0::F0Stage, sv1_f0::Sv1F0Stage,
            sv1_timbre::Sv1TimbreNetwork, timbre::TimbreNetwork, vocoder::VocoderStage,
        };
        if !self.speakers.is_empty() {
            for (role, slot) in [("duration", 1), ("f0", 2), ("timbre", 4), ("timbre", 5)] {
                let tree = self.model(role)?;
                let root = tree.node(tree.root_index())?;
                let node = *root
                    .children
                    .get(slot)
                    .ok_or_else(|| VoiceError::new("Choir model has no speaker conditioning"))?;
                if !matches!(tree.tensor(node)?, super::TensorStorage::FloatMatrix { rows, .. } if *rows == self.speakers.len())
                {
                    return Err(VoiceError::new(
                        "Choir speaker count differs between metadata and model stages",
                    ));
                }
            }
            for speaker in &self.speakers {
                self.member(speaker)?.prepare_stages()?;
            }
            return Ok(());
        }
        let duration = self.model("duration")?;
        match self.family {
            VoiceFamily::Sv1 => {
                crate::synthesis::stages::sv1_duration::Sv1DurationStage::prepare(duration)?;
                let enhancement = self.f0_enhancement.ok_or_else(|| {
                    VoiceError::new(
                        "SV1 voice is missing the researched F0 enhancement configuration",
                    )
                })?;
                Sv1F0Stage::prepare_native(self.model("f0")?, enhancement)?;
                Sv1TimbreNetwork::prepare_native_with_language_precision(
                    self.model("timbre")?, self.dds_half_precision_language)?;
            }
            VoiceFamily::Sv2Compatible => {
                CompatibleDurationStage::validate(duration)?;
                Sv1F0Stage::prepare(self.model("f0")?)?;
                Sv1TimbreNetwork::prepare(self.model("timbre")?)?;
            }
            VoiceFamily::Sv2 => {
                let root = duration.node(duration.root_index())?;
                if root.type_id != super::DURATION_ROOT_TYPE_ID || root.children.len() != 14 {
                    return Err(VoiceError::new("Unsupported duration stage structure"));
                }
                super::DurationConfiguration::parse(
                    &duration.resource_bytes(duration.root_index())?,
                )?;
                super::speakers::vector(duration, root.children[1], self.speaker)?;
                F0Stage::prepare(self.model("f0")?, self.speaker)?;
                TimbreNetwork::prepare(self.model("timbre")?, self.speaker, self.retained_acoustic_noise_only)?;
            }
        }
        VocoderStage::prepare(self.model("vocoder")?)?;
        Ok(())
    }

    pub fn member(&self, speaker: &super::Speaker) -> Result<Self> {
        if self
            .speakers
            .get(speaker.id)
            .is_none_or(|entry| entry.name != speaker.name || entry.part != speaker.part || entry.gain_db.to_bits()!=speaker.gain_db.to_bits())
        {
            return Err(VoiceError::new("Speaker is not a member of this choir"));
        }
        let catalogue=self.member_vocal_modes.iter().find(|m|m.speaker_id==speaker.id)
            .ok_or_else(||VoiceError::new("Choir member vocal-mode catalogue has not been prepared"))?;
        Ok(Self {
            files: self.files.clone(),
            family: self.family,
            f0_enhancement: self.f0_enhancement,
            dds_half_precision_language: self.dds_half_precision_language,
            retained_acoustic_noise_only: self.retained_acoustic_noise_only,
            vocal_modes: Some(catalogue.public_names.clone()),
            member_vocal_modes: Arc::clone(&self.member_vocal_modes),
            speakers: Vec::new(),
            speaker: Some(speaker.id),
            summary: OnceLock::new(),
        })
    }

    pub fn selected_speaker(&self) -> Option<usize> {self.speaker}

    pub(crate) fn with_vocoder(&self,source:&Self)->Result<Self> {
        if source.family!=self.family {return Err(VoiceError::new("Vocoder selection requires matching voice generations"));}
        let tree=source.shared_model("vocoder")?;
        let mut files=self.files.clone();
        for model in files.iter_mut().flat_map(|f|&mut f.trees) {if model.role=="vocoder" {model.tree=Arc::clone(&tree);}}
        Ok(Self {files,family:self.family,f0_enhancement:self.f0_enhancement,
            dds_half_precision_language:self.dds_half_precision_language,retained_acoustic_noise_only:self.retained_acoustic_noise_only,
            vocal_modes:self.vocal_modes.clone(),speakers:self.speakers.clone(),member_vocal_modes:Arc::clone(&self.member_vocal_modes),
            speaker:self.speaker,summary:OnceLock::new()})
    }

    pub fn model(&self, role: &str) -> Result<&PreparedTree> {
        Ok(self.find_model(role)?.tree.as_ref())
    }

    pub(crate) fn shared_model(&self, role: &str) -> Result<Arc<PreparedTree>> {
        Ok(Arc::clone(&self.find_model(role)?.tree))
    }

    fn find_model(&self, role: &str) -> Result<&PreparedModel> {
        let mut models = self
            .files
            .iter()
            .flat_map(|file| &file.trees)
            .filter(|model| model.role == role);
        let model = models
            .next()
            .ok_or_else(|| VoiceError::new(format!("声库缺少本地 {role} 模型")))?;
        if models.next().is_some() {
            return Err(VoiceError::new(format!(
                "声库包含多个 {role} 模型，无法确定执行目标"
            )));
        }
        Ok(model)
    }

    pub fn tree(&self, file: usize, tree: usize) -> Result<&PreparedTree> {
        self.files
            .get(file)
            .and_then(|file| file.trees.get(tree))
            .map(|model| model.tree.as_ref())
            .ok_or_else(|| VoiceError::new("Prepared model file/tree index is out of range"))
    }

    pub fn execute_module(
        &self,
        file: usize,
        tree: usize,
        node: usize,
        input: &[f32],
    ) -> Result<Vec<f32>> {
        self.tree(file, tree)?.execute_node(node, input)
    }
}
