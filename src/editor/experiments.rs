use super::Editor;
use crate::{model::EntityId,synthesis::experiments::ExperimentControls};

impl Editor {
    pub fn set_local_blend_weight(&mut self,key:&str,phone:Option<crate::synthesis::score::PhoneSource>,value:Option<f32>)->Result<(),String> {
        use crate::synthesis::experiments::LocalBlendWeight;
        if self.is_editing() || self.selected.is_empty() {return Err("请先选择音符并完成当前编辑。".into());}
        if value.is_some_and(|v|!v.is_finite() || !(0.0..=1.0).contains(&v)) {return Err("混合比例必须在 0..1 范围内。".into());}
        if phone.is_some_and(|p|!self.selected.contains(&p.note_id)) {return Err("选中的音素已经改变。".into());}
        let mut updates=Vec::new();
        for note in self.notes().iter().filter(|n|self.selected.contains(&n.id) && phone.is_none_or(|p|p.note_id==n.id)) {
            let mut local=LocalBlendWeight::read(&note.extra,key).map_err(|e|e.to_string())?;
            if let Some(phone)=phone {if let Some(value)=value {local.phones.insert(phone.phone_index,value);}else{local.phones.remove(&phone.phone_index);}}
            else{local.note=value;}
            updates.push((note.id,serde_json::to_value(local).map_err(|e|e.to_string())?));
        }
        self.begin("调整局部歌声混合");
        if let Some(group)=self.group_mut() {for (id,value) in updates {
            if let Some(note)=group.notes.iter_mut().find(|n|n.id==id) {
                let fields=note.extra.entry("rynthvBlendWeights".into()).or_insert_with(||serde_json::json!({}));
                if let Some(fields)=fields.as_object_mut() {fields.insert(key.into(),value);}
            }
        }}
        self.finish();Ok(())
    }
    pub fn experiment_controls(&self)->Result<(EntityId,ExperimentControls),String> {
        let track=self.current_track();
        if track.is_instrumental() {return Err("伴奏轨不能设置歌声参数。".into());}
        let reference=self.active_ref().unwrap_or(&track.main_ref);
        ExperimentControls::from_extra(&reference.extra).map(|c|(reference.id,c)).map_err(|e|e.to_string())
    }
    pub fn set_experiment_controls(&mut self,reference:EntityId,controls:ExperimentControls)->Result<bool,String> {
        if self.is_editing() {return Err("请先完成当前编辑。".into());}
        controls.validate().map_err(|e|e.to_string())?;
        let (selected,old)=self.experiment_controls()?;
        if selected!=reference {return Err("选中的音符组已经改变。".into());}
        if old==controls {return Ok(false);}
        let value=serde_json::to_value(controls).map_err(|e|e.to_string())?;
        self.begin("调整实验功能");
        let track=self.project.tracks.iter_mut().find(|t|t.id==self.track).unwrap();
        let target=std::iter::once(&mut track.main_ref).chain(&mut track.groups).find(|r|r.id==reference).unwrap();
        target.extra.insert("rynthvExperiments".into(),value);
        self.finish();
        Ok(true)
    }
}
