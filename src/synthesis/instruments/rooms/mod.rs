mod wire;
mod rice;
pub(crate) mod flxr;
pub mod hall;

use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}};
use wire::Reader;

#[derive(Clone, Debug)]
pub struct Response {
    pub position: [f32; 2],
    pub channels: usize,
    pub samples: usize,
    pub rate: usize,
    pub scale: f32,
    rice: u32,
    extent: std::ops::Range<usize>,
}
#[derive(Clone, Debug)]
pub struct Microphone {
    pub name: String,
    pub channel: u32,
    pub pan: f32,
    pub responses: Vec<Response>,
}
#[derive(Clone, Debug)]
pub struct Room {
    pub name: String,
    pub size: [f32; 2],
    pub origin: [f32; 2],
    pub microphones: Vec<Microphone>,
}
pub struct Pack {
    pub rooms: Vec<Room>,
    bytes: Arc<[u8]>,
    programs: BTreeMap<String, std::ops::Range<usize>>,
}
impl Pack {
    pub fn read(path: &Path) -> Result<Self, String> {
        let length = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
        if length > 32 * 1024 * 1024 { return Err("Instrument room pack exceeds limit".into()); }
        let bytes: Arc<[u8]> = std::fs::read(path).map_err(|e| e.to_string())?.into();
        let mut r = Reader { bytes: &bytes, at: 0 };
        if r.u32()? != 3 { return Err("Unsupported instrument room pack version".into()); }
        let count = r.count(16)?;
        let mut rooms = Vec::with_capacity(count);
        let mut names = BTreeSet::new();
        let mut first_data = bytes.len();
        for _ in 0..count {
            let name = r.text()?;
            if !names.insert(name.clone()) { return Err("Duplicate instrument room".into()); }
            let size = [r.f32()?, r.f32()?];
            let origin = [r.f32()?, r.f32()?];
            if size.iter().any(|v| *v <= 0.) { return Err("Invalid instrument room dimensions".into()); }
            let count = r.count(32)?;
            let mut microphones = Vec::with_capacity(count);
            let mut names = BTreeSet::new();
            for _ in 0..count {
                let name = r.text()?;
                if !names.insert(name.clone()) { return Err("Duplicate room microphone".into()); }
                let channel = r.u32()?;
                let pan = r.f32()?;
                if !(-1. ..=1.).contains(&pan) { return Err("Invalid room microphone pan".into()); }
                let count = r.count(128)?;
                let mut responses = Vec::with_capacity(count);
                for _ in 0..count {
                    let position = [r.f32()?, r.f32()?];
                    let channels = r.u32()? as usize;
                    let samples = r.u32()? as usize;
                    let rate = r.u32()? as usize;
                    let scale = r.f32()?;
                    let rice = r.u32()?;
                    let offset = usize::try_from(r.u64()?).map_err(|_| "Room response offset overflow")?;
                    let length = usize::try_from(r.u64()?).map_err(|_| "Room response length overflow")?;
                    let end = offset.checked_add(length).filter(|e| *e <= bytes.len())
                        .ok_or("Room response outside resource")?;
                    let measured = channels == 4 && rate == 48_000 && samples <= rate && length > 0 && rice <= 31;
                    let modeled = channels == 0 && samples == 0 && rate == 0 && rice == 0 && offset == 0 && length == 0;
                    if !measured && !modeled { return Err("Invalid room response dimensions".into()); }
                    if length > 0 { first_data = first_data.min(offset); }
                    responses.push(Response {position, channels, samples, rate, scale, rice, extent: offset..end});
                }
                microphones.push(Microphone {name, channel, pan, responses});
            }
            rooms.push(Room {name, size, origin, microphones});
        }
        let count = r.count(16)?;
        let mut programs = BTreeMap::new();
        for _ in 0..count {
            let name = r.text()?;
            let offset = usize::try_from(r.u64()?).map_err(|_| "Room program offset overflow")?;
            let length = usize::try_from(r.u64()?).map_err(|_| "Room program length overflow")?;
            let end = offset.checked_add(length).filter(|e| *e <= bytes.len())
                .ok_or("Room program outside resource")?;
            first_data = first_data.min(offset);
            if programs.insert(name, offset..end).is_some() { return Err("Duplicate room program".into()); }
        }
        if r.at != first_data { return Err("Instrument room pack header extent".into()); }
        Ok(Self {rooms, bytes, programs})
    }
    pub fn room(&self, name: &str) -> Option<&Room> { self.rooms.iter().find(|r| r.name == name) }
    pub fn response(&self, response: &Response) -> Result<Vec<Vec<f32>>, String> {
        rice::decode(&self.bytes[response.extent.clone()], response)
    }
    pub fn program(&self, name: &str) -> Result<flxr::Program, String> {
        let extent = self.programs.get(name).ok_or_else(|| format!("Spatial room model {name} absent"))?;
        flxr::Program::read(self.bytes.clone(), extent.clone())
    }
}

pub fn installed_path() -> Option<PathBuf> {
    std::env::var_os("ProgramFiles").map(PathBuf::from)
        .map(|p| p.join("Instrument X/rooms.drirpack")).filter(|p| p.is_file())
}

pub fn load(path: &Path) -> Result<Arc<Pack>, String> {
    type Identity = (PathBuf, u64, Option<std::time::SystemTime>);
    type Cached = Option<(Identity, Result<Arc<Pack>, String>)>;
    static CACHE: OnceLock<Mutex<Cached>> = OnceLock::new();
    let metadata = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let identity = (path.to_owned(), metadata.len(), metadata.modified().ok());
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((key, value)) = &*cache && key == &identity { return value.clone(); }
    let value = Pack::read(path).map(Arc::new);
    *cache = Some((identity, value.clone()));
    value
}
