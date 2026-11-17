use super::super::{Result, VoiceError, flat, materialized_flat, nofs};
use std::{fs::File, io::Read, path::{Path, PathBuf}, time::SystemTime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VocoderSource {
    R2File(PathBuf),
    R2Voice(PathBuf),
    R3File(PathBuf),
    R3Voice(PathBuf),
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
        if !metadata.is_file() { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
        Ok(Self { path, length: metadata.len(), modified: metadata.modified()? })
    }
    fn validate(&self) -> Result<()> {
        if Self::read(&self.path).as_ref().ok() != Some(self) { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ResolvedSource {
    models: Vec<Stamp>,
    manifest: Option<Stamp>,
    embedded: bool,
    pub r3: bool,
    external: bool,
}
impl ResolvedSource {
    pub fn resolve(source: &VocoderSource) -> Result<Self> {
        let mut manifest = None;
        let mut embedded = false;
        let r3 = matches!(source, VocoderSource::R3File(_) | VocoderSource::R3Voice(_));
        let paths = match source {
            VocoderSource::R2File(path) | VocoderSource::R3File(path) => vec![path.clone()],
            VocoderSource::R2Voice(path) => {
                let stamp = Stamp::read(path)?;
                if flat::is_profile(path) {
                    let model = flat::Profile::open(path)?.model("vocoder")?;
                    manifest = Some(stamp); vec![model]
                } else if materialized_flat::is_manifest(path) {
                    manifest = Some(stamp); vec![materialized_flat::vocoder_path(path)?]
                } else if nofs::is_file(path) {
                    embedded = true; vec![path.clone()]
                } else { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
            }
            VocoderSource::R3Voice(path) => {
                let mut paths = std::fs::read_dir(path)?.map(|entry| entry.map(|e| e.path())).collect::<std::io::Result<Vec<_>>>()?;
                paths.retain(|p| p.is_file() && p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("dnni")));
                paths.sort();
                if paths.is_empty() { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
                paths
            }
        };
        let models = paths.iter().map(|path| Stamp::read(path)).collect::<Result<Vec<_>>>()?;
        let resolved = Self { models, manifest, embedded, r3, external: matches!(source, VocoderSource::R3File(_)) };
        resolved.validate()?; Ok(resolved)
    }
    pub fn validate(&self) -> Result<()> {
        for model in &self.models { model.validate()?; }
        if let Some(manifest) = &self.manifest { manifest.validate()?; }
        Ok(())
    }
    pub fn prepare(&self) -> Result<(super::super::PreparedTree, String)> {
        self.validate()?;
        let result = if self.r3 {
            super::r3::prepare(&self.models.iter().map(|m| m.path.clone()).collect::<Vec<_>>(), self.external)?
        } else {
            let path = &self.models[0].path;
            let bytes = if self.embedded {
                let mut archive = nofs::NofsArchive::open(path)?;
                let description = nofs::describe(&mut archive)?;
                let index = description.models.iter().find(|(_, role)| *role == "vocoder")
                    .ok_or_else(|| VoiceError::new("VOCODER_OVERRIDE_INVALID"))?.0;
                archive.read(index, nofs::MAX_MODEL_BYTES)?
            } else { read_file(path)? };
            (super::super::PreparedTree::prepare_embedded_execution(&bytes, true)?, blake3::hash(&bytes).to_hex().to_string())
        };
        self.validate()?; Ok(result)
    }
}

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    if length == 0 || length > nofs::MAX_MODEL_BYTES as u64 { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
    let mut bytes = Vec::new();
    file.take(length + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != length { return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID")); }
    Ok(bytes)
}
