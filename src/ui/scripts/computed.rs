use crate::{app::App,model::*,synthesis::{plan::OwnerId,rendered::PitchTrack}};
use serde_json::{Value,json};
use std::collections::BTreeMap;
fn sample(track:&PitchTrack,seconds:f64)->Option<f64>{
    let at=(seconds-track.start_seconds)/track.frame_seconds;
    if !at.is_finite()||at<0.||at>track.values.len().saturating_sub(1)as f64{return None;}
    let i=at.floor()as usize;let a=f64::from(*track.values.get(i)?);let b=f64::from(*track.values.get(i+1).unwrap_or(&track.values[i]));
    Some(a+(b-a)*at.fract())
}
pub(super) fn request(app:&App,args:&[Value])->Result<Value,String>{
    let kind=args.first().and_then(Value::as_str).ok_or("Missing computed attribute kind")?;
    let id=|i:usize|args.get(i).and_then(rynthv::scripting::snapshot::read_id).ok_or("Invalid computed group identity");
    let track=app.editor.project.tracks.iter().find(|t|t.id==id(1).unwrap_or(0)).ok_or("Computed group is outside the project")?;
    let reference=track.references().find(|r|r.id==id(2).unwrap_or(0)).ok_or("Computed group is outside the track")?;
    let group=app.editor.project.group(&reference.group_id).ok_or("Missing note group")?;
    if kind=="pitch"{
        let values=args.get(3).and_then(Value::as_array).ok_or("Missing pitch sampling arguments")?;
        let number=|i:usize|values.get(i).and_then(Value::as_f64).filter(|v|v.is_finite()).ok_or("Invalid pitch sampling argument");
        let start=number(0)?;let interval=number(1)?;let count=values.get(2).and_then(Value::as_u64).filter(|v|*v<=2_000_000).ok_or("Invalid pitch frame count")?as usize;
        if interval<=0.{return Err("Pitch sampling interval must be positive".into());}
        if track.is_instrument(){
            let Some(output)=app.synthesis.instruments.outputs.get(&reference.id)else{return Ok(json!([]));};
            if output.pitch.is_empty(){return Ok(json!([]));}
            let result=(0..count).map(|i|{
                let seconds=app.editor.project.time.seconds_at((start+interval*i as f64).round()as i64);
                let at=(seconds-output.origin)*200.;
                if at<0.||at>=output.pitch.len()as f64{return 0.;}
                let a=at.floor()as usize;let first=f64::from(output.pitch[a]);let last=f64::from(*output.pitch.get(a+1).unwrap_or(&output.pitch[a]));
                let hz=first+(last-first)*at.fract();if hz>0.{69.+12.*(hz/440.).log2()}else{0.}
            }).collect::<Vec<_>>();return Ok(json!(result));
        }
        let pieces=app.synthesis.visuals(OwnerId::new(reference.id)).flat_map(|v|v.pieces()).filter(|p|p.snapshot.input.matches_project(&app.editor.project,track,reference,group)).collect::<Vec<_>>();
        if pieces.is_empty(){return Ok(json!([]));}
        let result=(0..count).map(|i|{
            let blick=(start+interval*i as f64).round()as i64;
            let seconds=app.editor.project.time.seconds_at(blick);
            pieces.iter().find(|p|p.range.contains(&blick)).and_then(|p|sample(&p.snapshot.data.pitch,seconds)).unwrap_or(0.)
        }).collect::<Vec<_>>();return Ok(json!(result));
    }
    if track.is_instrument(){
        if kind=="phonemes"{return Ok(json!(vec!["";group.notes.len()]));}
        if kind=="attributes"{
            let Some(output)=app.synthesis.instruments.outputs.get(&reference.id)else{return Ok(json!([]));};
            return Ok(json!(group.notes.iter().map(|n|output.controls.get(&n.id).map(|c|json!({"dynamic":c.dynamic,"articulations":c.articulations})).unwrap_or_else(||json!({}))).collect::<Vec<_>>()));
        }
    }
    let snapshots=app.synthesis.phonemes(OwnerId::new(reference.id)).filter(|s|s.input.matches_project(&app.editor.project,track,reference,group)).collect::<Vec<_>>();
    if snapshots.is_empty(){return Ok(json!([]));}
    let mut phones=BTreeMap::new();
    for snapshot in snapshots{for phone in snapshot.phones.0.iter(){if let Some(source)=phone.source{phones.insert((source.note_id,source.phone_index),phone);}}}
    if kind=="phonemes"{return Ok(json!(group.notes.iter().map(|n|phones.range((n.id,0)..=(n.id,usize::MAX)).map(|(_,p)|p.token.as_str()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>()));}
    if kind!="attributes"{return Err("Unknown computed attribute kind".into());}
    let default=app.synthesis.resolve_voice(track,reference).ok().flatten().and_then(|key|app.synthesis.inventory(&key))
        .and_then(|v|v.configuration.as_ref()).and_then(|c|Language::parse(&c.default_language));
    let language=default.and_then(|d|database_language(&track.main_ref.database,d).ok()).and_then(|d|database_language(&reference.database,d).ok());
    let result=group.notes.iter().map(|note|{
        let language=note.attributes.get("languageOverride").and_then(Value::as_str).filter(|s|!s.is_empty()).and_then(Language::parse).or(language);
        let rap=app.synthesis.visuals(OwnerId::new(reference.id)).flat_map(|v|v.pieces()).filter(|p|p.snapshot.input.matches_project(&app.editor.project,track,reference,group)).flat_map(|p|p.snapshot.data.pitch.rap_controls.iter()).find(|r|r.note_id==note.id);
        let is_rap=note.musical_type=="rap";
        json!({"accent":note.extra.get("accent").and_then(Value::as_str).unwrap_or(""),
            "rapTone":is_rap.then(||note.attributes.get("rTone").and_then(Value::as_f64).or_else(||rap.map(|r|f64::from(r.tone)))).flatten(),
            "rapIntonation":is_rap.then(||note.attributes.get("rIntonation").and_then(Value::as_f64).or_else(||rap.map(|r|f64::from(r.intonation)))).flatten(),
            "phonemes":phones.range((note.id,0)..=(note.id,usize::MAX)).map(|(_,p)|json!({
                "symbol":p.token,"language":if p.language.is_empty(){language.map(Language::id).unwrap_or("")}else{p.language.as_str()},
                "activity":p.has_activity.then_some(p.boundary_strength),"position":p.has_activity.then_some(p.boundary_position)
            })).collect::<Vec<_>>()})
    }).collect::<Vec<_>>();Ok(json!(result))
}
