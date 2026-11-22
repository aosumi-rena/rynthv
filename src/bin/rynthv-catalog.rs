use std::{path::PathBuf,process::ExitCode};

fn instruments(args: impl Iterator<Item=std::ffi::OsString>) -> Result<(),String> {
    use rynthv::synthesis::{
        audio::{ChannelLayout, RenderedAudio, save_wav},
        instruments,
    };
    let mut args = args.peekable();
    let accept_any_v4_signature = args.next_if(|arg| arg == "--accept-any-v4-signature").is_some();
    let root = args.next().map(PathBuf::from).ok_or("Expected an instrument discovery directory")?;
    let catalog = instruments::scan_directory(root, accept_any_v4_signature)?;
    let Some(id) = args.next() else {
        println!("{}",serde_json::to_string(&catalog).map_err(|e| e.to_string())?);
        return Ok(());
    };
    let revision = args.next().ok_or("Expected a revision number")?;
    let score = args.next().map(PathBuf::from).ok_or("Expected an instrument note JSON file")?;
    let wav = match args.next() {
        None => None,
        Some(flag) if flag == "--wav" => Some(
            args.next().map(PathBuf::from).ok_or("Expected an output WAV path")?,
        ),
        Some(_) => return Err("Expected --wav PATH after the note JSON file".into()),
    };
    if args.next().is_some() {
        return Err("Unexpected argument after the output WAV path".into());
    }
    let entry = catalog.entries.iter().find(|e|
        e.selection.database_id == id.to_string_lossy() && e.selection.revision == revision.to_string_lossy()
    ).ok_or_else(||format!("Instrument not discovered: {:?}",catalog.warnings))?;
    let notes = serde_json::from_slice(&std::fs::read(score).map_err(|e|e.to_string())?)
        .map_err(|e|e.to_string())?;
    let audio = instruments::preview(entry,notes,accept_any_v4_signature)?;
    if audio.is_empty() || audio.iter().any(|v|!v.is_finite()) {
        return Err("No finite PCM was emitted".into());
    }
    let peak=audio.iter().map(|x|x.abs()).fold(0.,f32::max);
    let rms=(audio.iter().map(|v|f64::from(*v).powi(2)).sum::<f64>()/audio.len() as f64).sqrt();
    let report = serde_json::json!({"name":entry.name,"frames":audio.len()/2,"peak":peak,
        "rms":rms,"rate":48000,"over_full_scale":audio.iter().filter(|x|x.abs()>1.).count(),
        "wav":wav});
    if let Some(path) = wav {
        let output = RenderedAudio::new(48000, 0., ChannelLayout::Stereo, audio)
            .map_err(|error| error.to_string())?;
        save_wav(&path, &output).map_err(|error| error.to_string())?;
    }
    println!("{report}");
    Ok(())
}

fn main()->ExitCode {
    let mut args=std::env::args_os().skip(1);
    let Some(root)=args.next().map(PathBuf::from) else {
        eprintln!("Usage: rynthv-catalog DATABASE_ROOT | --instruments [--accept-any-v4-signature] ROOT [DATABASE_ID REVISION NOTES [--wav PATH]]");return ExitCode::FAILURE;
    };
    if root.as_os_str()=="--instruments" {
        return match instruments(args) {
            Ok(())=>ExitCode::SUCCESS,
            Err(e)=>{eprintln!("{e}");ExitCode::FAILURE},
        };
    }
    match rynthv::synthesis::voice::VoiceCatalog::scan(&root)
        .and_then(|catalog|serde_json::to_string(&catalog).map_err(|e|rynthv::synthesis::voice::VoiceError::from(std::io::Error::other(e)))) {
        Ok(json)=>{println!("{json}");ExitCode::SUCCESS},
        Err(error)=>{eprintln!("{error}");ExitCode::FAILURE},
    }
}
