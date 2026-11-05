// SPDX-License-Identifier: MIT
use super::{Error, Result, StdBank, archive::Reader};
use std::{collections::HashMap, fs, path::Path, sync::{Arc, Mutex}};

const JP: &[&str] = &[
    "a", "i", "u", "e", "o", "N", "cl", "t", "d", "s", "sh", "j", "z", "ts", "k", "kw", "g", "gw",
    "h", "b", "p", "f", "ch", "ry", "ky", "py", "dy", "ty", "ny", "hy", "my", "gy", "by", "n", "m",
    "r", "w", "v", "y", "pau", "sil",
];
pub(super) const ZH: &[&str] = &[
    "a", "A", "o", "@", "e", "7", "U", "u", "i", "i\\", "i`", "y", "AU", "@U", "ia", "iA", "iAU",
    "ie", "iE", "iU", "i@U", "y{", "yE", "ua", "uA", "u@", "ue", "uo", ":\\i", "r\\`", ":n", "N",
    "p", "ph", "t", "th", "k", "kh", "ts\\", "ts", "tsh", "ts`", "ts`h", "x", "f", "s", "s`",
    "ts\\h", "s\\", "m", "n", "l", "z`", "w", "j", "pau", "sil",
];
pub(crate) struct UnitCostModel {
    members: Vec<Member>,
    ids: HashMap<String, usize>,
    japanese: bool,
    categories: HashMap<String, u8>,
    cache: Mutex<HashMap<[usize; 5], Arc<[f32]>>>,
    distances: Mutex<HashMap<([usize; 5], [usize; 5], bool), f64>>,
}
struct Tensor {
    rows: usize,
    columns: usize,
    weights: Vec<f32>,
    biases: Vec<f32>,
}
struct Member {
    parts: Vec<Tensor>,
}

