use serde::Serialize;

use super::{PreparedTree, Result, TensorStorage, VoiceError};

pub(crate) const STYLE_TYPE: [u8; 16] = [
    0x04, 0x12, 0x46, 0xa2, 0xe5, 0x5c, 0xb3, 0x12, 0x2b, 0xb6, 0xb8, 0x27, 0x1c, 0x2a, 0x10, 0xff,
];
pub(crate) const LIST_TYPE: [u8; 16] = [
    0x48, 0xe2, 0xd2, 0x83, 0xee, 0xc6, 0x08, 0x1c, 0xb5, 0x6f, 0x7f, 0xb7, 0x9b, 0x51, 0xb9, 0xc3,
];

#[derive(Clone, Debug, Serialize)]
pub struct Speaker {
    pub id: usize,
    pub name: String,
    pub part: String,
    pub gain_db: f32,
}

impl Speaker {
    pub(super) fn parse(text: &str) -> Result<Vec<Self>> {
        let mut speakers = Vec::new();
        for entry in text.split(',') {
            let mut fields = entry.split(':');
            let name = fields
                .next()
                .filter(|name| !name.is_empty())
                .ok_or_else(invalid)?;
            let mut id = None;
            let mut part = None;
            let mut gain = None;
            for field in fields {
                let (key, value) = field.split_once('=').ok_or_else(invalid)?;
                match key {
                    "id" if id.is_none() => {
                        id = Some(value.parse::<usize>().map_err(|_| invalid())?)
                    }
                    "part" if part.is_none() && !value.is_empty() => part = Some(value.to_owned()),
                    "gain" if gain.is_none() => {
                        gain = Some(value.parse::<f32>().map_err(|_| invalid())?)
                    }
                    _ => return Err(invalid()),
                }
            }
            let gain_db = gain.ok_or_else(invalid)?;
            if id != Some(speakers.len())
                || !gain_db.is_finite()
                || !10.0_f32.powf(gain_db / 20.0).is_finite()
            {
                return Err(invalid());
            }
            speakers.push(Self {
                id: id.unwrap(),
                name: name.to_owned(),
                part: part.ok_or_else(invalid)?,
                gain_db,
            });
        }
        Ok(speakers)
    }
}

pub(crate) fn vector(tree: &PreparedTree, node: usize, speaker: Option<usize>) -> Result<&[f32]> {
    let tensor = tree.tensor(node)?;
    match (tensor, speaker) {
        (TensorStorage::FloatVector { values }, None) => Ok(values),
        (
            TensorStorage::FloatMatrix {
                rows,
                columns,
                values,
                ..
            },
            Some(index),
        ) if index < *rows => Ok(&values[index * columns..(index + 1) * columns]),
        _ => {
            let layout = tensor.layout();
            let selection = speaker.map_or_else(|| "single voice".to_owned(), |id| format!("speaker {id}"));
            Err(VoiceError::new(format!(
                "Conditioning node {node} has {} {}×{} storage for {selection}",
                layout.storage, layout.rows, layout.columns,
            )))
        }
    }
}

pub(crate) fn style(tree: &PreparedTree, node: usize, speaker: Option<usize>) -> Result<usize> {
    let module = tree.node(node)?;
    match (module.type_id, speaker) {
        (STYLE_TYPE, None) => Ok(node),
        (LIST_TYPE, Some(index)) => module
            .children
            .get(index)
            .copied()
            .ok_or_else(|| VoiceError::new("Selected speaker has no vocal-mode basis")),
        _ => Err(VoiceError::new(
            "Vocal-mode ownership does not match the selected speaker",
        )),
    }
}

pub(crate) fn style_names(bytes: &[u8]) -> Result<Vec<String>> {
    let mut offset = 0;
    let read = |offset: &mut usize| -> Result<usize> {
        let end = offset
            .checked_add(4)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(invalid)?;
        let value = u32::from_le_bytes(bytes[*offset..end].try_into().unwrap()) as usize;
        *offset = end;
        Ok(value)
    };
    let count = read(&mut offset)?;
    if count > bytes.len() / 4 {
        return Err(invalid());
    }
    let mut names = Vec::new();
    for _ in 0..count {
        let length = read(&mut offset)?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(invalid)?;
        let name = std::str::from_utf8(&bytes[offset..end]).map_err(|_| invalid())?;
        if name.is_empty() || names.iter().any(|other| other == name) {
            return Err(invalid());
        }
        names.push(name.to_owned());
        offset = end;
    }
    if offset != bytes.len() {
        return Err(invalid());
    }
    Ok(names)
}

fn invalid() -> VoiceError {
    VoiceError::new("Invalid speaker or vocal-mode configuration")
}
