use crate::synthesis::{
    stages::{duration::DurationRecord, f0::frontend::FrontendNote},
    voice::{PreparedTree, Result, VoiceError, VoiceFamily},
};

pub(super) fn link_sustains(records: &mut [DurationRecord], notes: &[FrontendNote]) -> Result<()> {
    let mut previous: Option<usize> = None;
    for (index, note) in notes.iter().enumerate() {
        if note.kind == 2 {
            let previous = previous.ok_or_else(|| VoiceError::new("延音前缺少音符记录"))?;
            let seconds = crate::synthesis::stages::f0::frontend::frame_count(note)? as f64 * note.frame_seconds;
            records[previous].linked_durations_seconds.push(seconds);
        } else if !records[index].phones.is_empty() { previous=Some(index); }
    }
    Ok(())
}

pub(super) fn redistribute_onsets(
    tree: &PreparedTree,
    records: &mut [DurationRecord],
    notes: &mut [FrontendNote],
    phone_sources:&mut [Vec<Option<super::PhoneSource>>],
    family: VoiceFamily,
) -> Result<()> {
    if records.len() != notes.len() || records.len()!=phone_sources.len() {
        return Err(VoiceError::new(
            "Onset redistribution requires matching frontend records",
        ));
    }

    let mut prefixes = Vec::with_capacity(records.len());
    for record in records.iter() {
        let mut first_core = None;
        for (index, phone) in record.phones.iter().enumerate() {
            let resolved = tree.resolve_phone_alphabet(&phone.language, &phone.token,phone.xsampa)?;
            let core = match family {
                VoiceFamily::R1Std | VoiceFamily::R2Std => return Err(VoiceError::new("STD banks use their own phoneme allocation")),

                VoiceFamily::Sv1 => {
                    matches!(resolved.source_class, Some("vowel" | "diphthong"))
                }

                VoiceFamily::Sv2Compatible | VoiceFamily::Sv2 => {
                    matches!(phone.token.as_str(), "sil" | "pau")
                        || matches!(
                            resolved.source_class,
                            Some("vowel" | "diphthong" | "silence" | "breath")
                        )
                }
            };
            if core && first_core.is_none() {
                first_core = Some(index);
            }
        }
        prefixes.push(first_core.unwrap_or(0));
    }

    let mut previous: Option<usize> = None;
    for (index, &prefix) in prefixes.iter().enumerate() {

        if notes[index].kind == 2 {
            continue;
        }
        if let Some(previous) = previous
            && prefix > 0
        {
            let (earlier, current) = records.split_at_mut(index);
            earlier[previous]
                .phones
                .extend(current[0].phones.drain(..prefix));
            let (earlier,current)=phone_sources.split_at_mut(index);
            earlier[previous].extend(current[0].drain(..prefix));
        }
        previous = Some(index);
    }

    for (record, note) in records.iter().zip(notes) {
        note.phones = record
            .phones
            .iter()
            .map(|phone| phone.token.clone())
            .collect();
    }
    Ok(())
}
