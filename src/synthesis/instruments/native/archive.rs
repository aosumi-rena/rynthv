use super::{Result, archive_file, program::{self, Node}, fail, hex, u32at, u64at};
use crate::synthesis::voice::{V3_HEADER_INITIALIZER, V3_SIGNING_MODULUS_LE};
use num_bigint::BigUint;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

const SETUP: &str = "498badb601cf788470fd352598d0bd49f5d410438c173fd1ca7cc7c52c8c6448";
const MODULUS: &str = "ef99f5a06528b00d47c821ceb9b77f9d5acec1d32ddb75005874e6f3239eaf13b487af58b34f057e4c85435c8ad66607b0b2e6bf99048546fb5d589f6bd9ca918e973bef452d6f02ae38919e5101893771b57f610d97b09c2de2e0070f50c1d53d05db95727a7004befebee5af98b30bc4c7f1f2004f9c3774c2626bbfe6105c7de362e0a307d353e7472b1d586afbf230ad14b2fd10ad557b9c60421a64a061117bfeb38adf9674eabc5a1d4bb420ec628366398be279e8ac8c2f3106fd3eb56a2d687990d3e53564c568852c79f412f8342ceb6c29b4bc566a84abb77b82b2da70f4f7595954ad5ace7c4874ddfb926064e89924986c9e3ef1f2c0c6783482";
const DOMAINS: [&str; 5] = [
    "d0e72de12984e883d5a6689c52f50c76",
    "3e7c6f3da61d8d6dbe002aacf5dffa47",
    "e544ba80524f67334d703d82c176477a",
    "9d76cfe84f3b54bc29135e44a28832ba",
    "10dd93ca42ce634bdc39e8bc77983719",
];

#[derive(Clone, Copy)]
pub(super) enum ArchiveProfile {
    Current,
    Early,
}

impl ArchiveProfile {
    const ALL: [Self; 2] = [Self::Current, Self::Early];

    fn index_keys(self, header: &[u8]) -> [[u8; 32]; 5] {
        // Early v4 archives retained the v3 initializer and signing key.
        // Their index and resource encoding are otherwise v4.
        let mask = match self {
            Self::Current => block(&hex(SETUP), &header[20..32], 0),
            Self::Early => block(&V3_HEADER_INITIALIZER, &header[20..32], 0),
        };
        let material = std::array::from_fn(|i| header[32 + i] ^ mask[i]);
        DOMAINS.map(|domain| keyed(&material, &hex(domain)))
    }
}
#[derive(Clone)]
struct Resource {
    pub region: usize,
    pub offset: usize,
    pub size: usize,
    pub digest: [u8; 16],
}
pub struct Archive {
    file: Mutex<File>,
    resources: HashMap<[u8; 16], Resource>,
    programs: Mutex<HashMap<[u8; 16], Arc<[Node]>>>,
    keys: [[u8; 32]; 5],
    region1_offset: usize,
}

const HEADER_SIZE: usize = 140;

#[derive(Clone, Copy)]
pub(super) struct Layout {
    regions: [(usize, usize); 3],
    pub(super) end: usize,
    pub(super) digest_bytes: usize,
    pub(super) block_size: usize,
    pub(super) leaves: usize,
}

