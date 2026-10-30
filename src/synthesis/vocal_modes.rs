use std::collections::BTreeMap;
use crate::model::PublicModeMap;
use super::{plan::OwnerInput, voice::{PreparedVoice, PreparedTree, Result, VoiceError, VoiceFamily}};

#[derive(Clone, Copy, Debug, PartialEq)]
struct ModeWeight { pitch: f32, primary_ppg: f32, secondary_timbre: f32 }

#[derive(Clone, Debug, PartialEq)]
pub struct VocalModeCoefficients {
    values: BTreeMap<String, ModeWeight>,
    automation: Option<crate::synthesis::score::PreparedAutomation>,
    origin_seconds: f64,
    normalization:Vec<(String,f32)>,
    experimental_range:Option<(f32,f32)>,
}

pub fn validate_public(controls: &PublicModeMap) -> Result<()> {
    for control in controls.values() {
        control.validate().map_err(VoiceError::new)?;
    }
    Ok(())
}

pub fn resolve_for_voice(voice: &PreparedVoice, controls: Option<&PublicModeMap>)
    -> Result<Option<VocalModeCoefficients>>
{
    resolve_for_voice_with_range(voice,controls,None)
}

fn resolve_for_voice_with_range(voice:&PreparedVoice,controls:Option<&PublicModeMap>,range:Option<(f32,f32)>)
    ->Result<Option<VocalModeCoefficients>> {
    let Some(controls) = controls.filter(|modes| !modes.is_empty()) else { return Ok(None); };
    if voice.family==VoiceFamily::Sv1 && range.is_none() {
        return Err(VoiceError::new("Vocal modes require an R3 voice"));
    }
    if !voice.speakers.is_empty() {
        return Err(VoiceError::new("Choir coefficients must be resolved separately for each selected member"));
    }
    if let Some(id)=voice.speaker
        && !voice.member_vocal_modes.iter().any(|m|m.speaker_id==id && Some(m.public_names.as_slice())==voice.vocal_modes.as_deref())
    {
        return Err(VoiceError::new("Selected choir member has no matching vocal-mode catalogue"));
    }
    let catalogue = voice.vocal_modes.as_deref().ok_or_else(||
        VoiceError::new("Vocal-mode catalogue has not been loaded"))?;
    resolve_registered_with_range(controls, catalogue,range).map(Some)
}

pub fn validate_for_voice(voice:&PreparedVoice,controls:Option<&PublicModeMap>) -> Result<()> {
    if voice.speakers.is_empty() {resolve_for_voice(voice,controls)?;}
    else {for speaker in &voice.speakers {resolve_for_voice(&voice.member(speaker)?,controls)?;}}
    Ok(())
}

#[cfg(test)]
fn resolve_registered(controls: &PublicModeMap, catalogue: &[String]) -> Result<VocalModeCoefficients> {
    resolve_registered_with_range(controls,catalogue,None)
}
fn resolve_registered_with_range(controls:&PublicModeMap,catalogue:&[String],range:Option<(f32,f32)>)->Result<VocalModeCoefficients> {
    if let Some((minimum,maximum))=range {
        if minimum < -3.0 || maximum>3.0 || minimum>0.0 || maximum<1.5 {return Err(VoiceError::new("Invalid experimental style range"));}
        for value in controls.values().flat_map(|c|[c.pitch,c.timbre,c.pronunciation]).flatten() {
            if !value.is_finite() || value<f64::from(minimum)*100.0 || value>f64::from(maximum)*100.0 {return Err(VoiceError::new("Experimental style value is outside the selected range"));}
        }
    } else {validate_public(controls)?;}
    let mut values = BTreeMap::new();
    for (name, control) in controls {
        if !catalogue.contains(name) {
            continue;
        }
        let percent = |value: Option<f64>| (value.unwrap_or(0.0) as f32) / 100.0_f32;
        values.insert(name.clone(), ModeWeight {
            pitch: percent(control.pitch),
            primary_ppg: percent(control.pronunciation),
            secondary_timbre: percent(control.timbre),
        });
    }
    Ok(VocalModeCoefficients { values, automation:None, origin_seconds:0.0,normalization:Vec::new(),experimental_range:range })
}

