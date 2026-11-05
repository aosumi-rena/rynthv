use std::{path::PathBuf,sync::atomic::AtomicBool};
use rynthv::{model::{Project,Note,QUARTER},storage,synthesis::{voice::{VoiceInventory,VoiceCatalog},plan::Planner,engine::standard::StandardRenderer,audio::save_wav}};
#[path = "standard_output/mod.rs"]
mod output;

fn main()->std::process::ExitCode {
    match run(){Ok(())=>std::process::ExitCode::SUCCESS,Err(error)=>{eprintln!("{error}");std::process::ExitCode::FAILURE}}
}
fn run()->Result<(),String> {
    let args=std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg|arg=="--list") {
        let root=args.get(2).ok_or("--list requires a voicebank directory")?;
        let catalog=VoiceCatalog::scan(std::path::Path::new(root)).map_err(|error|error.to_string())?;
        println!("{}",serde_json::to_string_pretty(&catalog).map_err(|error|error.to_string())?);
        return Ok(());
    }
    if args.len()<4 {
        println!("rynthv-std <voicebank-path> <project.svp> <output.wav> [--demo] [--select-voice]\nrynthv-std --list <voicebank-directory>\nRender one STD singing part offline. --demo creates a new short singing project.\n--select-voice applies the explicitly selected bank in memory; the input project is unchanged.");
        return Ok(());
    }
    let profile=PathBuf::from(&args[1]);let project_path=PathBuf::from(&args[2]);let output=PathBuf::from(&args[3]);
    let inventory=VoiceInventory::load(&profile).map_err(|error|error.to_string())?;
    let config=inventory.configuration.as_ref().ok_or("STD voice metadata missing")?;
    if !config.family.is_standard(){return Err("This command requires a STD voice profile".into());}
    let mut project=if args.iter().any(|s|s=="--demo") {
        if project_path.exists(){return Err("The demo project already exists; choose a new filename".into());}
        let mut project=Project::default();project.time.tempo[0].bpm=120.0;
        let track=&mut project.tracks[0];
        track.main_ref.database=serde_json::json!({"id":inventory.database_id,"name":config.name,"version":config.version,"backendType":""});
        let audition:Vec<(&str,i32)>=if config.default_language=="mandarin"{
            vec![("你",60),("好",64),("世",67),("界",62)]
        }else if config.family==rynthv::synthesis::voice::VoiceFamily::R1Std{
            vec![("ka",60),("e",62),("ru",64),("no",65),("u",67),("ta",65),("ga",64)]
        }else{vec![("あ",60),("い",64),("う",67),("え",62),("お",65)]};
        for(i,(lyric,pitch))in audition.into_iter().enumerate(){
            let mut note=Note::new((i as i64+1)*QUARTER,QUARTER,pitch);note.lyrics=lyric.into();track.main_group.notes.push(note);
        }
        storage::save(&project_path,&project)?;project
    }else{storage::load(&project_path)?};
    if args.iter().any(|s|s=="--select-voice") {
        let selected=serde_json::json!({"id":inventory.database_id,"name":config.name,"version":config.version,"backendType":""});
        for track in &mut project.tracks {
            for reference in std::iter::once(&mut track.main_ref).chain(track.groups.iter_mut()) {
                reference.database=selected.clone();
            }
        }
    }
    let mut planner=Planner::default();
    let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    let owner=plan.requests.keys().next().ok_or("No singing part to render")?.0;
    if plan.requests.keys().any(|id|id.0!=owner){return Err("This command renders one singing part; use the editor export for a multitrack project".into());}
    let mut renderer=StandardRenderer::open(&profile).map_err(|error|error.to_string())?;
    let started=std::time::Instant::now();
    let mut requests=plan.requests.values().collect::<Vec<_>>();
    requests.sort_by_key(|r|r.coverage.sample_start.0);
    let note_ids=requests.iter().flat_map(|r|r.input.notes.iter().map(|n|n.id)).collect::<Vec<_>>();
    let mut clips=Vec::new();let mut sections=Vec::new();
    let profile_run=args.iter().any(|s|s=="--profile");
    for request in requests {
        let begin=std::time::Instant::now();
        use sha2::{Digest,Sha256};
        let mut digest=Sha256::new();let mut chunks=0;let mut stream_frames=0;let mut first_pcm=None;let mut phones_ready=None;
        let mut stream_origin_frame=None;let mut stream_clock_valid=true;
        let mut stage_times=Vec::new();
        let mut observer=|event| {
            use rynthv::synthesis::engine::standard::StreamEvent as E;
            match event {
                E::Stage(stage)=>{stage_times.push(serde_json::json!({"stage":format!("{stage:?}"),"at":begin.elapsed().as_secs_f64()}));},
                E::Prepared{..}=>{phones_ready.get_or_insert(begin.elapsed().as_secs_f64());},
                E::Audio{audio,..}=>{
                    first_pcm.get_or_insert(begin.elapsed().as_secs_f64());chunks+=1;
                    let frame=(audio.start_seconds()*f64::from(audio.sample_rate())).round() as i64;
                    let base=*stream_origin_frame.get_or_insert(frame);
                    stream_clock_valid &= frame==base+stream_frames as i64;
                    stream_frames+=audio.frames();
                    if profile_run {for value in audio.samples(){digest.update(value.to_le_bytes());}}
                },
            }
            Ok(())
        };
        let clip=renderer.render_stream(&request.input,&AtomicBool::new(false),&mut observer).map_err(|error|error.to_string())?;
        let stream_hash=profile_run.then(||digest.finalize().iter().map(|v|format!("{v:02x}")).collect::<String>());
        if profile_run {
            let mut actual=Sha256::new();for value in clip.audio.samples(){actual.update(value.to_le_bytes());}
            if !stream_clock_valid || stream_frames!=clip.audio.frames() || stream_hash.as_deref()!=Some(actual.finalize().iter().map(|v|format!("{v:02x}")).collect::<String>().as_str()) {
                return Err("STD delivered PCM chunks differ from the complete render".into());
            }
        }
        let clipped_phones=clip.visuals.phonemes.0.iter().filter(|p|p.source.is_some() && p.error.is_none()
            && (p.start_seconds+1.0/48000.0<clip.audio.start_seconds() || p.end_seconds-1.0/48000.0>clip.audio.end_seconds()))
            .map(|p|serde_json::json!({"token":p.token,"start":p.start_seconds,"end":p.end_seconds})).collect::<Vec<_>>();
        let section=serde_json::json!({"notes":request.input.notes.len(),"elapsed":begin.elapsed().as_secs_f64(),
            "ready_at":started.elapsed().as_secs_f64(),"start_seconds":clip.audio.start_seconds(),"end_seconds":clip.audio.end_seconds(),
            "phones_outside_audio":clipped_phones,"first_pcm":first_pcm,"phones_ready":phones_ready,
            "stream_clock_valid":stream_clock_valid,"stream_origin_frame":stream_origin_frame,"stream_chunks":chunks,"stream_frames":stream_frames,"stream_sha256":stream_hash,"stages":stage_times});
        if args.iter().any(|s|s=="--profile") {eprintln!("{section}");}
        sections.push(section);clips.push(clip);
    }
    let failures=clips.iter().flat_map(|clip|clip.visuals.phonemes.0.iter()).filter_map(|phone|phone.source.zip(phone.error.as_ref()))
        .map(|(source,error)|(source.note_id,error.to_string())).collect::<std::collections::BTreeMap<_,_>>();
    let note_errors=failures.into_iter().map(|(note_id,message)|serde_json::json!({
        "note_index":note_ids.iter().position(|id|*id==note_id),"message":message
    })).collect::<Vec<_>>();
    let audio=output::join(&clips)?;
    save_wav(&output,&audio).map_err(|error|error.to_string())?;
    println!("{}",serde_json::json!({"wav":output,"project":project_path,"rate":audio.sample_rate(),
        "samples":audio.frames(),"start_seconds":audio.start_seconds(),"phones":clips.iter().map(|c|c.visuals.phonemes.0.len()).sum::<usize>(),
        "pitch_points":clips.iter().flat_map(|c|&c.visuals.pitch.segments).map(|s|s.end-s.start).sum::<usize>(),
        "pitch_segments":clips.iter().map(|c|c.visuals.pitch.segments.len()).sum::<usize>(),"note_errors":note_errors,"sections":sections}));
    Ok(())
}
