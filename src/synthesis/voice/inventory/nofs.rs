use std::{collections::BTreeMap, path::Path, sync::OnceLock};

use super::{GraphSummary, LoadedModel, TensorSummary, VoiceFile, VoiceInventory, inspect};
use crate::synthesis::voice::{Result, VoiceError, embedded, nofs, tensor};

pub(super) fn load(path: &Path) -> Result<VoiceInventory> {
    let mut archive = nofs::NofsArchive::open(path)?;
    let description = nofs::describe(&mut archive)?;
    let mut graphs = Vec::new();
    let mut trees = Vec::new();
    let mut tensors = BTreeMap::<_, TensorSummary>::new();
    for (index, role) in description.models {
        let bytes = archive.read(index, nofs::MAX_MODEL_BYTES)?;
        let records = embedded::parse(&bytes)?;
        let root = records
            .last()
            .ok_or_else(|| VoiceError::new("SV1 model is empty"))?;
        let mut depths = Vec::<usize>::with_capacity(records.len());
        for record in &records {
            let depth = record.children.iter().try_fold(0usize, |depth, child| {
                depths
                    .get(*child)
                    .map(|child_depth| depth.max(child_depth + 1))
                    .ok_or_else(|| VoiceError::new("SV1 model is not in postorder"))
            })?;
            depths.push(depth);
            if tensor::is_known(&record.bytes) {
                inspect::count_tensor(&record.bytes, 0, &mut tensors)?;
            }
        }
        graphs.push(GraphSummary {
            resource_index: index,
            nodes: records.len(),
            root_children: root.children.len(),
            maximum_depth: *depths.last().expect("nonempty model"),
            role,
        });
        trees.push(bytes);
    }
    let graph_count = graphs.len();
    let tensor_count = tensors.values().map(|summary| summary.resources).sum();
    let resource_count = archive.records.len();
    let file = VoiceFile {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bytes: usize::try_from(archive.length)
            .map_err(|_| VoiceError::new("SV1 file size exceeds address space"))?,
        resources: resource_count,
        borrowed_resources: resource_count,
        encoded_resources: 0,

        header_signature_equal_branch_verified: false,
        index_digest_verified: false,
        all_resource_digests_verified: false,
        graphs,
        referenced_modules: graph_count,
        unreferenced_resources: resource_count - graph_count,
        tensors,
        configuration_parsed: true,
    };
    let mut inventory = VoiceInventory {
        database_id: nofs::database_id(path)?,
        directory: archive.path,
        display_name: description.configuration.name.clone(),
        source_identity: super::source_identity(path),
        resource_count,
        graph_count,
        tensor_count,
        files: vec![file],
        configuration: Some(description.configuration),
        _models: vec![LoadedModel::Nofs { trees }],
        preparation: OnceLock::new(),
    };
    let names = inventory.embedded_style_names()?;
    inventory.configuration.as_mut().expect("NOFS configuration")
        .resolve_embedded_styles(&names)?;
    Ok(inventory)
}
