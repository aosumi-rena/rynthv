use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VoiceRevision {
    number: u32,
    suffix: RevisionSuffix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum RevisionSuffix {
    Alpha(u32),
    Beta(u32),
    Release,
    Update(u32),
}

impl VoiceRevision {
    pub const fn beta(number: u32, build: u32) -> Self {
        Self { number, suffix: RevisionSuffix::Beta(build) }
    }

    pub const fn release(number: u32) -> Self {
        Self {
            number,
            suffix: RevisionSuffix::Release,
        }
    }
}

impl FromStr for VoiceRevision {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let number_end = text.bytes().take_while(u8::is_ascii_digit).count();
        let number = parse_number(&text[..number_end])?;
        let tail = &text[number_end..];
        let suffix = if tail.is_empty() {
            RevisionSuffix::Release
        } else {
            if !matches!(tail.as_bytes()[0], b'a' | b'b' | b'.') {
                return Err(invalid_revision());
            }
            let build = parse_number(&tail[1..])?;
            match tail.as_bytes()[0] {
                b'a' => RevisionSuffix::Alpha(build),
                b'b' => RevisionSuffix::Beta(build),
                b'.' => RevisionSuffix::Update(build),
                _ => return Err(invalid_revision()),
            }
        };
        Ok(Self { number, suffix })
    }
}

fn parse_number(text: &str) -> Result<u32, String> {
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(invalid_revision());
    }
    text.parse().map_err(|_| invalid_revision())
}

fn invalid_revision() -> String {
    "Voice revisions must use an integer, <int>a<int>, <int>b<int>, or <int>.<int>".into()
}

impl fmt::Display for VoiceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.number)?;
        match self.suffix {
            RevisionSuffix::Release => Ok(()),
            RevisionSuffix::Alpha(build) => write!(formatter, "a{build}"),
            RevisionSuffix::Beta(build) => write!(formatter, "b{build}"),
            RevisionSuffix::Update(build) => write!(formatter, ".{build}"),
        }
    }
}

impl From<VoiceRevision> for String {
    fn from(revision: VoiceRevision) -> Self {
        revision.to_string()
    }
}

impl TryFrom<String> for VoiceRevision {
    type Error = String;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}
