mod parse;
mod rewrite;

use std::{collections::HashMap, path::Path};
use crate::synthesis::voice::{Result, VoiceError};

#[derive(Debug)]
pub struct PhoneConversion {
    symbols: Vec<String>,
    ids: HashMap<String, usize>,
    graphs: [Graph; 2],
    boundary: usize,
}

#[derive(Debug)]
struct Graph {
    start: usize,
    states: Vec<State>,
    members: HashMap<usize, Vec<bool>>,
}

#[derive(Debug)]
struct State { final_cost: Option<i32>, arcs: Vec<Arc> }

#[derive(Debug)]
struct Arc { input: usize, output: usize, target: usize, cost: i32 }

impl PhoneConversion {
    pub fn identity(&self) -> &'static str { Self::IDENTITY }

    const IDENTITY: &'static str = "2307db43cd330d2d386681aa71fecd8ac929532634db83ae5305d0b1b03e3be2";

    pub fn load(path: &Path) -> Result<Self> {
        let file=std::fs::File::open(path).map_err(|error|VoiceError::new(format!("{}: {error}",path.display())))?;
        if file.metadata().map_err(|error|VoiceError::new(error.to_string()))?.len()!=8143 {
            return Err(invalid());
        }
        use std::io::Read;
        let mut bytes=Vec::new();
        file.take(8144).read_to_end(&mut bytes).map_err(|error|VoiceError::new(error.to_string()))?;
        Self::from_bytes(&bytes)
    }
}

fn invalid() -> VoiceError { VoiceError::new(crate::i18n::t("PHONE_CONVERSION_INVALID")) }
fn limit() -> VoiceError { VoiceError::new(crate::i18n::t("PHONE_CONVERSION_LIMIT")) }
