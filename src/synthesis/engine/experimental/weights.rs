use crate::synthesis::{experiments::{BlendTarget,LocalBlendWeight},plan::{OwnerInput,GlobalBlick},voice::{Result,VoiceError}};

#[derive(Default)]
pub(super) struct LocalWeights { notes:Vec<[f64;3]>,phones:Vec<[f64;3]> }
impl LocalWeights {
    pub fn new(input:&OwnerInput,primary:&super::RenderResult,target:&BlendTarget)->Result<Self> {
        let mut result=Self::default();
        let key=format!("{}:{}",target.voice.database_id,target.voice.revision);
        for note in &input.notes {
            let local=LocalBlendWeight::read(&note.extensions,&key)?;
            if let Some(value)=local.note {
                let begin=input.content_offset.0.checked_add(note.onset.0).ok_or_else(||VoiceError::new("Local blend placement overflow"))?;
                let end=begin.checked_add(note.duration).ok_or_else(||VoiceError::new("Local blend end overflow"))?;
                result.notes.push([input.tempo.seconds_at(GlobalBlick(begin)),input.tempo.seconds_at(GlobalBlick(end)),f64::from(value)]);
            }
            for (index,value) in local.phones {
                let mut found=false;
                for phone in primary.visuals.phonemes.0.iter().filter(|p|p.source.is_some_and(|s|s.note_id==note.id && s.phone_index==index)) {
                    result.phones.push([phone.start_seconds,phone.end_seconds,f64::from(value)]);found=true;
                }
                if !found {return Err(VoiceError::new("局部混合所引用的音素已经改变；请重新选择音素。"));}
            }
        }
        result.notes.sort_by(|a,b|a[0].total_cmp(&b[0]));result.phones.sort_by(|a,b|a[0].total_cmp(&b[0]));
        Ok(result)
    }
    pub fn base(&self,time:f64,default:f32)->f32 {
        let find=|rows:&[[f64;3]]| {
            let end=rows.partition_point(|r|r[0]<=time);
            end.checked_sub(1).map(|i|rows[i]).filter(|r|time<r[1]).map(|r|r[2] as f32)
        };
        find(&self.phones).or_else(||find(&self.notes)).unwrap_or(default)
    }
}
