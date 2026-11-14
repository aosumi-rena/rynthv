use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const VERSION: u32 = 2;
pub const DOCUMENT_LIMIT: usize = 16 * 1024 * 1024;
pub const INPUT_LIMIT: usize = 1024 * 1024;
pub const FRAME_LIMIT: usize = 64 * 1024 * 1024;
pub const MIN_SIZE: [f32; 2] = [860.0, 560.0];

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct State {
    #[serde(with = "crate::model::shared_json")]
    pub document: std::sync::Arc<serde_json::Value>,
    pub pregain_db: f32,
    pub pitch_semitones: f32,
    #[serde(default)]
    pub has_target: bool,
}

#[derive(Deserialize, Serialize)]
pub struct Open {
    pub version: u32,
    pub state: Option<State>,
}

#[derive(Deserialize, Serialize)]
pub struct Input {
    pub size: [f32; 2],
    pub scale: f32,
    pub focused: bool,
    pub events: Vec<Event>,
    pub files: Vec<std::path::PathBuf>,
}

#[derive(Deserialize, Serialize)]
pub enum Event {
    Move([f32; 2]),
    Motion([f32; 2]),
    Button([f32; 2], u8, bool, u8),
    Leave,
    Scroll([f32; 2], u8, u8, u8),
    Zoom(f32),
    Rotate(f32),
    Key(String, Option<String>, bool, bool, u8),
    Modifiers(u8),
    Copy,
    Cut,
    Paste(String),
    Text(String),
    Preedit(String, Option<std::ops::Range<usize>>),
    Commit(String),
    DeleteSurrounding(usize, usize),
    Touch(u64, u64, u8, [f32; 2], Option<f32>),
}

#[derive(Default, Deserialize, Serialize)]
pub struct Frame {
    pub error: Option<String>,
    pub size: [f32; 2],
    pub state: Option<State>,
    #[serde(default)]
    pub unsettled: bool,
    #[serde(default)]
    pub busy: bool,
    #[serde(default)]
    pub reuse_picture: bool,
    pub textures: Vec<Texture>,
    pub free: Vec<u64>,
    pub meshes: Vec<Mesh>,
    pub cursor: usize,
    pub clipboard: Option<String>,
    pub ime: Option<([f32; 4], [f32; 4], bool)>,
    pub wait_ms: Option<u32>,
}

#[derive(Deserialize, Serialize)]
pub struct Texture {
    pub id: u64,
    pub size: [usize; 2],
    pub offset: Option<[usize; 2]>,
    pub sampling: [u8; 4],
}

#[derive(Deserialize, Serialize, PartialEq)]
pub struct Mesh {
    pub id: u64,
    pub clip: [f32; 4],
    pub vertices: usize,
    pub indices: usize,
}

pub fn read(r: &mut impl Read, max: usize) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0; 4];
    if r.read(&mut len[..1])? == 0 { return Ok(None); }
    r.read_exact(&mut len[1..])?;
    let len = u32::from_le_bytes(len) as usize;
    if len > max { return Err(io::Error::new(io::ErrorKind::InvalidData, "Rocoflex message is too large")); }
    let mut bytes = vec![0; len];
    r.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

pub fn write(w: &mut impl Write, bytes: &[u8], max: usize) -> io::Result<()> {
    if bytes.len() > max { return Err(io::Error::new(io::ErrorKind::InvalidData, "Rocoflex message is too large")); }
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(bytes)
}
