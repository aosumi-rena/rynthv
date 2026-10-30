use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

use super::{GraphSummary, LoadedTree, TensorSummary, VoiceFile};
use crate::synthesis::voice::{
    Result, VoiceError,
    archive::{Archive, ResourceId},
    config::CONFIGURATION_ID,
    embedded, tensor,
    tree::ModuleTree,
    u32_at,
};

pub(super) fn file(path: &Path, archive: &Archive) -> Result<(VoiceFile, Vec<LoadedTree>)> {
    let mut tensors = BTreeMap::<_, TensorSummary>::new();
    let mut graphs = Vec::new();
    let mut trees = Vec::new();
    let mut references = HashSet::new();
    for (index, resource) in archive.resources.iter().enumerate() {
        let prefix = archive.prefix(index, 32)?;
        if tensor::is_known(&prefix) {
            count_tensor(&archive.read(index, None)?, resource.flag, &mut tensors)?;
        }
        if prefix.starts_with(&[0xff, 0, 0xca, 0x7f]) {
            let role = embedded_role(resource.id)
                .ok_or_else(|| VoiceError::new("Unidentified embedded voice model resource"))?;
            let records = embedded::parse(&archive.read(index, None)?)?;
            let root = records
                .last()
                .ok_or_else(|| VoiceError::new("Embedded model tree is empty"))?;
            let mut depths = Vec::<usize>::with_capacity(records.len());
            for record in &records {
                let depth = record.children.iter().try_fold(0, |depth, child| {
                    depths
                        .get(*child)
                        .map(|child_depth| depth.max(child_depth + 1))
                        .ok_or_else(|| VoiceError::new("Embedded model child is not in postorder"))
                })?;
                depths.push(depth);
                if tensor::is_known(&record.bytes) {
                    count_tensor(&record.bytes, resource.flag, &mut tensors)?;
                }
            }
            graphs.push(GraphSummary {
                resource_index: index,
                nodes: records.len(),
                root_children: root.children.len(),
                maximum_depth: *depths.last().expect("nonempty model"),
                role,
            });
            trees.push(LoadedTree::Embedded {
                resource_index: index,
            });
            references.insert(resource.id);
            continue;
        }
        if embedded_role(resource.id).is_some() {
            return Err(VoiceError::new(
                "Embedded voice model lacks its DNNI header",
            ));
        }
        let length = resource.range.len();
        let role = indexed_role(resource.id);
        let known_graph = role != "unconfirmed";
        let tree_candidate = resource.flag == 1
            && length >= 20
            && length.is_multiple_of(20)
            && u32_at(&prefix, 0)? as usize <= length / 20;
        if !tree_candidate && !known_graph {
            continue;
        }

        let tree = match ModuleTree::parse(&archive.read(index, None)?, archive) {
            Ok(tree) => tree,
            Err(error) if known_graph => return Err(error),
            Err(_) => continue,
        };
        for node in &tree.nodes {
            let module = archive
                .index(node.resource)
                .expect("tree parser resolves IDs");
            if archive.resources[module].range.len() < 16 {
                return Err(VoiceError::new("Module resource lacks its type prefix"));
            }
            references.insert(node.resource);
        }
        graphs.push(GraphSummary {
            resource_index: index,
            nodes: tree.nodes.len(),
            root_children: tree.root().children.len(),
            maximum_depth: tree.maximum_depth,
            role,
        });
        trees.push(LoadedTree::Indexed(tree));
    }
    let borrowed_resources = archive
        .resources
        .iter()
        .filter(|resource| resource.flag == 0)
        .count();
    let file = VoiceFile {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bytes: archive.file_bytes(),
        resources: archive.resources.len(),
        borrowed_resources,
        encoded_resources: archive.resources.len() - borrowed_resources,
        header_signature_equal_branch_verified: true,
        index_digest_verified: true,
        all_resource_digests_verified: true,
        graphs,
        referenced_modules: references.len(),
        unreferenced_resources: archive.resources.len() - references.len(),
        tensors,
        configuration_parsed: archive.index(CONFIGURATION_ID).is_some(),
    };
    Ok((file, trees))
}

pub(super) fn count_tensor(
    bytes: &[u8],
    flag: u32,
    tensors: &mut BTreeMap<&'static str, TensorSummary>,
) -> Result<()> {
    let layout = tensor::parse(bytes)?;
    let summary = tensors.entry(layout.storage).or_default();
    summary.resources += 1;
    summary.borrowed_resources += usize::from(flag == 0);
    summary.decoded_resources += usize::from(flag == 1);
    summary.elements += layout.elements;
    summary.scales += layout.scales;
    Ok(())
}

fn embedded_role(id: ResourceId) -> Option<&'static str> {
    match u128::from_le_bytes(id.0) {
        0xf82b01e4d6811e3929adc88975d18bdd => Some("f0"),
        0xf1ae8760a5d09483514e0211f299af0f => Some("timbre"),
        _ => None,
    }
}

fn indexed_role(id: ResourceId) -> &'static str {
    match u128::from_le_bytes(id.0) {
        0xbc72d4467151f7763681c6765678bb13 => "duration",
        0x34620edf25443fa373c2ee09ae6de624 => "f0",
        0xa3d15b952e72382824a13092648ea34e => "timbre",
        0xa95bea71e057b1ae3b7924462d6ca9aa => "vocoder",
        _ => "unconfirmed",
    }
}
