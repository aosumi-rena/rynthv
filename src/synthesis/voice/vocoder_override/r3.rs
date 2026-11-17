use super::super::{
    PreparedTree, Result, VoiceError,
    archive::{Archive, ResourceId},
    format::FormatContext,
    tree::ModuleTree,
};
use crate::synthesis::stages::vocoder::configuration::{COMPACT_CHILDREN, ROOT_TYPE};
use std::{path::PathBuf, sync::Arc};

pub(super) fn prepare(paths: &[PathBuf], external: bool) -> Result<(PreparedTree, String)> {
    let mut selected = None;
    for path in paths {
        let bytes = super::source::read_file(path)?;
        let identity = blake3::hash(&bytes).to_hex().to_string();
        let tree = if bytes.get(..8) == Some(&[0xff, 0, 0xca, 0x7f, 3, 0, 0, 0]) {
            let context = FormatContext::dnni_v3();
            let (archive, unsigned) = if external {
                Archive::from_vocoder_file(bytes.into(), &context)?
            } else {
                (Archive::from_bytes(bytes.into(), &context)?, false)
            };
            let archive = Arc::new(archive);
            archive.verify_all()?;
            let graph = if external {
                let graph = external_graph(&archive)?;
                if unsigned && graph.root().children.len() != COMPACT_CHILDREN {
                    return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID"));
                }
                graph
            } else {
                let id = ResourceId(0xa95bea71e057b1ae3b7924462d6ca9aa_u128.to_le_bytes());
                let Some(index) = archive.index(id) else {
                    continue;
                };
                ModuleTree::parse(&archive.read(index, None)?, &archive)?
            };
            PreparedTree::prepare_execution(archive, &graph, false)?
        } else {
            PreparedTree::prepare_embedded_execution(&bytes, false)?
        };
        if selected.replace((tree, identity)).is_some() {
            return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID"));
        }
    }
    selected.ok_or_else(|| VoiceError::new("VOCODER_OVERRIDE_INVALID"))
}

fn external_graph(archive: &Archive) -> Result<ModuleTree> {
    let mut selected = None;
    for (index, resource) in archive.resources.iter().enumerate() {
        if resource.range.len() < 20 || !resource.range.len().is_multiple_of(20) {
            continue;
        }
        let Ok(graph) = ModuleTree::parse(&archive.read(index, None)?, archive) else {
            continue;
        };
        let root = archive
            .index(graph.root().resource)
            .expect("validated graph resource");
        if archive.prefix(root, 16)?.as_ref() != ROOT_TYPE {
            continue;
        }
        if selected.replace(graph).is_some() {
            return Err(VoiceError::new("VOCODER_OVERRIDE_INVALID"));
        }
    }
    selected.ok_or_else(|| VoiceError::new("VOCODER_OVERRIDE_INVALID"))
}
