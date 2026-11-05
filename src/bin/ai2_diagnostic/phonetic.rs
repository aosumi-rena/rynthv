//! Bounded original-weight checkpoint of the named-descriptor phone encoder.
use rynthv::synthesis::{
    stages::timbre::TimbreLayout,
    voice::{PHONETIC_MAP_TYPE_ID, PhoneticMap, PreparedModule, VoiceInventory},
};
use serde::Deserialize;
use serde_json::json;
use std::{ffi::OsString, fs, io::Write, path::PathBuf};

#[derive(Deserialize)]
pub(super) struct Phone {
    pub(super) token: String,
    pub(super) language: String,
    pub(super) note: usize,
}
#[derive(Deserialize)]
struct Input {
    phones: Vec<Phone>,
    #[serde(default)]
    notes: Vec<Note>,
}
#[derive(Deserialize)]
pub(super) struct Note {
    pub(super) phones: Vec<String>,
    pub(super) pitch: f32,
    pub(super) duration_seconds: f64,
    pub(super) kind: u32,
    pub(super) flag: bool,
}

pub(super) fn run(args: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let with_context = args.len() == 4 && args[3] == "--context";
    if args.len() != 3 && !with_context {
        return Err("Usage: --phonetic VOICE PHONE_SEQUENCE_REPORT NEW_REPORT [--context]".into());
    }
    let path = PathBuf::from(&args[2]);
    if path.exists() {
        return Err("Phonetic checkpoint already exists".into());
    }
    let bytes = fs::read(&args[1])?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("Phone sequence report exceeds checkpoint bound".into());
    }
    let input: Input = serde_json::from_slice(&bytes)?;
    if input.phones.is_empty() || input.phones.len() > 256 {
        return Err("Phonetic checkpoint requires 1..256 phones".into());
    }
    let inventory = VoiceInventory::load(&PathBuf::from(&args[0]))?;
    let voice = inventory.prepare()?;
    let mut roles = serde_json::Map::new();
    for role in ["f0", "timbre"] {
        let tree = voice.model(role)?;
        let root = tree.node(tree.root_index())?;
        let (frontend, features) = if role == "f0" {
            if root.children.len() != 5 {
                return Err("Expected five-child F0 root".into());
            }
            (root.children[0], root.children[3])
        } else {
            let layout = TimbreLayout::prepare(tree, None)?;
            (layout.frontend, layout.feature_network)
        };
        let map_node = *tree
            .node(frontend)?
            .children
            .first()
            .ok_or("Missing phonetic map")?;
        if tree.node(map_node)?.type_id != PHONETIC_MAP_TYPE_ID {
            return Err("Expected named phonetic frontend".into());
        }
        let map = PhoneticMap::parse(&tree.resource_bytes(map_node)?)?;
        let projection = *tree
            .node(features)?
            .children
            .first()
            .ok_or("Missing phone projection")?;
        match &tree.node(projection)?.module {
            PreparedModule::Affine(layer)
                if layer.input_features == 73 && layer.output_features == 32 => {}
            _ => return Err("Phonetic checkpoint requires original 73-to-32 affine".into()),
        }
        let mut values = Vec::with_capacity(input.phones.len() * 73);
        for phone in &input.phones {
            values.extend(map.encode(&phone.language, &phone.token)?);
        }
        let projected = tree.execute_node(projection, &values)?;
        let mut row = json!({"map_entries":map.len(),"rows":input.phones.len(),
            "input_width":73,"input_values":values,"projection_width":32,"projection_values":projected});
        if with_context {
            row["context"] = super::phonetic_context::run(
                tree,
                frontend,
                features,
                &input.notes,
                &input.phones,
                &map,
                &projected,
            )?;
        }
        roles.insert(role.into(), row);
    }
    let result = json!({"scope":"Original metadata and source arithmetic, then original-weight phone projection only; no duration/shared-context/audio parity claim",
        "phone_sequence_report":args[1].to_string_lossy(),"voice":args[0].to_string_lossy(),"roles":roles});
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&serde_json::to_vec(&result)?)?;
    Ok(())
}
