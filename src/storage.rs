use crate::model::*;
use std::{collections::HashSet, fs, io::Write, path::Path};

mod identity;
mod audio;

/// Update native audio-reference metadata in memory; never writes a file.
pub fn synchronize_audio_references(project: &mut Project) { audio::save(project); }

pub fn load(path: &Path) -> Result<Project, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("工程超过 64 MiB 读取上限".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    load_bytes(&bytes,path)
}

pub fn load_bytes(bytes:&[u8],path:&Path)->Result<Project,String> {

    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let bytes = bytes.trim_ascii_end().strip_suffix(&[0]).unwrap_or(bytes).trim_ascii_end();
    let mut project: Project =
        serde_json::from_slice(bytes).map_err(|e| format!("SVP 解析失败：{e}"))?;
    audio::load(&mut project)?;
    validate(&project)?;
    let base = std::path::absolute(path).map_err(|e| e.to_string())?;
    for track in &mut project.tracks {
        if let Some(clip) = track.instrumental.as_mut().and_then(|t| t.clip.as_mut())
            && clip.path.is_relative()
        {
            clip.path = base.parent().unwrap().join(&clip.path);
        }
    }
    project.tracks.sort_by_key(|t| t.disp_order);
    for group in project
        .library
        .iter_mut()
        .chain(project.tracks.iter_mut().map(|t| &mut t.main_group))
    {
        group.notes.sort_by_key(|n| n.onset);
    }
    Ok(project)
}

pub fn save(path: &Path, project: &Project) -> Result<(), String> {
    validate(project)?;
    let mut ordered = project.clone();

    ordered.version = SVP_VERSION;
    let base = std::path::absolute(path).map_err(|e| e.to_string())?;
    for track in &mut ordered.tracks {
        if let Some(clip) = track.instrumental.as_mut().and_then(|t| t.clip.as_mut())
            && let Ok(relative) = clip.path.strip_prefix(base.parent().unwrap())
        {
            clip.path = relative.to_owned();
        }
    }
    audio::save(&mut ordered);
    for group in ordered
        .library
        .iter_mut()
        .chain(ordered.tracks.iter_mut().map(|t| &mut t.main_group))
    {
        group.notes.sort_by_key(|n| n.onset);
    }
    let bytes = serde_json::to_vec_pretty(&ordered).map_err(|e| e.to_string())?;
    atomic_write(path, &bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|e| format!("写入 {} 失败：{e}", path.display()))
}

pub(crate) fn validate(p: &Project) -> Result<(), String> {
    let bad = |s: &str| Err(format!("工程校验失败：{s}"));
    if !(113..=SVP_VERSION).contains(&p.version) {
        return bad(&format!(
            "支持 SVP version 113 至 {SVP_VERSION}，文件版本为 {}",
            p.version
        ));
    }
    identity::validate(p)?;
    if p.tracks.is_empty() {
        return bad("至少需要一条音轨");
    }
    for (name, dictionary) in &p.dictionaries {
        if dictionary.xsampa_entries.iter().any(|word|!dictionary.entries.contains_key(word)) {
            return bad("发音词典的字词或音素无效");
        }
        if name.trim().is_empty() || name != name.trim() {
            return bad("发音词典名称不能为空或包含首尾空白");
        }
        if dictionary.entries.iter().any(|(word, phones)| {
            word.trim().is_empty()
                || word != word.trim()
                || phones.split_whitespace().next().is_none()
        }) {
            return bad("发音词典的字词或音素无效");
        }
    }
    if p.time.tempo.first().is_none_or(|t| t.position != 0)
        || p.time.meter.first().is_none_or(|m| m.index != 0)
    {
        return bad("时间轴必须从 0 开始");
    }
    if !p.time.start_time_seconds.is_finite()
        || p.time
            .tempo
            .iter()
            .any(|t| !t.bpm.is_finite() || t.bpm <= 0.0 || t.position < 0)
        || p.time
            .tempo
            .windows(2)
            .any(|a| a[0].position >= a[1].position)
    {
        return bad("曲速必须为正数，标记按时间严格递增");
    }
    if p.time.meter.iter().any(|m| {
        m.index < 0
            || m.numerator == 0
            || m.numerator > 64
            || !m.denominator.is_power_of_two()
            || m.denominator > 64
    }) || p.time.meter.windows(2).any(|a| a[0].index >= a[1].index)
    {
        return bad("拍号或拍号位置无效");
    }
    let mut ids = HashSet::new();
    for g in p
        .library
        .iter()
        .chain(p.tracks.iter().map(|t| &t.main_group))
    {
        if g.uuid.is_empty() || !ids.insert(g.uuid.as_str()) {
            return bad("音符组 UUID 缺失或重复");
        }
        if g.notes.iter().any(|n| {
            n.duration <= 0
                || n.onset.checked_add(n.duration).is_none()
                || !(0..=127).contains(&n.pitch)
        }) {
            return bad("音符时间或 MIDI 音高无效");
        }
        for note in &g.notes {
            NotePerformance::from_note(note)
                .map_err(|error| format!("工程校验失败：音符 {}：{error}", note.lyrics))?;
            TakeSet::from_extra(&note.extra)
                .map_err(|error| format!("工程校验失败：音符 {}：{error}", note.lyrics))?;
        }
        for (name,curve) in g.parameters.iter().chain(&g.vocal_modes) {
            if name.trim().is_empty() {return bad("参数名称不能为空");}
            curve.validate().map_err(|e|format!("工程校验失败：参数 {name}：{e}"))?;
        }
        for control in &g.pitch_controls {
            match control {
                PitchControl::Point(point) if !point.valid() => {
                    return bad("音高控制点的音高或标识无效");
                }
                PitchControl::Curve(curve) if !curve.valid() => {
                    return bad("手绘音高曲线必须包含至少两个按时间递增的有效控制点");
                }
                _ => {}
            }
        }
    }
    for track in &p.tracks {
        if track.audio_clip().is_some_and(|c| !c.valid()) {
            return bad("伴奏文件路径或时间范围无效");
        }
        if track.is_instrumental()
            && (!track.groups.is_empty() || !track.main_group.notes.is_empty())
        {
            return bad("伴奏轨不能包含音符组");
        }
        if track.main_ref.group_id != track.main_group.uuid {
            return bad("主组引用不匹配");
        }
        for r in track.references() {
            TakeSet::from_extra(&r.extra)
                .map_err(|error| format!("工程校验失败：音轨 {}：{error}", track.name))?;
            let Some(group) = p.group(&r.group_id) else {
                return bad("存在悬空音符组引用");
            };

            if r.blick_absolute_end < -1
                || (r.blick_absolute_end != -1 && r.blick_absolute_end < r.blick_absolute_begin)
            {
                return bad("片段可见范围无效");
            }
            if group.notes.iter().any(|n| {
                n.onset.checked_add(r.blick_offset).is_none()
                    || (n.onset + n.duration).checked_add(r.blick_offset).is_none()
            }) {
                return bad("实例时间溢出");
            }
            if group.pitch_controls.iter().any(|control| match control {
                PitchControl::Point(point) => point.pos.checked_add(r.blick_offset).is_none(),
                PitchControl::Curve(curve) => curve
                    .absolute_points()
                    .any(|(time, _)| time.checked_add(r.blick_offset).is_none()),
                PitchControl::Unsupported(_) => false,
            }) {
                return bad("实例音高控制时间溢出");
            }
        }
    }
    Ok(())
}
