//! Portable non-bank resources. No editor-installation or machine-path probing.
use std::{collections::HashSet,fs::File,io::Read,path::{Path,PathBuf}};
use super::{StdResources,configuration::require,discovery::{Discovered,read_bounded}};
use super::super::{Result,VoiceError,VoiceFamily};

fn file(path:&Path)->Result<PathBuf> {
    if !path.is_file(){return Err(VoiceError::new(format!("STD resource missing: {}. Install the matching std-resources bundle beside the RynthV executable, then rescan voices.",path.display())));}
    Ok(path.canonicalize()?)
}
fn text(path:&Path)->Result<String> {
    String::from_utf8(read_bounded(&file(path)?,32*1024*1024)?).map_err(|_|VoiceError::new(format!("STD resource must use UTF-8: {}",path.display())))
}
fn phones(path:&Path,expected:Option<&[&str]>)->Result<()> {
    let raw=text(path)?;require(raw.len()<=1_048_576,"STD phone table exceeds 1 MiB")?;
    let rows:Vec<Vec<&str>>=raw.lines().filter(|line|!line.is_empty()).map(|line|line.split_whitespace().collect()).collect();
    require(!rows.is_empty()&&rows.len()<=1024&&rows.iter().all(|r|r.len()>=2),"STD phone table must contain ordered phone/category pairs")?;
    let mut keys=HashSet::new();require(rows.iter().all(|r|keys.insert(r[0]))&&keys.contains("sil"),"STD phone table has duplicate identities or lacks sil")?;
    if let Some(expected)=expected {
        let extra=if expected.len()==57{2}else{1};
        require(rows.len()==expected.len()+extra&&rows.iter().zip(expected).all(|(r,n)|r[0]==*n)&&
            rows.last().is_some_and(|r|r.as_slice()==["br","silence"])&&
            (extra==1||rows[57].as_slice()==["cl","stop"]),"STD phone-table geometry/order does not match the selected bank language")?;
    }
    Ok(())
}
pub(super) fn resolve(bank:&Discovered)->Result<StdResources> {
    let exe=std::env::current_exe()?;
    let root=exe.parent().ok_or_else(||VoiceError::new("RynthV executable has no resource directory"))?.join("std-resources");
    let settings=crate::synthesis::settings::SynthesisSettings::load().map_err(VoiceError::new)?;
    if bank.family==VoiceFamily::R1Std {
        let directory=settings.require_phoneme_directory(crate::synthesis::settings::PhonemeVersion::R1Std).map_err(VoiceError::new)?;
        for name in ["japanese-romaji-dict.txt","japanese-hira2romaji-dict.txt",
            "japanese-kata2romaji-dict.txt","japanese-sute2romaji-dict.txt","edict.txt"] {file(&directory.join(name))?;}
        let table=if directory.join("japanese-romaji.txt").is_file(){"japanese-romaji.txt"}else{"japanese-romaji-phones.txt"};
        phones(&directory.join(table),None)?;
        let renderer=file(&root.join("r1/r1_llsm2.dll"))?;
        let mut input=File::open(&renderer)?;let mut magic=[0;2];input.read_exact(&mut magic)?;
        require(&magic==b"MZ","Invalid R1 renderer DLL in std-resources")?;
        return Ok(StdResources::R1{metadata:bank.path.clone(),data:file(bank.data.as_ref().ok_or_else(||VoiceError::new("R1 data path missing"))?)?,
            language_directory:directory.canonicalize()?,renderer_library:renderer});
    }
    let directory=settings.require_phoneme_directory(crate::synthesis::settings::PhonemeVersion::R2Std).map_err(VoiceError::new)?;
    let japanese=bank.language=="japanese";
    let names=if japanese {"a i u e o N cl t d s sh j z ts k kw g gw h b p f ch ry ky py dy ty ny hy my gy by n m r w v y pau sil"}
        else {r"a A o @ e 7 U u i i\ i` y AU @U ia iA iAU ie iE iU i@U y{ yE ua uA u@ ue uo :\i r\` :n N p ph t th k kh ts\ ts tsh ts` ts`h x f s s` ts\h s\ m n l z` w j pau sil"};
    let expected:Vec<&str>=names.split(' ').collect();
    phones(&directory.join(if japanese{"japanese-romaji-phones.txt"}else{"mandarin-xsampa-phones.txt"}),Some(&expected))?;
    for name in if japanese {&["japanese-romaji-dict.txt","japanese-hira2romaji-dict.txt","japanese-kata2romaji-dict.txt","edict.txt"][..]}
        else {&["mandarin-xsampa-dict.txt","cedict.txt"][..]} {file(&directory.join(name))?;}
    let f0=file(&root.join("r2/f0.dnni"))?;
    let mut input=File::open(&f0)?;require((8..=32*1024*1024).contains(&input.metadata()?.len()),"Invalid shared STD F0 model extent")?;
    let mut head=[0;8];input.read_exact(&mut head)?;
    require(head==[0xff,0x00,0xca,0x7f,1,0,0,0],"Unsupported shared STD F0 model header")?;
    Ok(StdResources::R2 {bank:bank.path.clone(),classifier_directory:directory.canonicalize()?,f0_model:f0})
}
