use super::{
    Result,
    archive::{ArchiveProfile, Layout},
    fail,
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

const BLOCK_READ_BUFFER: usize = 64 * 1024;
const MAX_DIGEST_TABLE: usize = 16 * 1024 * 1024;

fn pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hash = blake3::Hasher::new();
    hash.update(&[1]);
    hash.update(left);
    hash.update(right);
    *hash.finalize().as_bytes()
}

pub(super) fn read_at(file: &mut File, offset: usize, bytes: &mut [u8]) -> Result<()> {
    file.seek(SeekFrom::Start(offset as u64))
        .and_then(|_| file.read_exact(bytes))
        .map_err(|error| error.to_string())
}

struct DigestTree {
    nodes: Vec<u8>,
    leaves: usize,
    root: [u8; 32],
}

impl DigestTree {
    fn prove(&self, mut index: usize, mut digest: [u8; 32]) -> bool {
        let mut start = 0;
        let mut count = self.leaves;
        while count > 1 {
            let sibling = index ^ 1;
            if sibling < count {
                let offset = (start + sibling) * 32;
                let sibling = self.nodes[offset..offset + 32].try_into().unwrap();
                digest = if index % 2 == 0 {
                    pair(&digest, sibling)
                } else {
                    pair(sibling, &digest)
                };
            }
            index /= 2;
            start += count;
            count = count.div_ceil(2);
        }
        digest == self.root
    }
}

// The signed index authenticates the plaintext digest of every resource.
pub(super) fn read_index(
    file: &mut File,
    layout: Layout,
    length: usize,
    strict: bool,
) -> Result<(Vec<u8>, Option<ArchiveProfile>)> {
    if !strict {
        let mut index = vec![0; length];
        read_at(file, 0, &mut index)?;
        return Ok((index, None));
    }
    if layout.digest_bytes > MAX_DIGEST_TABLE {
        return fail("DNNI-v4 digest table exceeds read limit");
    }
    let mut nodes = vec![0; layout.digest_bytes];
    read_at(file, layout.end, &mut nodes)?;
    let root: [u8; 32] = nodes[nodes.len() - 32..].try_into().unwrap();
    let mut footer = [0; 256];
    file.read_exact(&mut footer).map_err(|error| error.to_string())?;
    let profile = super::archive::signature(&footer, &root)?;
    let tree = DigestTree { nodes, leaves: layout.leaves, root };
    let mut index = Vec::with_capacity(length);
    let mut buffer = vec![0; layout.block_size.min(layout.end).min(BLOCK_READ_BUFFER)];
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    for (block, start) in (0..length).step_by(layout.block_size).enumerate() {
        let stop = (start + layout.block_size).min(layout.end);
        let mut hash = blake3::Hasher::new();
        hash.update(&[0]);
        for position in (start..stop).step_by(buffer.len()) {
            let count = (stop - position).min(buffer.len());
            let chunk = &mut buffer[..count];
            file.read_exact(chunk).map_err(|e| e.to_string())?;
            hash.update(chunk);
            index.extend_from_slice(&chunk[..length.saturating_sub(position).min(count)]);
        }
        if !tree.prove(block, *hash.finalize().as_bytes()) {
            return fail("DNNI-v4 digest mismatch");
        }
    }
    Ok((index, Some(profile)))
}