impl Layout {
    fn parse(header: &[u8], length: usize) -> Result<Self> {
        if header.len() != HEADER_SIZE
            || length >= 0x7fffffff
            || header[..8] != [255, 0, 202, 127, 4, 0, 0, 0]
            || u32at(header, 8) as usize != HEADER_SIZE
        {
            return fail("Invalid DNNI-v4 header");
        }
        let mut regions = [(0, 0); 3];
        let to_usize =
            |offset| usize::try_from(u64at(header, offset)).map_err(|_| "Invalid DNNI-v4 regions");
        for (i, region) in regions.iter_mut().enumerate() {
            *region = (to_usize(64 + 16 * i)?, to_usize(72 + 16 * i)?);
        }
        let end = to_usize(112)?;
        let digest_bytes = to_usize(120)?;
        let block_size = u32at(header, 128) as usize;
        if block_size == 0
            || !(HEADER_SIZE..=length).contains(&end)
            || regions[0].0 != HEADER_SIZE
            || regions[0].1 < 20
            || regions
                .iter()
                .any(|&(offset, size)| offset > end || size > end - offset)
        {
            return fail("Invalid DNNI-v4 regions");
        }
        if end
            .checked_add(digest_bytes)
            .and_then(|at| at.checked_add(256))
            != Some(length)
        {
            return fail("DNNI-v4 signature extent");
        }
        let leaves = end.div_ceil(block_size);
        if leaves != u32at(header, 132) as usize || leaves > digest_bytes / 32 {
            return fail("DNNI-v4 digest count");
        }
        let mut nodes = leaves;
        let mut level = leaves;
        while level > 1 {
            level = level.div_ceil(2);
            nodes += level;
        }
        if nodes.checked_mul(32) != Some(digest_bytes) {
            return fail("DNNI-v4 digest extent");
        }
        Ok(Self {
            regions,
            end,
            digest_bytes,
            block_size,
            leaves,
        })
    }
}
fn keyed(key: &[u8; 32], b: &[u8]) -> [u8; 32] {
    *blake3::keyed_hash(key, b).as_bytes()
}
fn quarter(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}
fn block(key: &[u8], nonce: &[u8], index: u64) -> [u8; 64] {
    let mut s = [0; 16];
    s[2..6].copy_from_slice(&[0xb5243714, 0x767307d5, 0xa2847673, 0x87ae3203]);
    for i in 0..8 {
        s[i + 6] = u32at(key, i * 4);
    }
    let ctr = (u32at(nonce, 0) as u64).wrapping_add(index);
    s[0] = ctr as u32;
    s[1] = (ctr >> 32) as u32;
    s[14] = u32at(nonce, 4);
    s[15] = u32at(nonce, 8);
    let initial = s;
    for _ in 0..4 {
        quarter(&mut s, 0, 4, 8, 12);
        quarter(&mut s, 1, 5, 9, 13);
        quarter(&mut s, 2, 6, 10, 14);
        quarter(&mut s, 3, 7, 11, 15);
        quarter(&mut s, 0, 5, 10, 15);
        quarter(&mut s, 1, 6, 11, 12);
        quarter(&mut s, 2, 7, 8, 13);
        quarter(&mut s, 3, 4, 9, 14);
    }
    let mut out = [0; 64];
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&s[i].wrapping_add(initial[i]).to_le_bytes());
    }
    out
}
fn decode_at(mut bytes: &mut [u8], key: &[u8], nonce: &[u8], offset: usize) {
    let mut index = offset / 64;
    let mut skip = offset % 64;
    while !bytes.is_empty() {
        let mask = block(key, nonce, index as u64);
        let count = bytes.len().min(64 - skip);
        let (part, rest) = bytes.split_at_mut(count);
        for (value, mask) in part.iter_mut().zip(&mask[skip..]) {
            *value ^= mask;
        }
        bytes = rest;
        index += 1;
        skip = 0;
    }
}
fn poly1305(message: &[u8], key: &[u8]) -> [u8; 16] {
    let mut r = key[..16].to_vec();
    for i in [3, 7, 11, 15] {
        r[i] &= 15;
    }
    for i in [4, 8, 12] {
        r[i] &= 252;
    }
    let r = BigUint::from_bytes_le(&r);
    let p = (BigUint::from(1u8) << 130usize) - BigUint::from(5u8);
    let mut a = BigUint::from(0u8);
    for c in message.chunks(16) {
        let mut v = c.to_vec();
        v.push(1);
        a = ((a + BigUint::from_bytes_le(&v)) * &r) % &p;
    }
    let b = (a + BigUint::from_bytes_le(&key[16..])).to_bytes_le();
    let mut out = [0; 16];
    let n = b.len().min(16);
    out[..n].copy_from_slice(&b[..n]);
    out
}
pub(super) fn signature(bytes: &[u8], root: &[u8]) -> Result<ArchiveProfile> {
    match verify_signature(bytes, root, BigUint::from_bytes_le(&hex(MODULUS))) {
        Ok(()) => Ok(ArchiveProfile::Current),
        Err(error) => verify_signature(bytes, root, BigUint::from_bytes_le(&V3_SIGNING_MODULUS_LE))
            .map(|()| ArchiveProfile::Early)
            .map_err(|_| error),
    }
}

