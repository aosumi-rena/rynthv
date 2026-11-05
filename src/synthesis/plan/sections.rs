use super::{GlobalBlick, OwnerInput};
use crate::model::singer_database;
use serde_json::Value;

impl OwnerInput {
    pub fn sections(&self) -> Vec<(Option<u64>, Self)> {
        let db=singer_database(&self.voice.track_default_database,&self.voice.database);
        if db.get("id").and_then(Value::as_str).is_some_and(|id|
            id.starts_with("r1-std:") || id.starts_with("r2-std:")) {
            return super::standard_sections::sections(self);
        }
        if db.get("backendType").and_then(Value::as_str)!=Some("SVR3") {
            return vec![(None,self.clone())];
        }
        let seconds=|blick:i64| self.tempo.seconds_at(GlobalBlick(blick.saturating_add(self.content_offset.0)));
        let active:Vec<_>=self.notes.iter().enumerate().filter(|(_,n)|
            n.attributes.get("muted").and_then(Value::as_bool)!=Some(true)).collect();
        let mut ranges=Vec::new();let mut first=0;
        let mut covered_end=active.first().map_or(0,|(_,note)|note.onset.0.saturating_add(note.duration));
        for i in 1..active.len() {
            let (_,next)=active[i];
            let end=seconds(covered_end);
            let begin=seconds(next.onset.0);

            let frame=|value:f64| (value/0.005+f64::from_bits(0x3fdf_ffff_ffff_ffff)).trunc();
            if frame(begin)-frame(end)>=200.0 {ranges.push(first..i);first=i;}
            covered_end=covered_end.max(next.onset.0.saturating_add(next.duration));
        }
        if ranges.is_empty() {return vec![(None,self.clone())];}
        ranges.push(first..active.len());
        let (publication_begin,publication_end) = self.publication_bounds();
        let mut result=Vec::new();
        for (index,range) in ranges.iter().enumerate() {
            let (start,first)=active[range.start];let (end,last)=active[range.end-1];
            let mut section=self.clone();section.notes=self.notes[start..=end].to_vec();
            let origin=((seconds(first.onset.0)-0.5)/0.04).floor()*0.04;
            let mut begin=origin;
            if index>0 {
                let (_,previous)=active[ranges[index-1].end-1];
                let previous_end=seconds(previous.onset.0.saturating_add(previous.duration))+0.5;
                if previous_end>begin {begin=(previous_end+seconds(first.onset.0)-0.5)*0.5;}
            }
            let section_begin=GlobalBlick(publication_begin.0.max(self.tempo.position_at(begin).0));
            let mut section_end=publication_end;
            if index+1<ranges.len() {
                let end_seconds=seconds(last.onset.0.saturating_add(last.duration))+0.5;
                let limit=self.tempo.position_at(end_seconds);
                section_end=Some(GlobalBlick(publication_end.map_or(limit.0,|v|v.0.min(limit.0))));
            }
            section.section_bounds=Some((section_begin,section_end));
            result.push((Some(first.id),section));
        }
        result
    }
}
