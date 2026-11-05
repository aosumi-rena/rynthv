//! Read-only public phone-inventory audit across duration/pitch/acoustic stages.
use rynthv::{model::{Language,PhonemeCatalog},synthesis::voice::{VoiceInventory,PreparedModule}};
use serde_json::json;
use std::{ffi::OsString,fs,io::Write,path::PathBuf};

pub(super) fn run(args:&[OsString])->Result<(),Box<dyn std::error::Error>> {
    if args.len()!=3 {return Err("Usage: --phone-routing VOICE CLF_DATA NEW_REPORT".into());}
    let output=PathBuf::from(&args[2]);if output.exists(){return Err("Routing report already exists".into());}
    let inventory=VoiceInventory::load(&PathBuf::from(&args[0]))?;let voice=inventory.prepare()?;
    let config=inventory.configuration.as_ref().ok_or("Missing voice metadata")?;
    let format=rynthv::synthesis::settings::PhonemeVersion::configuration(config,false)?.format();
    let catalog=PhonemeCatalog::load_format(&PathBuf::from(&args[1]),format)?;
    let mut stages=Vec::new();
    for role in ["duration","f0","timbre"] {
        let tree=voice.model(role)?;let mut languages=Vec::new();
        for node in tree.nodes() {
            let PreparedModule::PhoneSet(table)=&node.module else {continue};
            if table.name=="multiphone" {continue;}
            let Some(language)=table.name.split('-').next().and_then(Language::parse) else {continue};
            let mut entries=Vec::new();
            for entry in catalog.for_language(language) {
                let selected=if catalog.uses_system_symbols() {tree.model_phone_token(language.id(),&entry.symbol,&entry.xsampa)}
                    else {tree.native_phone_token(language.id(),&entry.symbol)};
                let local=table.lookup(&entry.symbol);
                let candidates=table.symbols.iter().zip(&table.mapped_symbols)
                    .filter_map(|(symbol,mapped)|(mapped==&entry.xsampa).then_some(symbol)).collect::<Vec<_>>();
                let resolved=selected.as_ref().map_err(|e|e.to_string()).and_then(|token|
                    tree.resolve_phone_alphabet(language.id(),token.strip_prefix("xsampa:").unwrap_or(token),token.starts_with("xsampa:"))
                        .map(|r|json!({"symbol":r.unified_symbol,"index":r.unified_phone_index,"class":r.source_class,"silence_fallback":r.used_silence_fallback}))
                        .map_err(|e|e.to_string()));
                entries.push(json!({"system":entry.symbol,"xsampa":entry.xsampa,
                    "model_local":local.map(|p|p.symbol),"model_local_mapped":local.and_then(|p|p.mapped_symbol),
                    "reverse_candidates":candidates,"selected":selected.map_err(|e|e.to_string()),"resolved":resolved}));
            }
            languages.push(json!({"language":language.id(),"table":table,"entries":entries}));
        }
        stages.push(json!({"role":role,"languages":languages}));
    }
    let report=json!({"scope":"Actual public inventories and model routing, no neural/model/auth/session/settings writes",
        "voice":inventory.display_name,"directory":inventory.directory,"configuration":inventory.configuration,"stages":stages});
    fs::OpenOptions::new().write(true).create_new(true).open(output)?.write_all(&serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
