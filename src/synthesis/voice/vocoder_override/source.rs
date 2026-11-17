use super::super::{Result, VoiceError, flat, materialized_flat, nofs};
use std::{fs::File, io::Read, path::{Path, PathBuf}, time::SystemTime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum R2VocoderSource {
    File(PathBuf),
    Voice(PathBuf),
}

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
}

impl Stamp {
    fn read(path: &Path) -> Result<Self> {
        let path = path.canonicalize()?;
        let metadata = path.metadata()?;
        if !metadata.is_file() { return Err(VoiceError::new("R2_VOCODER_INVALID")); }
        Ok(Self { path, length: metadata.len(), modified: metadata.modified()? })
    }

    fn validate(&self) -> Result<()> {
        if Self::read(&self.path).as_ref().ok() != Some(self) {
            return Err(VoiceError::new("R2_VOCODER_INVALID"));
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ResolvedSource {
    model: Stamp,
    manifest: Option<Stamp>,
    embedded: bool,
}

impl ResolvedSource {
    pub fn resolve(source: &R2VocoderSource) -> Result<Self> {
        let (path, manifest, embedded) = match source {
            R2VocoderSource::File(path) => (path.clone(), None, false),
            R2VocoderSource::Voice(path) => {
                let stamp = Stamp::read(path)?;
                if flat::is_profile(path) {
                    let model = flat::Profile::open(path)?.model("vocoder")?;
                    (model, Some(stamp), false)
                } else if materialized_flat::is_manifest(path) {
                    (materialized_flat::vocoder_path(path)?, Some(stamp), false)
                } else if nofs::is_file(path) {
                    (path.clone(), None, true)
                } else {
                    return Err(VoiceError::new("R2_VOCODER_INVALID"));
                }
            }
        };
        let resolved = Self { model: Stamp::read(&path)?, manifest, embedded };
        resolved.validate()?;
        Ok(resolved)
    }

    pub fn validate(&self) -> Result<()> {
        self.model.validate()?;
        if let Some(manifest) = &self.manifest { manifest.validate()?; }
        Ok(())
    }

    pub fn read(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = if self.embedded {
            let mut archive = nofs::NofsArchive::open(&self.model.path)?;
            let description = nofs::describe(&mut archive)?;
            let index = description.models.iter().find(|(_, role)| *role == "vocoder")
                .ok_or_else(|| VoiceError::new("R2_VOCODER_INVALID"))?.0;
            archive.read(index, nofs::MAX_MODEL_BYTES)?
        } else {
            if self.model.length == 0 || self.model.length > nofs::MAX_MODEL_BYTES as u64 {
                return Err(VoiceError::new("R2_VOCODER_INVALID"));
            }
            let file = File::open(&self.model.path)?;
            let mut bytes = Vec::new();
            file.take(self.model.length + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 != self.model.length { return Err(VoiceError::new("R2_VOCODER_INVALID")); }
            bytes
        };
        self.validate()?;
        Ok(bytes)
    }
}