fn verify_signature(bytes: &[u8], root: &[u8], n: BigUint) -> Result<()> {
    let sig = BigUint::from_bytes_be(bytes);
    if sig >= n {
        return fail("DNNI-v4 signature range");
    }
    let raw = sig.modpow(&BigUint::from(3u8), &n).to_bytes_be();
    if raw.len() > 256 {
        return fail("DNNI-v4 signature width");
    }
    let mut em = [0; 256];
    em[256 - raw.len()..].copy_from_slice(&raw);
    if em[255] != 188 || em[0] & 128 != 0 {
        return fail("DNNI-v4 signature encoding");
    }
    let h = &em[223..255];
    let mut mask: Vec<u8> = Vec::new();
    for i in 0..7u32 {
        let mut b = h.to_vec();
        b.extend(i.to_be_bytes());
        mask.extend(blake3::hash(&b).as_bytes());
    }
    let mut db: Vec<u8> = em[..223].iter().zip(mask).map(|(a, b)| a ^ b).collect();
    db[0] &= 127;
    if db[..190].iter().any(|x| *x != 0) || db[190] != 1 {
        return fail("DNNI-v4 signature padding");
    }
    let mut b = vec![0; 8];
    b.extend(blake3::hash(root).as_bytes());
    b.extend(&db[191..]);
    if blake3::hash(&b).as_bytes() != h {
        return fail("DNNI-v4 signature digest");
    }
    Ok(())
}
impl Archive {
    pub fn open(path: &Path, accept_any_v4_signature: bool) -> Result<Arc<Self>> {
        super::archive_cache::open(path, accept_any_v4_signature)
    }

    pub(super) fn read_file(path: &Path, accept_any_v4_signature: bool) -> Result<Arc<Self>> {
        Self::from_file(path, accept_any_v4_signature).map(Arc::new)
    }

    fn from_file(path: &Path, accept_any_v4_signature: bool) -> Result<Self> {
        const MAX_INDEX_SIZE: usize = 16 * 1024 * 1024;
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let length = usize::try_from(file.metadata().map_err(|e| e.to_string())?.len())
            .map_err(|_| "Invalid DNNI-v4 length")?;
        let mut header = [0; HEADER_SIZE];
        file.read_exact(&mut header).map_err(|e| e.to_string())?;
        let layout = Layout::parse(&header, length)?;
        let (offset, size) = layout.regions[0];
        if size > MAX_INDEX_SIZE {
            return fail("DNNI-v4 resource index exceeds read limit");
        }
        let (mut index, signed_profile) = archive_file::read_index(
            &mut file, layout, offset + size, !accept_any_v4_signature,
        )?;
        if index[..offset] != header {
            return fail("DNNI-v4 header changed during index read");
        }
        let (keys, resources) =
            Self::read_index(&header, &mut index[offset..], &layout, signed_profile)?;
        Ok(Self {
            file: Mutex::new(file),
            resources,
            programs: Mutex::new(HashMap::new()),
            keys,
            region1_offset: layout.regions[2].0,
        })
    }

    fn read_index(
        header: &[u8],
        index: &mut [u8],
        layout: &Layout,
        signed_profile: Option<ArchiveProfile>,
    ) -> Result<([[u8; 32]; 5], HashMap<[u8; 16], Resource>)> {
        let profiles = signed_profile
            .as_ref()
            .map_or(ArchiveProfile::ALL.as_slice(), std::slice::from_ref);
        let mut first_shape_error = None;
        let mut record_error = None;
        for &profile in profiles {
            let keys = profile.index_keys(header);
            let mut prefix = [0; 20];
            prefix.copy_from_slice(&index[..20]);
            decode_at(&mut prefix, &keys[0], &keys[3][..12], 0);
            let start = match Self::record_start(&prefix, index.len()) {
                Ok(start) => start,
                Err(error) => {
                    first_shape_error.get_or_insert(error);
                    continue;
                }
            };
            decode_at(index, &keys[0], &keys[3][..12], 0);
            match Self::read_resources(&index[start..], layout, &keys[1]) {
                Ok(resources) => return Ok((keys, resources)),
                Err(error) if signed_profile.is_some() => return Err(error),
                Err(error) => record_error = Some(error),
            }
            // Restore the ciphertext before trying another initializer.
            decode_at(index, &keys[0], &keys[3][..12], 0);
        }
        Err(record_error
            .or(first_shape_error)
            .unwrap_or_else(|| "Invalid DNNI-v4 index".into()))
    }

