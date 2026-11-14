use crate::{model::Note, synthesis::{plan::{NoteInput, OwnerInput}, rendered::{PhonemeTrack, RenderedPhone}}};
use std::{borrow::Cow, collections::HashMap};

pub(super) fn phones<'a>(
    notes: &HashMap<u64, &Note>,
    originals: &HashMap<u64, &NoteInput>,
    input: &OwnerInput,
    track: &'a PhonemeTrack,
) -> Cow<'a, [RenderedPhone]> {
    let changed = originals.iter().any(|(id, old)| notes.get(id).is_some_and(|now|
        now.attributes.get("phonemes") != old.attributes.get("phonemes")));
    if !changed || originals.iter().any(|(id, old)| !notes.get(id).is_some_and(|now|
        now.onset==old.onset.0 && now.duration==old.duration && now.lyrics==old.lyrics && now.phonemes==old.phonemes))
    {
        return Cow::Borrowed(&track.0);
    }
    let mut phones = track.0.to_vec();
    for phone in &mut phones {phone.start_seconds=phone.predicted_start_seconds;phone.end_seconds=phone.predicted_end_seconds;}
    let model_origin=phones.first().map_or(0.0,|p|p.start_seconds);
    let strength_offset=input.voice.instance_options.get("voice")
        .and_then(|v|v.get("consonantStrength")).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
    for index in 0..phones.len() {
        let Some(source)=phones[index].source else {continue;};
        let Some(current)=notes.get(&source.note_id) else {continue;};
        let Some(original)=originals.get(&source.note_id) else {continue;};
        if current.onset!=original.onset.0 || current.duration!=original.duration
            || current.lyrics!=original.lyrics || current.phonemes!=original.phonemes {continue;}
        let now=current.attributes.get("phonemes");
        let row=now.and_then(|v|v.as_array()).and_then(|a|a.get(source.phone_index));
        let read=|key:&str|row.and_then(|v|v.get(key)).and_then(serde_json::Value::as_f64);
        phones[index].strength=read("strength").unwrap_or(if row.is_some(){1.0}else{0.0}) as f32;
        phones[index].boundary_position=read("position").unwrap_or(phones[index].predicted_position).clamp(0.0,1.0);
        if let Some(predicted)=phones[index].predicted_activity {
            let specified=read("activity");
            phones[index].boundary_strength=if specified.is_none() && strength_offset==0.0 {predicted}
                else {f64::from(((specified.unwrap_or(predicted)+f64::from(strength_offset)) as f32).clamp(0.0,1.0))};
        }
        if index>0 && let Some(offset)=read("leftOffset") {
            let origin=input.tempo.seconds_at(crate::synthesis::plan::GlobalBlick(current.onset+input.content_offset.0));
            let frame=((origin+offset-model_origin)/0.005+f64::from_bits(0x3fdf_ffff_ffff_ffff)).trunc();
            let onset=model_origin+frame*0.005;
            phones[index].start_seconds=onset.max(phones[index-1].start_seconds+0.005);
            phones[index-1].end_seconds=phones[index].start_seconds;
            for right in index+1..phones.len() {
                phones[right].start_seconds=phones[right].start_seconds.max(phones[right-1].start_seconds+0.005);
                phones[right-1].end_seconds=phones[right].start_seconds;
            }
            let last=phones.len()-1;
            phones[last].end_seconds=phones[last].end_seconds.max(phones[last].start_seconds+0.005);
        }
    }
    Cow::Owned(phones)
}
