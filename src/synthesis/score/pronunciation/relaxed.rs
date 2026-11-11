use crate::{model::Language, synthesis::voice::{PreparedTree, Result}};

pub(super) struct Word {
    pub lyrics: String,
    pub beats: f64,
    pub syllables: Vec<Vec<String>>,
}

pub(super) fn apply(words: &mut [Word], tree: &PreparedTree,frontend:&super::LyricFrontend) -> Result<()> {

    let indices: Vec<_> = words.iter().enumerate().flat_map(|(w, word)|
        (0..word.syllables.len()).map(move |s| (w,s))).collect();
    for pair in indices.windows(2) {
        let (w,s) = pair[0];
        let (next_w,next_s) = pair[1];
        if words[w].lyrics.starts_with('.') {continue;}
        let Some(first) = words[next_w].syllables[next_s].first() else {continue;};
        let following = frontend.phone_class(Language::English,first,tree)?;
        let same = words[w].syllables[s].last() == Some(first);
        let beats = words[w].beats / words[w].syllables.len() as f64;
        let syllable = &mut words[w].syllables[s];
        let Some(last) = syllable.last() else {continue;};
        let last_class = frontend.phone_class(Language::English,last,tree)?;
        let previous_class = if syllable.len()>1 {
            frontend.phone_class(Language::English,&syllable[syllable.len()-2],tree)?
        } else {None};
        if matches!(last.as_str(), "t"|"d") && matches!(following, Some("vowel"|"diphthong")) {
            *syllable.last_mut().expect("nonempty") = "dx".into();
        }
        if (previous_class == Some("stop") && last_class == Some("stop") && following == Some("stop"))
            || (beats < 0.51 && same && last_class == Some("stop") && following == Some("stop") && syllable.len()>1) {
            syllable.pop();
        }
    }
    for i in 1..words.len() {
        if words[i].lyrics == "to" && words[i].syllables.len()==1 && words[i-1].beats < 0.51 {
            words[i].syllables[0] = vec!["dx".into(), "uw".into()];
        }
    }
    Ok(())
}
