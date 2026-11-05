use super::dictionary::{split_nuclei};
use crate::synthesis::voice::Result;

pub(super) struct Spanish {nuclei: Vec<String>}
impl Spanish {
    pub(super) fn load(catalog:&crate::model::PhonemeCatalog) -> Result<Self> {
        let nuclei=catalog.for_language(crate::model::Language::Spanish).filter(|e|matches!(e.category.as_str(),"vowel"|"diphthong")).map(|e|e.symbol.clone()).collect();
        Ok(Self {nuclei})
    }
    pub(super) fn resolve(&self, lyrics: &str, european: bool) -> Result<Vec<Vec<String>>> {
        let mut result=Vec::new();
        for word in lyrics.split_whitespace() {
            let phones=transduce(&word.to_lowercase()).into_iter().map(|phone|
                if matches!(phone,"S"|"Z") {if european {"C"} else {"s"}} else {phone}
            ).map(str::to_owned).collect();

            result.extend(split_nuclei(phones,&self.nuclei));
        }
        Ok(result)
    }
}

const VOWELS: &str="aeiouáéíóúü";
fn vowel(c: char) -> bool {VOWELS.contains(c)}
fn plain_vowel(c: char) -> bool {"aeiouáéíóú".contains(c)}
fn hard(phone: &str) -> bool {
    matches!(phone,"."|"m"|"n"|"N"|"J"|"D"|"G"|"B"|"t"|"p"|"k"|"b"|"g"|"d"|"x")
}
fn trill(phone: &str) -> bool {
    matches!(phone,"n"|"N"|"m"|"J"|"l"|"ll"|"ch"|"s"|"x"|"C"|"sh")
}
fn assimilate(out: &mut [&'static str], phone: &'static str) {
    if let Some(last)=out.last_mut() && *last=="n" {*last=phone;}
}
fn preceding_i(out: &mut [&'static str], in_vowel: bool) {
    if in_vowel && let Some(last)=out.last_mut() && *last=="I" {*last="y";}
}

fn transduce(word: &str) -> Vec<&'static str> {
    let chars:Vec<_>=word.chars().collect();let mut out=Vec::new();let mut in_vowel=false;let mut i=0;
    while i<chars.len() {
        let c=chars[i];let previous=i.checked_sub(1).map_or('\0',|p|chars[p]);
        let next=chars.get(i+1).copied().unwrap_or('\0');
        let next2=chars.get(i+2).copied().unwrap_or('\0');
        let last=out.last().copied().unwrap_or(".");
        match c {

            'á'|'é'|'í'|'ó'|'ú'|'ü' => {
                preceding_i(&mut out,in_vowel);
                out.push(match c {'á'=>"a",'é'=>"e",'í'=>"i",'ó'=>"o",'ú'=>"u",_=>"U"});
                in_vowel=true;
            }
            'a'|'e'|'o' => {preceding_i(&mut out,in_vowel);out.push(match c {'a'=>"a",'e'=>"e",_=>"o"});in_vowel=true;}

            'i' => {
                if in_vowel {preceding_i(&mut out,true);out.push(if matches!(previous,'i'|'ü'|'u') {"i"}else{"I"});}
                else {out.push(if i>0 && vowel(next) {"y"}else{"i"});}
                in_vowel=true;
            }
            'u' => {
                if in_vowel {preceding_i(&mut out,true);out.push(if matches!(previous,'i'|'ü'|'u') {"u"}else{"U"});in_vowel=true;}
                else if i==0 || next=='\0' {out.push("u");in_vowel=true;}
                else if !matches!(previous,'q'|'g') {out.push(if plain_vowel(next) {"U"}else{"u"});in_vowel=true;}
                else if next=='i' && plain_vowel(next2) {out.push("y");i+=1;in_vowel=true;}
                else if matches!(next,'e'|'u'|'é'|'í'|'ú'|'i') {}
                else {out.push(if matches!(next,'a'|'o'|'á'|'ó') {"U"}else{"u"});in_vowel=true;}
            }
            'y' => {
                if chars.len()==1 {out.push("i");in_vowel=true;}
                else if next=='\0' || (i>0 && in_vowel && !vowel(next) && next!='y') {out.push("I");in_vowel=true;}
                else {out.push("y");in_vowel=false;}
            }

            'b' => {out.push(if hard(last) {"b"}else{"B"});in_vowel=false;}
            'd' => {out.push(if hard(last) {"d"}else{"D"});in_vowel=false;}
            'c' => {
                if next=='h' {out.push("ch");i+=1;in_vowel=false;}
                else if next=='u' {assimilate(&mut out,"N");out.extend(["k",if plain_vowel(next2) {"U"}else{"u"}]);i+=1;in_vowel=true;}
                else if matches!(next,'e'|'i'|'é'|'í') {out.push("S");in_vowel=false;}
                else {assimilate(&mut out,"N");out.push("k");in_vowel=false;}
            }
            'g' => {assimilate(&mut out,"N");out.push(if matches!(next,'e'|'i'|'é'|'í') {"x"}else{"g"});in_vowel=false;}
            'h' => {}
            'j'|'k'|'q' => {assimilate(&mut out,"N");out.push(if c=='j' {"x"}else{"k"});in_vowel=false;}
            'l' => {out.push(if next=='l' {"ll"}else{"l"});i+=usize::from(next=='l');in_vowel=false;}
            'm'|'p'|'v' => {assimilate(&mut out,"m");out.push(match c {'m'=>"m",'p'=>"p",_=>"b"});in_vowel=false;}
            'ñ' => {assimilate(&mut out,"N");out.push("J");in_vowel=false;}

            'r' => {
                if i==0 {out.push(if chars.len()>3 || trill(last) || last=="f" {"rr"}else{"r"});}
                else if trill(last) {out.push("rr");}
                else if previous=='r' {out.pop();out.push("rr");}
                else {out.push("r");}
                in_vowel=false;
            }
            'n'|'f'|'s'|'t' => {out.push(match c {'n'=>"n",'f'=>"f",'s'=>"s",_=>"t"});in_vowel=false;}
            'w' => {if in_vowel && previous=='u' {out.push("B");}out.push("U");in_vowel=false;}

            'x' => {
                if matches!(word,"xela"|"xicalango"|"xicalangó") {out.push("sh");}
                else if matches!(word,"mexico"|"méxico"|"texas"|"oaxaca"|"quixote") {out.push("x");}
                else if next=='\0' || i==0 || (in_vowel && previous=='e') {assimilate(&mut out,"N");out.extend(["k","s"]);}
                else if in_vowel && (vowel(next)||matches!(next,'y'|'t')) {assimilate(&mut out,"N");out.extend(["g","s"]);}
                else {out.push("s");}
                in_vowel=false;
            }
            'z'|'ç' => {out.push("Z");in_vowel=false;}
            _ => {}
        }
        i+=1;
    }
    out
}
