pub(in crate::synthesis::score) fn split(phones: Vec<String>, classes: &[Option<&str>]) -> Vec<Vec<String>> {
    if phones.is_empty() { return Vec::new(); }
    let vowel=|i:usize| matches!(classes[i],Some("vowel"|"diphthong"));
    let mut roles:Vec<i8>=(0..phones.len()).map(|i|if vowel(i){0}else if classes[i]==Some("coda"){-1}else{-10}).collect();

    for i in 1..phones.len() {
        if !vowel(i-1) && vowel(i) && roles[i-1]==-10 {roles[i-1]=1;}
    }
    for i in 1..phones.len() {
        if !vowel(i-1) && classes[i]==Some("semivowel") && roles[i-1]==-10 {roles[i-1]=1;}
    }
    for i in 1..phones.len() {
        if vowel(i-1) && !vowel(i) && roles[i]==-10 {roles[i]=-1;}
    }

    for i in 0..phones.len()-1 {
        if phones[i]=="s" && "ktplr".contains(phones[i+1].as_str()) && roles[i]==-10 {roles[i]=1;}
    }
    for i in 1..phones.len() {
        if !vowel(i-1) && classes[i]==Some("liquid") && roles[i-1]==-10 {roles[i-1]=1;}
    }
    for i in 1..phones.len() {
        if !vowel(i-1) && !vowel(i) && roles[i-1]==-10 {roles[i-1]=1;}
    }
    let mut ranges=vec![0..1];
    for i in 1..phones.len() {
        if roles[i]!=-10 && roles[i-1]!=-10 && roles[i]<=roles[i-1] && !(roles[i]==0 && roles[i-1]==0) {
            ranges.last_mut().expect("first syllable").end=i+1;
        } else {ranges.push(i..i+1);}
    }

    let mut result=vec![phones[ranges[0].clone()].to_vec()];
    for range in ranges.into_iter().skip(1) {
        if range.clone().any(&vowel) {result.push(phones[range].to_vec());}
        else {result.last_mut().expect("first syllable").extend_from_slice(&phones[range]);}
    }
    result
}