pub fn for_phrase(voice:&PreparedVoice, phrase:&crate::synthesis::score::PreparedPhrase)
    ->Result<Option<VocalModeCoefficients>>
{
    let mut controls=phrase.vocal_modes.clone().unwrap_or_default();
    let mut has_curves=false;
    if let Some(curves)=&phrase.automation {
        for name in curves.mode_names() {
            if voice.vocal_modes.as_deref().is_some_and(|names|names.iter().any(|n|n==name)) {
                controls.entry(name.into()).or_default();has_curves=true;
            }
        }
    }
    let mut coefficients=resolve_for_voice_with_range(voice,Some(&controls),phrase.experimental_style_range)?;
    if has_curves && let Some(coefficients)=&mut coefficients {
        coefficients.automation=phrase.automation.clone();
        coefficients.origin_seconds=phrase.start_seconds;
    }
    if let Some(coefficients)=&mut coefficients && voice.family!=VoiceFamily::Sv1 {
        let tree=voice.model("timbre")?;
        let node=tree.node(tree.root_index())?.children[1];
        let node=crate::synthesis::voice::speakers::style(tree,node,voice.speaker)?;
        let module=tree.node(node)?;
        let normalizer=*module.children.get(1).ok_or_else(||VoiceError::new("Missing mode normalization coefficients"))?;
        let values=tree.tensor(normalizer)?.vector().ok_or_else(||VoiceError::new("Mode normalization is not a vector"))?;
        let names=ordered_style_names(tree,node,values.len())?;
        if values.iter().any(|v|!v.is_finite()) {return Err(VoiceError::new("Nonfinite mode normalization coefficient"));}
        coefficients.normalization=names.into_iter().zip(values.iter().copied()).collect();
    }
    Ok(coefficients)
}

impl VocalModeCoefficients {
    pub fn normalization_at(&self,frame:usize)->f32 {
        let mut logarithm=0.0_f32;
        for (name,coefficient) in &self.normalization {
            let value=self.at(name,self.values.get(name).map_or(0.0,|v|v.secondary_timbre),frame,"timbre");
            logarithm+=value*coefficient;
        }
        logarithm.exp().max(0.000_01)
    }
    pub fn animated(&self)->bool {self.automation.is_some()}
    fn at(&self,name:&str,base:f32,frame:usize,component:&str)->f32 {
        match &self.automation {
            None=>base,
            Some(curves)=>{
                let seconds=self.origin_seconds+frame as f64*crate::synthesis::score::FRAME_SECONDS;
                let mut value=base+curves.value(&format!("vocalMode_{name}"),seconds,0.0)/100.0;
                if self.experimental_range.is_some() {
                    let extra=curves.value(&format!("vocalModeComponent_{component}:{name}"),seconds,0.0)/100.0;
                    if extra!=0.0 {value+=extra;}
                }
                let (minimum,maximum)=self.experimental_range.unwrap_or((0.0,1.5));
                value.clamp(minimum,maximum)
            },
        }
    }
    pub fn pitch_weights_at(&self,names:&[String],frame:usize)->Vec<f32> {
        names.iter().map(|name|self.at(name,self.values.get(name).map_or(0.0,|v|v.pitch),frame,"pitch")).collect()
    }
    pub fn weights_at(&self,names:&[String],primary:bool,frame:usize)->Vec<f32> {
        names.iter().map(|name|self.at(name,self.values.get(name).map_or(0.0,|v|
            if primary {v.primary_ppg}else{v.secondary_timbre}),frame,if primary {"pronunciation"}else{"timbre"})).collect()
    }
    pub fn pitch_weights(&self, model_names: &[String]) -> Vec<f32> {
        model_names.iter().map(|name| self.values.get(name).map_or(0.0, |v| v.pitch)).collect()
    }
    pub fn weights(&self, model_names: &[String], primary: bool) -> Vec<f32> {
        model_names.iter().map(|name| self.values.get(name).map_or(0.0, |v|
            if primary {v.primary_ppg} else {v.secondary_timbre})).collect()
    }
}

pub fn from_owner(input: &OwnerInput, family: VoiceFamily) -> Result<Option<PublicModeMap>> {
    from_voice_options(&input.voice.instance_options, family)
}

fn from_voice_options(instance: &crate::model::Extra,
    family: VoiceFamily) -> Result<Option<PublicModeMap>>
{
    let modes = |options: &crate::model::Extra| -> Result<Option<PublicModeMap>> {
        options.get("voice").and_then(|voice| voice.get("vocalModeParams"))
            .map(|value| serde_json::from_value(value.clone())
                 .map_err(|error| VoiceError::new(format!("Invalid vocal-mode object: {error}"))))
            .transpose()
    };
    let local = modes(instance)?;
    if let Some(local) = &local {
        if !local.is_empty() && !matches!(family,VoiceFamily::Sv2|VoiceFamily::Sv2Compatible) {
            return Err(VoiceError::new("Acoustic vocal modes require R3"));
        }
        validate_public(local)?;
    }
    Ok(local)
}

