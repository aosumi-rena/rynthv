use super::{Result, archive::Archive, id, render::WAVE_PROGRAM};
use std::{path::Path, sync::Arc};

pub(in super::super) fn open_waveform(path: &Path, accept_any_v4_signature: bool) -> Result<Arc<Archive>> {
    let archive = Archive::open(path, accept_any_v4_signature)?;
    let nodes = archive.program(id(WAVE_PROGRAM))?;
    if ![931, 941].iter().all(|tag| nodes.iter().filter(|node| node.flags & 2 != 0 && node.tag == *tag).count() == 1) {
        return Err("Incompatible instrument waveform outputs".into());
    }
    Ok(archive)
}
