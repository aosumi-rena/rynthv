use crate::model::PublicModeMap;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StylePreset {
    pub revision: String,

    pub modes: PublicModeMap,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub voice: BTreeMap<String, serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub experiments:Option<crate::synthesis::experiments::ExperimentControls>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StylePresets {
    banks: BTreeMap<String, BTreeMap<String, StylePreset>>,
}

fn bank_id(bank: &str) -> Result<String, String> {
    if let Some(tail)=bank.strip_prefix("sv1:").or_else(||bank.strip_prefix("flat:")) {
        if !tail.is_empty() && tail.len()<=1024 && !tail.chars().any(|c|c.is_control() || matches!(c,'/'|'\\')) {return Ok(bank.to_owned());}
    }
    uuid::Uuid::parse_str(bank).map(|v| v.to_string()).map_err(|_| "歌声 K1 无效".into())
}
fn name(value: &str) -> Result<&str, String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("请输入预设名称，不能含控制字符".into());
    }
    Ok(value)
}

impl StylePreset {
    pub fn validate_for(&self, revision: &str, catalogue: &[String]) -> Result<(), String> {
        if let Some(controls)=&self.experiments {controls.validate().map_err(|e|e.to_string())?;}
        if self.revision.is_empty() { return Err("预设缺少来源版本".into()); }
        for (mode, control) in &self.modes {
            control.validate_storage()?;
            if !catalogue.contains(mode) {
                return Err(format!("预设来自版本 {}，当前版本 {revision} 不支持唱法「{mode}」；未应用", self.revision));
            }
        }
        Ok(())
    }
}

impl StylePresets {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("无法读取唱法预设：{e}")),
        };
        if file.metadata().map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 { return Err("唱法预设超过 2 MiB 上限".into()); }
        let store: Self = serde_json::from_reader(file).map_err(|e| format!("唱法预设格式无效：{e}"))?;
        for (bank, presets) in &store.banks {
            if bank_id(bank)? != *bank { return Err("唱法预设 K1 必须为标准 UUID".into()); }
            for (label, preset) in presets {
                if !label.is_empty() { name(label)?; }
                if preset.revision.is_empty() { return Err("唱法预设缺少来源版本".into()); }
                if let Some(controls)=&preset.experiments {controls.validate().map_err(|e|e.to_string())?;}
                for control in preset.modes.values() { control.validate_storage()?; }
            }
        }
        Ok(store)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > 2 * 1024 * 1024 { return Err("唱法预设超过 2 MiB 上限".into()); }
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        crate::storage::atomic_write(path, &bytes)
    }

    pub fn list(&self, bank: &str) -> impl Iterator<Item = (&String, &StylePreset)> {
        bank_id(bank).ok().and_then(|key| self.banks.get(&key)).into_iter().flat_map(|p| p.iter())
    }

    pub fn get(&self, bank: &str, label: &str) -> Option<&StylePreset> {
        self.banks.get(&bank_id(bank).ok()?)?.get(label)
    }

    pub fn store(&mut self, bank: &str, label: &str, preset: StylePreset, catalogue: &[String]) -> Result<(), String> {
        if !label.is_empty() { name(label)?; }
        preset.validate_for(&preset.revision, catalogue)?;
        self.banks.entry(bank_id(bank)?).or_default().insert(label.into(), preset);
        Ok(())
    }

    pub fn insert(&mut self, bank: &str, label: &str, preset: StylePreset, catalogue: &[String]) -> Result<(), String> {
        name(label)?;
        preset.validate_for(&preset.revision, catalogue)?;
        let entries = self.banks.entry(bank_id(bank)?).or_default();
        if entries.contains_key(label) { return Err("已有同名唱法预设，请使用新名称或先删除旧预设".into()); }
        entries.insert(label.into(), preset);
        Ok(())
    }

    pub fn rename(&mut self, bank: &str, previous: &str, next: &str) -> Result<(), String> {
        name(next)?;
        let entries = self.banks.get_mut(&bank_id(bank)?).ok_or("此歌声没有唱法预设")?;
        if previous == next && entries.contains_key(previous) { return Ok(()); }
        if entries.contains_key(next) { return Err("已有同名唱法预设".into()); }
        let value = entries.remove(previous).ok_or("预设已不存在")?;
        entries.insert(next.into(), value);
        Ok(())
    }

    pub fn remove(&mut self, bank: &str, label: &str) -> Result<(), String> {
        let entries = self.banks.get_mut(&bank_id(bank)?).ok_or("此歌声没有唱法预设")?;
        entries.remove(label).ok_or("预设已不存在")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bank_isolation_restart_lanes_revision_and_named_operations() {
        let bank=uuid::Uuid::new_v4().to_string();
        let other_bank=uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir().join(format!("rynthv-presets-{}.json", uuid::Uuid::new_v4()));
        let modes = serde_json::from_value(serde_json::json!({"Bright":{"pitch":0.0,"timbre":51.25,"pronunciation":27.5},"Smooth":{}})).unwrap();
        let catalogue = vec!["Bright".into(),"Smooth".into()];
        let preset = StylePreset {revision:"202".into(),modes,voice:Default::default(),experiments:None};
        let mut store = StylePresets::default();
        store.insert(&bank,"Mixed",preset.clone(),&catalogue).unwrap();
        assert!(store.insert(&bank,"Mixed",preset.clone(),&catalogue).is_err());
        assert_eq!(store.list(&other_bank).count(),0);
        store.save(&path).unwrap();
        let mut restored = StylePresets::load(&path).unwrap();
        assert_eq!(restored.list(&bank).next().unwrap().1,&preset);
        preset.validate_for("203",&catalogue).unwrap();
        assert!(preset.validate_for("203",&["Bright".into()]).is_err());
        restored.rename(&bank,"Mixed","Next").unwrap();
        restored.save(&path).unwrap();
        assert_eq!(StylePresets::load(&path).unwrap().list(&bank).next().unwrap().0,"Next");
        restored.remove(&bank,"Next").unwrap();restored.save(&path).unwrap();
        assert_eq!(StylePresets::load(&path).unwrap().list(&bank).count(),0);
        std::fs::remove_file(path).unwrap();
    }
}