pub fn ordered_style_names(tree: &PreparedTree, node: usize, expected: usize) -> Result<Vec<String>> {
    let bytes = tree.payload(node)?;
    let mut offset = 0usize;
    let u32_at = |offset: &mut usize| -> Result<u32> {
        let end = offset.checked_add(4).filter(|end| *end <= bytes.len())
            .ok_or_else(|| VoiceError::new("Truncated acoustic mode name metadata"))?;
        let value = u32::from_le_bytes(bytes[*offset..end].try_into().expect("bounded"));
        *offset = end; Ok(value)
    };
    let count = u32_at(&mut offset)? as usize;
    if count != expected || count > bytes.len() / 4 {
        return Err(VoiceError::new("Acoustic mode names differ from the named basis rows"));
    }
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let length = u32_at(&mut offset)? as usize;
        let end = offset.checked_add(length).filter(|end| *end <= bytes.len())
            .ok_or_else(|| VoiceError::new("Truncated acoustic mode name"))?;
        let name = std::str::from_utf8(&bytes[offset..end])
            .map_err(|_| VoiceError::new("Invalid acoustic mode name"))?.to_owned();
        if names.contains(&name) { return Err(VoiceError::new("Duplicate acoustic mode name")); }
        names.push(name); offset = end;
    }
    if offset != bytes.len() { return Err(VoiceError::new("Trailing acoustic mode name metadata")); }
    Ok(names)
}

#[cfg(test)]
mod acoustic_mode_tests {
    use super::*;
    use crate::model::PublicModeControl;
    fn modes(timbre: f64, pronunciation: f64) -> PublicModeMap {
        BTreeMap::from([("Breathy".into(), PublicModeControl {
            pitch: Some(0.0), timbre: Some(timbre), pronunciation: Some(pronunciation)
        })])
    }
    #[test]
    fn observed_fractional_api_rounding_and_role_mapping() {
        let model = resolve_registered(&modes(25.123456, 12.34567), &["Breathy".into()]).unwrap();
        assert_eq!(model.weights(&["Breathy".into()], true)[0].to_bits(), 1039980253);
        assert_eq!(model.weights(&["Breathy".into()], false)[0].to_bits(), 1048617425);
        assert_ne!(((25.123456_f64 as f32)*0.01_f32).to_bits(), 1048617425);
        assert_ne!((12.34567_f64/100.0) as f32, model.weights(&["Breathy".into()], true)[0]);
    }
    #[test]
    fn observed_endpoint_and_serialized_model_order() {
        let model = resolve_registered(&modes(100.0, 150.0), &["Breathy".into()]).unwrap();
        assert_eq!(model.weights(&["AbsentRole".into(), "Breathy".into()], true), [0.0,1.5]);
        assert_eq!(model.weights(&["Breathy".into(), "AbsentRole".into()], false), [1.0,0.0]);
        let retained_previous_singer=resolve_registered(&modes(25.0,0.0), &[]).unwrap();
        assert_eq!(retained_previous_singer.weights(&["Breathy".into()],false),[0.0]);
    }
    #[test]
    fn invalid_controls_fail_and_pitch_retains_its_own_lane() {
        for v in [f64::NAN, f64::INFINITY, -1.0, 150.001] {
            assert!(resolve_registered(&modes(v,0.0), &["Breathy".into()]).is_err());
        }
        let mut values = modes(25.0,0.0);
        values.get_mut("Breathy").unwrap().pitch = Some(25.0);
        let model = resolve_registered(&values, &["Breathy".into()]).unwrap();
        assert_eq!(model.pitch_weights(&["Breathy".into()]), [0.25]);
        assert_eq!(model.weights(&["Breathy".into()], true), [0.0]);
    }

    #[test]
    fn native_inherited_true_does_not_erase_local_mode() {
        let instance = BTreeMap::from([("voice".into(), serde_json::json!({
            "vocalModeInherited": true,
            "vocalModeParams": {"Breathy": {"pitch":0,"timbre":25,"pronunciation":0}}
        }))]);
        let actual = from_voice_options(&instance, VoiceFamily::Sv2).unwrap().unwrap();
        assert_eq!(actual["Breathy"].timbre, Some(25.0));
        assert!(from_voice_options(&instance, VoiceFamily::Sv1).is_err());
        assert_eq!(from_voice_options(&instance, VoiceFamily::Sv2Compatible).unwrap().unwrap(),actual);
    }

    #[test]
    fn absent_empty_and_explicit_zero_remain_distinct_at_owner_boundary() {
        let absent = BTreeMap::new();
        let empty = BTreeMap::from([("voice".into(), serde_json::json!({"vocalModeParams":{}}))]);
        let zero = BTreeMap::from([("voice".into(), serde_json::json!({
            "vocalModeParams":{"Breathy":{"timbre":0}}
        }))]);
        assert_eq!(from_voice_options(&absent,VoiceFamily::Sv2).unwrap(), None);
        assert_eq!(from_voice_options(&empty,VoiceFamily::Sv2).unwrap(), Some(BTreeMap::new()));
        let actual = from_voice_options(&zero,VoiceFamily::Sv2).unwrap().unwrap();
        assert_eq!(actual["Breathy"].timbre, Some(0.0));
        assert_eq!(actual["Breathy"].pronunciation, None);
    }
}
