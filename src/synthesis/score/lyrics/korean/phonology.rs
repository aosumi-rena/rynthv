// Preserve syllable boundaries during pronunciation changes.
const ONSETS: [&str; 19] = [
    "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p",
    "h",
];
const VOWELS: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "weo", "we",
    "wi", "yu", "eu", "ui", "i",
];
const CODAS: [&str; 28] = [
    "", "g", "kk", "gs", "n", "nj", "nh", "d", "l", "lg", "lm", "lb", "ls", "lt", "lp", "lh", "m",
    "b", "bs", "s", "ss", "ng", "j", "ch", "k", "t", "p", "h",
];

#[derive(Clone)]
pub(super) struct Syllable {
    onset: &'static str,
    vowel: &'static str,
    coda: &'static str,
}

pub(super) fn syllables(word: &str) -> Option<Vec<Syllable>> {
    let mut output = Vec::new();
    let mut chars = word.chars().peekable();
    while let Some(ch) = chars.next() {
        let code = ch as u32;
        if (0xac00..=0xd7a3).contains(&code) {
            let index = (code - 0xac00) as usize;
            output.push(Syllable {
                onset: ONSETS[index / 588],
                vowel: VOWELS[index / 28 % 21],
                coda: CODAS[index % 28],
            });
        } else if (0x1100..=0x1112).contains(&code) {
            let vowel = chars.next()? as u32;
            if !(0x1161..=0x1175).contains(&vowel) {
                return None;
            }
            let coda = chars
                .peek()
                .copied()
                .map(|c| c as u32)
                .filter(|c| (0x11a8..=0x11c2).contains(c));
            if coda.is_some() {
                chars.next();
            }
            output.push(Syllable {
                onset: ONSETS[(code - 0x1100) as usize],
                vowel: VOWELS[(vowel - 0x1161) as usize],
                coda: CODAS[coda.map_or(0, |c| (c - 0x11a7) as usize)],
            });
        } else {
            return None;
        }
    }
    (!output.is_empty()).then_some(output)
}

fn neutral(coda: &str) -> &'static str {
    match coda {
        "g" | "kk" | "gs" | "lg" | "k" => "k",
        "n" | "nj" | "nh" => "n",
        "d" | "s" | "ss" | "j" | "ch" | "t" | "h" => "t",
        "l" | "ls" | "lt" | "lh" => "l",
        "lm" | "m" => "m",
        "b" | "lb" | "bs" | "lp" | "p" => "p",
        "ng" => "ng",
        _ => "",
    }
}

fn liaison(coda: &'static str) -> (&'static str, &'static str) {
    match coda {
        "gs" => ("g", "ss"),
        "nj" => ("n", "j"),
        "nh" => ("n", ""),
        "lg" => ("l", "g"),
        "lm" => ("l", "m"),
        "lb" => ("l", "b"),
        "ls" => ("l", "ss"),
        "lt" => ("l", "t"),
        "lp" => ("l", "p"),
        "lh" => ("l", ""),
        "bs" => ("b", "ss"),
        "l" => ("", "r"),
        "h" => ("", ""),
        "ng" => ("ng", ""),
        other => ("", other),
    }
}

pub(super) fn rewrite(mut parts: Vec<Syllable>) -> Vec<String> {
    for index in 0..parts.len().saturating_sub(1) {
        let (before, after) = parts.split_at_mut(index + 1);
        let left = &mut before[index];
        let right = &mut after[0];
        // Preserve the original coda until assimilation is resolved.
        if matches!(left.coda, "h" | "nh" | "lh") {
            let aspirated = match right.onset {
                "g" => Some("k"),
                "d" => Some("t"),
                "b" => Some("p"),
                "j" => Some("ch"),
                _ => None,
            };
            if let Some(onset) = aspirated {
                left.coda = match left.coda {
                    "nh" => "n",
                    "lh" => "l",
                    _ => "",
                };
                right.onset = onset;
            } else if right.onset == "s" {
                left.coda = match left.coda {
                    "nh" => "n",
                    "lh" => "l",
                    _ => "",
                };
                right.onset = "ss";
            }
        }
        if right.onset == "" {
            let (coda, onset) = liaison(left.coda);
            left.coda = coda;
            right.onset = match (onset, right.vowel) {
                ("d", "i") => "j",
                ("t", "i") => "ch",
                _ => onset,
            };
        } else if right.onset == "h" {
            let aspirated = match neutral(left.coda) {
                "k" => Some("k"),
                "t" => Some(if right.vowel == "i" { "ch" } else { "t" }),
                "p" => Some("p"),
                _ => None,
            };
            if let Some(onset) = aspirated {
                left.coda = if left.coda.starts_with('l') { "l" } else { "" };
                right.onset = onset;
            }
        }
        if left.coda == "n" && right.onset == "r" || left.coda == "l" && right.onset == "n" {
            left.coda = "l";
            right.onset = "l";
        } else if matches!(neutral(left.coda), "m" | "ng" | "k" | "p") && right.onset == "r" {
            right.onset = "n";
        }
        if matches!(right.onset, "n" | "m") {
            left.coda = match neutral(left.coda) {
                "k" => "ng",
                "t" => "n",
                "p" => "m",
                other => other,
            };
        } else if matches!(neutral(left.coda), "k" | "t" | "p") {
            right.onset = match right.onset {
                "g" => "kk",
                "d" => "tt",
                "b" => "pp",
                "s" => "ss",
                "j" => "jj",
                other => other,
            };
        }
        // The liquid after a retained liquid is lateral, not an onset flap.
        if left.coda == "l" && right.onset == "r" {
            right.onset = "l";
        }
    }
    parts
        .into_iter()
        .map(|part| format!("{}{}{}", part.onset, part.vowel, neutral(part.coda)))
        .collect()
}
