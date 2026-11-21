use super::{Result, fail, hex, u32at, u64at};
use num_bigint::BigUint;
use std::{
    path::Path,
    sync::{Arc, OnceLock},
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
#[derive(Clone)]
pub struct Resource {
    pub hash: [u8; 16],
    pub region: usize,
    pub offset: usize,
    pub size: usize,
    pub digest: [u8; 16],
}
pub struct Archive {
    data: Vec<u8>,
    pub resources: Vec<Resource>,
    keys: [[u8; 32]; 5],
    region1: (usize, usize),
    plain: OnceLock<Vec<u8>>,
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
fn decode(b: &[u8], key: &[u8], nonce: &[u8]) -> Vec<u8> {
    let mut out = b.to_vec();
    for (i, v) in out.chunks_mut(64).enumerate() {
        let mask = block(key, nonce, i as u64);
        for (a, b) in v.iter_mut().zip(mask) {
            *a ^= b;
        }
    }
    out
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
fn signature(data: &[u8], at: usize, root: &[u8]) -> Result<()> {
    if data.len() != at + 256 {
        return fail("DNNI-v4 signature extent");
    }
    let n = BigUint::from_bytes_le(&hex(MODULUS));
    let sig = BigUint::from_bytes_be(&data[at..]);
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
    pub fn open(path: &Path) -> Result<Arc<Self>> {
        super::archive_cache::open(path)
    }
    pub(super) fn read_file(path: &Path) -> Result<Arc<Self>> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(data, false).map(|(archive, _)| archive)
    }

    pub(super) fn open_waveform(path: &Path) -> Result<(Arc<Self>, bool)> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(data, true)
    }

    fn parse(data: Vec<u8>, waveform: bool) -> Result<(Arc<Self>, bool)> {
        if data.len() < 140
            || data.len() >= 0x7fffffff
            || data[..8] != [255, 0, 202, 127, 4, 0, 0, 0]
            || u32at(&data, 8) != 140
        {
            return fail("Invalid DNNI-v4 header");
        }
        let regions: Vec<(usize, usize)> = (64..128)
            .step_by(16)
            .map(|a| (u64at(&data, a) as usize, u64at(&data, a + 8) as usize))
            .collect();
        let end = regions[3].0;
        let blocksize = u32at(&data, 128) as usize;
        let unsigned = waveform && data.get(end.saturating_add(regions[3].1)..).is_some_and(|footer| {
            footer.len() == 256 && footer.iter().all(|&byte| byte == 0)
        });
        if blocksize == 0
            || end < 140
            || end > data.len()
            || regions[0].0 != 140
            || regions[0].1 < 20
            || regions[..3].iter().any(|(a, n)| *a > end || *n > end - a)
            || (unsigned && (blocksize < 4096 || data.len() > 64 * 1024 * 1024))
        {
            return fail("Invalid DNNI-v4 regions");
        }
        let mut level: Vec<[u8; 32]> = data[..end]
            .chunks(blocksize)
            .map(|b| {
                let mut h = blake3::Hasher::new();
                h.update(&[0]);
                h.update(b);
                *h.finalize().as_bytes()
            })
            .collect();
        if level.len() != u32at(&data, 132) as usize {
            return fail("DNNI-v4 digest count");
        }
        let mut at = end;
        loop {
            for h in &level {
                if data.get(at..at + 32) != Some(h.as_slice()) {
                    return fail("DNNI-v4 digest mismatch");
                }
                at += 32;
            }
            if level.len() == 1 {
                break;
            }
            level = level
                .chunks(2)
                .map(|pair| {
                    if pair.len() == 1 {
                        pair[0]
                    } else {
                        let mut h = blake3::Hasher::new();
                        h.update(&[1]);
                        h.update(&pair[0]);
                        h.update(&pair[1]);
                        *h.finalize().as_bytes()
                    }
                })
                .collect();
        }
        if at - end != regions[3].1 {
            return fail("DNNI-v4 digest extent");
        }
        if !unsigned {
            signature(&data, at, &level[0])?;
        }
        let mask = block(&hex(SETUP), &data[20..32], 0);
        let mut material = [0; 32];
        for i in 0..32 {
            material[i] = data[32 + i] ^ mask[i];
        }
        let keys = DOMAINS.map(|s| keyed(&material, &hex(s)));
        let (a, n) = regions[0];
        let index = decode(&data[a..a + n], &keys[0], &keys[3][..12]);
        if index.len() < 20 {
            return fail("Truncated DNNI-v4 index");
        }
        let count = u32at(&index, 8) as usize;
        let g = u32at(&index, 12) as usize;
        let range = u32at(&index, 16) as usize;
        if range < count || g == 0 || (unsigned && count > 4096) {
            return fail("Invalid DNNI-v4 index dimensions");
        }
        let start = 20 + g + (range - count) * 4;
        if start.checked_add(count * 69) != Some(index.len()) {
            return fail("Invalid DNNI-v4 index length");
        }
        let mut resources = Vec::new();
        for b in index[start..].chunks_exact(69) {
            let mut m = hex("f4a440");
            m.extend(&b[..16]);
            m.extend(&b[21..37]);
            let ot = keyed(&keys[1], &m);
            if poly1305(&b[..53], &ot) != b[53..69] {
                return fail("DNNI-v4 resource authentication");
            }
            let r = Resource {
                hash: b[..16].try_into().unwrap(),
                region: b[20] as usize,
                offset: u64at(b, 21) as usize,
                size: u64at(b, 29) as usize,
                digest: b[37..53].try_into().unwrap(),
            };
            if r.region > 1
                || r.offset < regions[r.region + 1].0
                || r.offset
                    .checked_add(r.size)
                    .is_none_or(|e| e > regions[r.region + 1].0 + regions[r.region + 1].1)
                || resources.iter().any(|v: &Resource| v.hash == r.hash)
            {
                return fail("DNNI-v4 resource extent/duplicate");
            }
            resources.push(r);
        }
        Ok((Arc::new(Self {
            data,
            resources,
            keys,
            region1: regions[2],
            plain: OnceLock::new(),
        }), unsigned))
    }
    pub fn find(&self, id: &[u8; 16]) -> Result<&Resource> {
        let mut b = vec![1];
        b.extend(id);
        let h = keyed(&self.keys[2], &b);
        self.resources
            .iter()
            .find(|r| r.hash == h[..16])
            .ok_or_else(|| "Resource absent from instrument model".into())
    }
    pub fn read(&self, id: &[u8; 16]) -> Result<Vec<u8>> {
        let r = self.find(id)?;
        let b = if r.region == 0 {
            &self.data[r.offset..r.offset + r.size]
        } else {
            let (a, n) = self.region1;
            let p = self
                .plain
                .get_or_init(|| decode(&self.data[a..a + n], &self.keys[0], &self.keys[4][..12]));
            &p[r.offset - a..r.offset - a + r.size]
        };
        if blake3::hash(b).as_bytes()[..16] != r.digest {
            return fail("Instrument resource plaintext digest");
        }
        Ok(b.to_vec())
    }
}
