use crate::synthesis::{score::PreparedPhrase,stages::f0::frontend::frame_count,voice::{Result,VoiceError}};
const MULTIPLIER:u64=0x5851_f42d_4c95_7f2d;
const FRAME_INCREMENT:u64=0x8765_4321;

pub fn for_phrase(phrase:&PreparedPhrase,first_frame:usize)->Result<Vec<u64>> {
    if phrase.notes.len()!=phrase.retake_seeds.len() {return Err(VoiceError::new("Vocoder seed records and retakes differ"));}
    let mut output=Vec::new();
    for (index,(note,retake)) in phrase.notes.iter().zip(&phrase.retake_seeds).enumerate() {
        let count=frame_count(note)?;
        let increment=retake.map_or(1,|s|u64::from(s.timbre)*2+1);
        let sources=phrase.phone_sources.get(index).filter(|sources|sources.len()==note.phones.len())
            .ok_or_else(||VoiceError::new("Vocoder phone ownership differs from its record"))?;
        let tokens=note.phones.iter().zip(sources).map(|(token,source)|
            source.and_then(|source|phrase.vocoder_phone_labels.get(&source)).unwrap_or(token).clone()
        ).collect::<Vec<_>>();
        append(&mut output,&tokens,count,increment)?;
    }
    if first_frame>output.len() {return Err(VoiceError::new("Vocoder seed origin exceeds record timeline"));}
    Ok(output.split_off(first_frame))
}

fn text_hash(phones:&[String])->u64 {
    let mut text=String::new();
    for phone in phones {text.push_str(phone);text.push(',');}
    text.push('_');
    xxhash_rust::xxh3::xxh3_128(text.as_bytes()) as u64
}

fn append(output:&mut Vec<u64>,phones:&[String],frames:usize,increment:u64)->Result<()> {
    output.try_reserve(frames).map_err(|_|VoiceError::new("Could not allocate vocoder seed timeline"))?;
    let mut state=text_hash(phones).wrapping_add(increment).wrapping_mul(MULTIPLIER).wrapping_add(increment);
    for _ in 0..frames {
        output.push(state);
        state=state.wrapping_mul(MULTIPLIER).wrapping_add(FRAME_INCREMENT);
    }
    Ok(())
}
