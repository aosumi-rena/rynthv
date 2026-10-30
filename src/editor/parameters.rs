use super::Editor;
use crate::{model::EntityId, synthesis::parameters::ParameterSpec};
use serde_json::{Map, Value};

impl Editor {
    pub fn voice_parameter(&self,spec:ParameterSpec) -> Result<(EntityId, f32), String> {
        let track = self.current_track();
        if track.is_instrumental() { return Err("伴奏轨没有歌声参数".into()); }
        let reference = self.active_ref().unwrap_or(&track.main_ref);
        let value=reference.extra.get("voice").and_then(|v|v.get(spec.key))
            .map_or(Ok(0.0),|v|v.as_f64().filter(|v|v.is_finite()).map(|v|v as f32)
                .ok_or_else(||format!("{}参数无效",spec.label)))?;
        Ok((reference.id,value))
    }
    pub fn set_voice_parameter(&mut self, reference_id: EntityId,spec:ParameterSpec, value: f32) -> Result<bool, String> {
        if self.is_editing() { return Err("请先完成当前编辑".into()); }
        if !value.is_finite() || value.abs()>spec.limit { return Err(format!("{}必须在 {}..{} 范围内",spec.label,-spec.limit,spec.limit)); }
        let (selected, previous) = self.voice_parameter(spec)?;
        if selected != reference_id { return Err("选中的音符组已经改变".into()); }
        if value == previous { return Ok(false); }
        let reference = self.active_ref().unwrap_or(&self.current_track().main_ref);
        let mut next = reference.extra.clone();
        next.entry("voice".into()).or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut().ok_or("voice 字段必须是对象")?
            .insert(spec.key.into(), Value::from(value));
        if crate::synthesis::parameters::VOICE_CONTROLS.iter().any(|s|s.key==spec.key && spec.limit>s.limit) {
            let mut controls=crate::synthesis::experiments::ExperimentControls::from_extra(&next).map_err(|e|e.to_string())?;
            controls.extended_expression=true;
            next.insert("rynthvExperiments".into(),serde_json::to_value(controls).map_err(|e|e.to_string())?);
        }
        self.begin(&format!("调整歌声{}",spec.label));
        let track = self.project.tracks.iter_mut().find(|track| track.id == self.track).expect("selected track exists");
        std::iter::once(&mut track.main_ref).chain(&mut track.groups)
            .find(|reference| reference.id == reference_id).expect("validated reference").extra = next;
        self.finish();
        Ok(true)
    }
}
