use std::collections::HashMap;

pub(super) fn link(lyrics:&[&str],words:&mut [Vec<Vec<String>>],classes:&HashMap<String,String>) {
    let mut syllables=Vec::new();
    for (word,parts) in words.iter_mut().enumerate() {
        let spelling=lyrics[word];
        let block=["ss","ç","Ç","xc","sc"].iter().any(|s|spelling.contains(s));
        let bytes=spelling.as_bytes();
        let single_s=bytes.last().is_some_and(|c|matches!(c,b's'|b'S'))
            && bytes.get(bytes.len().saturating_sub(2)).is_none_or(|c|bytes.len()==1||!matches!(c,b's'|b'S'));
        let count=parts.len();
        for (index,phones) in parts.iter_mut().enumerate() {
            syllables.push(Syllable {word,last:index+1==count,blocked:block&&(index+1!=count||!single_s),phones:std::mem::take(phones)});
        }
    }

    for index in 0..syllables.len().saturating_sub(1) {
        let current=&syllables[index];let next=&syllables[index+1];
        if current.last&&current.word!=next.word&&current.phones.last().is_some_and(|p|p=="H")
            && lyrics[next.word].as_bytes().first().is_some_and(|c|matches!(c,b'r'|b'R')) {
            syllables[index].phones.pop();
        }
    }
    let vowel=|p:&str|classes.get(p).is_some_and(|c|matches!(c.as_str(),"vowel"|"diphthong"));

    let sonorant=|p:&str|vowel(p)||classes.get(p).is_some_and(|c|c=="semivowel");
    for index in 0..syllables.len() {
        let current=&syllables[index];
        if current.blocked||current.phones.last().is_none_or(|p|p!="s") {continue}
        let previous=current.phones.len().checked_sub(2).and_then(|i|current.phones.get(i));
        let next=syllables.get(index+1).and_then(|s|s.phones.first());
        if previous.is_some_and(|p|sonorant(p))&&next.is_some_and(|p|sonorant(p)) {
            *syllables[index].phones.last_mut().expect("final s")="z".into();
        }
    }
    for index in 0..syllables.len() {
        if syllables[index].phones.last().is_some_and(|p|p=="H")
            && syllables.get(index+1).and_then(|s|s.phones.first()).is_some_and(|p|vowel(p)) {
            *syllables[index].phones.last_mut().expect("final X")="r".into();
        }
    }
    for index in 0..syllables.len() {
        for phone in 0..syllables[index].phones.len() {
            let current=&syllables[index].phones[phone];
            if !matches!(current.as_str(),"t"|"d") {continue}
            let next=syllables[index].phones.get(phone+1).or_else(||syllables.get(index+1).and_then(|s|s.phones.first()));
            if next.is_some_and(|p|p=="i") {
                syllables[index].phones[phone]=if current=="t" {"tch"}else{"dj"}.into();
            }
        }
    }
    for index in 0..syllables.len() {
        let mut phone=0;
        while phone<syllables[index].phones.len() {
            if !matches!(syllables[index].phones[phone].as_str(),"s"|"z"|"ch"|"j") {phone+=1;continue}
            let within=syllables[index].phones.get(phone+1);
            if within.is_some_and(|p|vowel(p)) {phone+=1;continue}
            let next=within.or_else(||syllables.get(index+1).and_then(|s|s.phones.first()));
            if next.is_some_and(|p|p=="s") {syllables[index].phones.remove(phone);continue}
            let replacement=match next {
                None=>Some("ch"),
                Some(p) if UNVOICED.contains(&p.as_str())||classes.get(p).is_some_and(|c|c=="silence")=>Some("ch"),
                Some(p) if VOICED.contains(&p.as_str())=>Some("j"),
                _=>None,
            };
            if let Some(replacement)=replacement {syllables[index].phones[phone]=replacement.into()}
            phone+=1;
        }
    }
    for parts in words.iter_mut(){parts.clear()}
    for syllable in syllables {words[syllable.word].push(syllable.phones)}
}

struct Syllable {word:usize,last:bool,blocked:bool,phones:Vec<String>}
const UNVOICED:&[&str]=&["ch","f","gw","k","kw","p","s","t","tch"];
const VOICED:&[&str]=&["r","nh","lh","R","j","b","d","dj","g","h","l","m","n","-r","rr","v","z"];
