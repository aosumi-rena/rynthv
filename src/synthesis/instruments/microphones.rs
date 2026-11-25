use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Named<T> { pub key: String, pub value: T }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel { pub gain: f32, pub pan: f32 }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group { pub mics: Vec<Named<Channel>>, pub mute: bool, pub solo: bool, pub link: bool }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all="camelCase")]
pub struct Room { pub name: String, pub mic_arrays: Vec<Named<Group>> }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all="camelCase")]
pub struct Configuration { pub instrument_mics: Group, pub room: Room }
#[derive(Clone, Debug, Serialize)]
pub struct Mix { pub name: String, pub gain: f32, pub pan: f32 }

pub fn split(name:&str)->(&str,&str) {name.split_once('-').unwrap_or((name,""))}
fn unique<T>(items:&[Named<T>])->Result<(),String> {
    let mut names=BTreeSet::new();
    if items.iter().any(|v|!names.insert(&v.key)) {return Err("Duplicate instrument microphone setting".into());}
    Ok(())
}
impl Configuration {
    pub fn parse(value:&serde_json::Value)->Result<Self,String> {
        let result:Self=serde_json::from_value(value.clone()).map_err(|e|format!("Invalid microphone settings: {e}"))?;
        unique(&result.room.mic_arrays)?;
        for group in std::iter::once(&result.instrument_mics).chain(result.room.mic_arrays.iter().map(|v|&v.value)) {
            unique(&group.mics)?;
            if group.mics.iter().any(|v|!v.value.gain.is_finite() || v.value.gain<0. || !v.value.pan.is_finite()) {
                return Err("Invalid instrument microphone gain or pan".into());
            }
        }
        Ok(result)
    }
    pub fn read(value:Option<&serde_json::Value>,names:&[String],room:Option<&str>)->Result<Self,String> {
        let source=value.map(Self::parse).transpose()?.unwrap_or_default();
        let implicit=source==Self::default();
        let mut result=source.clone();
        if result.room.name.is_empty() {result.room.name=room.unwrap_or_default().into();}
        for name in names {
            let (group,channel)=split(name);
            if group!="Close" && !result.room.mic_arrays.iter().any(|v|v.key==group) {
                result.room.mic_arrays.push(Named {key:group.into(),value:Group {
                    mute:implicit&&group!="Decca",..Group::default()
                }});
            }
            let group=result.group_mut(group).expect("materialized microphone group");
            if !group.mics.iter().any(|v|v.key==channel) {
                let gain=if implicit&&split(name).0=="Close"&&!group.mics.is_empty(){0.}else{1.};
                group.mics.push(Named {key:channel.into(),value:Channel {gain,pan:match channel {"L"=>-1.,"R"=>1.,_=>0.}}});
            }
        }
        result.room.mic_arrays.sort_by(|a,b|a.key.cmp(&b.key));
        for group in std::iter::once(&mut result.instrument_mics).chain(result.room.mic_arrays.iter_mut().map(|v|&mut v.value)) {
            group.mics.sort_by(|a,b|a.key.cmp(&b.key));
        }
        Ok(result)
    }
    pub fn group(&self,name:&str)->Option<&Group> {
        if name=="Close" {Some(&self.instrument_mics)} else {self.room.mic_arrays.iter().find(|v|v.key==name).map(|v|&v.value)}
    }
    pub fn group_mut(&mut self,name:&str)->Option<&mut Group> {
        if name=="Close" {Some(&mut self.instrument_mics)} else {self.room.mic_arrays.iter_mut().find(|v|v.key==name).map(|v|&mut v.value)}
    }
    pub fn mix(&self,names:&[String])->Vec<Mix> {
        let solo=self.instrument_mics.solo || self.room.mic_arrays.iter().any(|v|v.value.solo);
        let mut sorted=names.to_vec();sorted.sort();
        sorted.into_iter().filter_map(|name| {
            let (g,c)=split(&name);let group=self.group(g)?;
            if group.mute || (solo&&!group.solo) {return None;}
            let channel=&group.mics.iter().find(|v|v.key==c)?.value;
            (channel.gain!=0.).then(||Mix {name:name.clone(),gain:channel.gain,pan:channel.pan.clamp(-1.,1.)})
        }).collect()
    }
}