    fn record_start(prefix: &[u8; 20], index_size: usize) -> Result<usize> {
        let count = u32at(prefix, 8) as usize;
        let g = u32at(prefix, 12) as usize;
        let range = u32at(prefix, 16) as usize;
        if range < count || g == 0 {
            return fail("Invalid DNNI-v4 index dimensions");
        }
        let start = (range - count)
            .checked_mul(4)
            .and_then(|remap| remap.checked_add(g))
            .and_then(|offset| offset.checked_add(20))
            .ok_or("Invalid DNNI-v4 index length")?;
        if count
            .checked_mul(69)
            .and_then(|bytes| start.checked_add(bytes))
            != Some(index_size)
        {
            return fail("Invalid DNNI-v4 index length");
        }
        Ok(start)
    }

    fn read_resources(
        records: &[u8],
        layout: &Layout,
        authentication_key: &[u8; 32],
    ) -> Result<HashMap<[u8; 16], Resource>> {
        let mut resources = HashMap::with_capacity(records.len() / 69);
        for b in records.chunks_exact(69) {
            let mut m = [0u8; 35];
            m[..3].copy_from_slice(&[0xf4, 0xa4, 0x40]);
            m[3..19].copy_from_slice(&b[..16]);
            m[19..].copy_from_slice(&b[21..37]);
            let ot = keyed(authentication_key, &m);
            if poly1305(&b[..53], &ot) != b[53..69] {
                return fail("DNNI-v4 resource authentication");
            }
            let r = Resource {
                region: b[20] as usize,
                offset: u64at(b, 21) as usize,
                size: u64at(b, 29) as usize,
                digest: b[37..53].try_into().unwrap(),
            };
            if r.region > 1
                || r.offset < layout.regions[r.region + 1].0
                || r.offset.checked_add(r.size).is_none_or(|e| {
                    e > layout.regions[r.region + 1].0 + layout.regions[r.region + 1].1
                })
                || resources.insert(b[..16].try_into().unwrap(), r).is_some()
            {
                return fail("DNNI-v4 resource extent/duplicate");
            }
        }
        Ok(resources)
    }
    fn resource(&self, id: &[u8; 16]) -> Option<&Resource> {
        let mut key = [1; 17];
        key[1..].copy_from_slice(id);
        let hash = keyed(&self.keys[2], &key);
        self.resources.get(&hash[..16])
    }

    pub fn contains(&self, id: &[u8; 16]) -> bool {
        self.resource(id).is_some()
    }

    pub(super) fn program(&self, id: [u8; 16]) -> Result<Arc<[Node]>> {
        let mut programs = self.programs.lock().map_err(|_| "Instrument program cache unavailable")?;
        if let Some(nodes) = programs.get(&id) {
            return Ok(nodes.clone());
        }
        let nodes: Arc<[Node]> = program::parse(&self.read(&id)?)?.into();
        programs.insert(id, nodes.clone());
        Ok(nodes)
    }

    pub fn read(&self, id: &[u8; 16]) -> Result<Vec<u8>> {
        let r = self.resource(id).ok_or("Resource absent from instrument model")?;
        let mut bytes = vec![0; r.size];
        {
            let mut file = self.file.lock().map_err(|_| "Instrument archive reader stopped")?;
            archive_file::read_at(&mut file, r.offset, &mut bytes)?;
        }
        if r.region == 1 {
            decode_at(
                &mut bytes,
                &self.keys[0],
                &self.keys[4][..12],
                r.offset - self.region1_offset,
            );
        }
        if blake3::hash(&bytes).as_bytes()[..16] != r.digest {
            return fail("Instrument resource plaintext digest");
        }
        Ok(bytes)
    }
}
