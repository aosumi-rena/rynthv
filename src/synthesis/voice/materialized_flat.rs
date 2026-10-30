use super::{
    PreparedTree, PreparedVoice, Result, VoiceError, VoiceFamily,
    embedded::{self, types},
    preparation::{PreparedFile, PreparedModel},
};
use serde::Deserialize;
use std::{collections::BTreeSet, fs::File, io::Read, path::Path, sync::Arc};

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_MODEL_BYTES: usize = 128 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Manifest {
    format: String,
    family: String,
    engine_profile: String,
    name: String,
    #[serde(default)]
    voice_version:String,
    #[serde(default)]
    language:String,
    #[serde(default)]
    vendor:String,
    #[serde(default)]
    style_names:Vec<String>,
    f0_enhancement: f32,
    #[serde(default)]
    language_storage: LanguageStorage,
    models: Vec<Model>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LanguageStorage {
    #[default]
    Float32,
    Float16,
}

#[derive(Deserialize)]
struct Model {
    role: String,
    file: String,
    bytes: usize,
}

fn role(name: &str) -> Result<(&'static str, [u8; 16])> {
    match name {
        "duration" => Ok(("duration", types::DURATION_TYPE_ID)),
        "f0" => Ok(("f0", types::F0_WRAPPER_TYPE_ID)),
        "timbre" => Ok(("timbre", types::TIMBRE_TYPE_ID)),
        "vocoder" => Ok(("vocoder", types::VOCODER_TYPE_ID)),
        _ => Err(VoiceError::new("Unknown materialized Flat model role")),
    }
}

fn validate(manifest: &Manifest) -> Result<()> {
    if manifest.format != "svflat-original-materialized-dds-v1"
        || manifest.family != "Sv1"
        || manifest.engine_profile != "native-flat-1.3.4"
        || manifest.name.trim().is_empty()
        || !manifest.f0_enhancement.is_finite()
        || manifest.models.len() != 4
    {
        return Err(VoiceError::new("Invalid materialized Flat manifest contract"));
    }
    let mut roles = BTreeSet::new();
    let mut total = 0_usize;
    for model in &manifest.models {
        let (name, _) = role(&model.role)?;
        if !roles.insert(name)
            || model.file != format!("{name}.dnni")
            || !(28..=MAX_MODEL_BYTES).contains(&model.bytes)
        {
            return Err(VoiceError::new("Invalid, duplicated or nonlocal Flat model entry"));
        }
        total = total.checked_add(model.bytes)
            .ok_or_else(|| VoiceError::new("Flat model total size overflow"))?;
    }
    if total > MAX_TOTAL_BYTES {
        return Err(VoiceError::new("Materialized Flat models exceed 256 MiB"));
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(VoiceError::new("Materialized Flat input exceeds its size bound"));
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum || bytes.len() as u64 != metadata.len() {
        return Err(VoiceError::new("Materialized Flat input changed length during read"));
    }
    Ok(bytes)
}

pub fn load_materialized_flat(manifest_path: &Path) -> Result<PreparedVoice> {
    let manifest_path = manifest_path.canonicalize()?;
    let directory = manifest_path.parent()
        .ok_or_else(|| VoiceError::new("Materialized manifest has no parent"))?;
    let manifest: Manifest = serde_json::from_slice(&read_bounded(&manifest_path, MAX_MANIFEST_BYTES)?)
        .map_err(|error| VoiceError::new(format!("Invalid Flat manifest JSON: {error}")))?;
    validate(&manifest)?;
    let mut files = Vec::with_capacity(4);
    for model in &manifest.models {
        let (role, expected_type) = role(&model.role)?;
        let path = directory.join(&model.file).canonicalize()?;
        if path.parent() != Some(directory) {
            return Err(VoiceError::new("Materialized model target escapes its directory"));
        }
        let bytes = read_bounded(&path, model.bytes)?;
        if bytes.len() != model.bytes {
            return Err(VoiceError::new(format!("Materialized {role} bytes differ from manifest")));
        }

        if embedded::root_type(&bytes)? != expected_type {
            return Err(VoiceError::new(format!("Materialized {role} root has the wrong native type")));
        }
        let tree = PreparedTree::prepare_embedded_execution(
            &bytes, matches!(role, "f0" | "timbre" | "vocoder"))?;
        files.push(PreparedFile {
            name: model.file.clone(),
            trees: vec![PreparedModel { role, tree: Arc::new(tree) }],
        });
    }
    let mut voice = PreparedVoice::new(files, VoiceFamily::Sv1, Some(manifest.f0_enhancement), Vec::new())
        .with_dds_half_precision_language(matches!(manifest.language_storage, LanguageStorage::Float16))?;
    let tree=voice.model("timbre")?;
    let node=tree.node(tree.root_index())?.children[1];
    let count=tree.node(tree.node(node)?.children[0])?.children.len().checked_sub(1).ok_or_else(||VoiceError::new("Flat style map is empty"))?;
    let names=crate::synthesis::vocal_modes::ordered_style_names(tree,node,count)?;
    voice=voice.with_vocal_modes(Some(names))?;
    if let Some(error) = &voice.summary().stage_preparation_error {
        return Err(VoiceError::new(format!("Materialized Flat stage preparation: {error}")));
    }
    Ok(voice)
}

pub(super) fn metadata(path:&Path)->Result<super::config::VoiceConfiguration> {
    let manifest:Manifest=serde_json::from_slice(&read_bounded(path,MAX_MANIFEST_BYTES)?).map_err(|e|VoiceError::new(e.to_string()))?;
    validate(&manifest)?;
    let mut configuration=super::config::VoiceConfiguration::from_materialized(manifest.name,manifest.voice_version,manifest.vendor,manifest.language,manifest.f0_enhancement)?;
    let names=manifest.style_names.into_iter().filter(|n|!matches!(n.as_str(),"(base)"|"(default)")).collect::<Vec<_>>();
    configuration.styles=names.clone();configuration.resolve_embedded_styles(&names)?;Ok(configuration)
}

pub(super) fn is_manifest(path:&Path)->bool {
    path.file_name().is_some_and(|s|s.eq_ignore_ascii_case("manifest.json")) && path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            format: "svflat-original-materialized-dds-v1".into(), family: "Sv1".into(),
            voice_version:"1".into(),language:"japanese".into(),vendor:String::new(),
            style_names:Vec::new(),
            engine_profile: "native-flat-1.3.4".into(), name: "metadata-only-test".into(),
            language_storage: LanguageStorage::Float32,
            f0_enhancement: 1.0,
            models: ["duration", "f0", "timbre", "vocoder"].into_iter().map(|role| Model {
                role: role.into(), file: format!("{role}.dnni"), bytes: 28,
            }).collect(),
        }
    }

    #[test]
    fn accepts_only_complete_distinct_local_roles() {
        let mut value = manifest(); assert!(validate(&value).is_ok());
        value.models[3].role = "f0".into(); assert!(validate(&value).is_err());
        let mut value = manifest(); value.models.pop(); assert!(validate(&value).is_err());
        let mut value = manifest(); value.models[0].file = "../duration.dnni".into();
        assert!(validate(&value).is_err());
    }

    #[test]
    fn rejects_nonfinite_condition_wrong_family_and_size() {
        let mut value = manifest(); value.f0_enhancement = f32::NAN;
        assert!(validate(&value).is_err());
        let mut value = manifest(); value.family = "Sv2".into(); assert!(validate(&value).is_err());
        let mut value = manifest(); value.models[0].bytes = MAX_MODEL_BYTES + 1;
        assert!(validate(&value).is_err());
        let mut value = manifest(); value.models.iter_mut().for_each(|model| model.bytes = MAX_MODEL_BYTES);
        assert!(validate(&value).is_err());
    }
}