impl UnitCostModel {
    pub fn open(bank: &StdBank, clf_directory: &Path) -> Result<Self> {
        let japanese = bank.profile.encoder_only;
        let names = if japanese { JP } else { ZH };
        let file = clf_directory.join(if japanese {
            "japanese-romaji-phones.txt"
        } else {
            "mandarin-xsampa-phones.txt"
        });
        let text = fs::read_to_string(file)?;
        if text.len() > 1_048_576 {
            return Err(Error::Unsupported("phone inventory extent"));
        }
        let rows: Vec<Vec<&str>> = text
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.split_whitespace().collect())
            .collect();
        let expected = if japanese { 42 } else { 59 };
        if rows.len() != expected
            || rows.iter().any(|row| row.len() != 2)
            || rows[..names.len()]
                .iter()
                .zip(names)
                .any(|(row, name)| row[0] != *name)
            || rows[expected - 1] != ["br", "silence"]
            || !japanese && rows[57] != ["cl", "stop"]
        {
            return Err(Error::Unsupported("exact STD embedding phone order"));
        }
        let ids = names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.to_string(), i))
            .collect();
        let class_names = [
            "vowel",
            "diphthong",
            "stop",
            "affricate",
            "fricative",
            "aspirate",
            "liquid",
            "nasal",
            "semivowel",
            "coda",
            "silence",
        ];
        let mut categories = HashMap::new();
        for row in &rows {
            let kind = class_names
                .iter()
                .position(|name| *name == row[1])
                .ok_or(Error::Format("phone category"))? as u8;
            categories.insert(row[0].to_owned(), kind);
        }
        if bank.configuration.single_text("ude_arch")? != "factorized" {
            return Err(Error::Unsupported("STD UDE architecture"));
        }
        let selectors = bank.configuration.strings("ude_ensemble")?;
        if selectors.len() != 5 {
            return Err(Error::Unsupported("five-member STD UDE ensemble"));
        }
        let mut members = Vec::with_capacity(5);
        for selector in selectors {
            if selector.is_empty() || selector.contains(&0) {
                return Err(Error::Format("UDE selector name"));
            }
            let mut name = vec![0x8d, 0xc6, 0x2b, 0x5b, 0x69];
            name.extend_from_slice(selector);
            let mut reader = Reader::new(bank.archive.named(&name)?);
            let mut parts = Vec::with_capacity(7);
            for (i, (rows, columns)) in [
                (names.len(), 32),
                (names.len(), 16),
                (64, 48),
                (32, 64),
                (64, 48),
                (32, 64),
                (1, 96),
            ]
            .into_iter()
            .enumerate()
            {
                parts.push(Tensor::read(&mut reader, rows, columns, i >= 2)?);
            }
            reader.end()?;
            members.push(Member { parts });
        }
        Ok(Self {
            members,
            ids,
            japanese,
            categories,
            cache: Mutex::new(HashMap::new()),
            distances: Mutex::new(HashMap::new()),
        })
    }
    fn map(&self, context: [&str; 5]) -> Result<[usize; 5]> {
        let mut ids = [0; 5];
        for (i, label) in context.into_iter().enumerate() {
            let lookup = if self.japanese {
                label.trim_end_matches(|c: char| c.is_ascii_digit())
            } else {
                label
            };
            ids[i] = *self.ids.get(lookup).ok_or(Error::Unsupported(
                "phone absent from bank UDE inventory; no embedding fallback",
            ))?;
        }
        Ok(ids)
    }
    fn features(&self, context: [&str; 5]) -> Result<Arc<[f32]>> {
        let ids = self.map(context)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| Error::Format("UDE feature cache poisoned"))?;
        if let Some(value) = cache.get(&ids) {
            return Ok(Arc::clone(value));
        }
        let mut features = Vec::with_capacity(480);
        for member in &self.members {
            features.extend(member.parts[0].gather(ids[2])?);
            for side in 0..2 {
                let mut x = Vec::with_capacity(48);
                for j in 0..3 {
                    x.extend(member.parts[1].gather(ids[side * 2 + j])?);
                }
                let mut y = member.parts[2 + side * 2].affine(&x)?;
                for value in &mut y {
                    *value = value.tanh();
                }
                features.extend(member.parts[3 + side * 2].affine(&y)?);
            }
        }
        let features: Arc<[f32]> = features.into();
        if cache.len() < 4096 {
            cache.insert(ids, Arc::clone(&features));
        }
        Ok(features)
    }
    pub fn category(&self, phone: &str) -> u8 {
        let phone = if self.japanese {
            phone.trim_end_matches(|c: char| c.is_ascii_digit())
        } else {
            phone
        };
        self.categories.get(phone).copied().unwrap_or(10)
    }
    pub fn cost(&self, target: [&str; 5], candidate: [&str; 5], silence: bool) -> Result<f64> {
        let equal = target[2] == candidate[2];
        if equal && silence {
            return Ok(0.0);
        }
        let key = (self.map(target)?, self.map(candidate)?, equal);
        if let Some(value) = self.distances.lock()
            .map_err(|_| Error::Format("UDE distance cache poisoned"))?.get(&key) {
            return Ok(*value);
        }
        let target_features = self.features(target)?;
        let source_features = self.features(candidate)?;
        let mut sum = 0.0;
        for (i, member) in self.members.iter().enumerate() {
            let squares: Vec<_> = source_features[i * 96..(i + 1) * 96]
                .iter()
                .zip(&target_features[i * 96..(i + 1) * 96])
                .map(|(a, b)| {
                    let d = a - b;
                    d * d
                })
                .collect();
            let reduced = member.parts[6].affine(&squares)?[0];
            sum += f64::from(if reduced > 0.0 { reduced } else { 0.0 });
        }
        let value = sum / (self.members.len() as f64) + if equal { 0.0 } else { 2.0 };
        let mut distances = self.distances.lock()
            .map_err(|_| Error::Format("UDE distance cache poisoned"))?;
        if distances.len() < 65536 { distances.insert(key, value); }
        Ok(value)
    }
}
impl Tensor {
    fn read(reader: &mut Reader<'_>, rows: usize, columns: usize, affine: bool) -> Result<Self> {
        if reader.f32()? != rows as f32
            || reader.f32()? != columns as f32
            || affine && reader.f32()? != rows as f32
        {
            return Err(Error::Unsupported("UDE tensor dimensions"));
        }
        let weights = (0..rows * columns)
            .map(|_| reader.f32())
            .collect::<Result<Vec<_>>>()?;
        let biases = if affine {
            (0..rows)
                .map(|_| reader.f32())
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        if weights.iter().chain(&biases).any(|v| !v.is_finite()) {
            return Err(Error::Format("UDE finite weights"));
        }
        Ok(Self {
            rows,
            columns,
            weights,
            biases,
        })
    }
    fn gather(&self, index: usize) -> Result<Vec<f32>> {
        if index >= self.rows {
            return Err(Error::Format("UDE embedding index"));
        }
        Ok((0..self.columns)
            .map(|j| self.weights[j * self.rows + index])
            .collect())
    }
    fn affine(&self, input: &[f32]) -> Result<Vec<f32>> {
        if input.len() != self.columns || self.biases.len() != self.rows {
            return Err(Error::Format("UDE affine shape"));
        }
        let mut output = vec![0.0; self.rows];
        for (k, &value) in input.iter().enumerate() {
            for (i, target) in output.iter_mut().enumerate() {
                *target += self.weights[k * self.rows + i] * value;
            }
        }
        for (target, bias) in output.iter_mut().zip(&self.biases) {
            *target += bias;
        }
        if output.iter().any(|v| !v.is_finite()) {
            return Err(Error::Format("UDE affine nonfinite output"));
        }
        Ok(output)
    }
}
