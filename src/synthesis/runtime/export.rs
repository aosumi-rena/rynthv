use super::{Runtime, events::WakeOnExit, mixer};
use crate::{
    model::Project,
    synthesis::audio::{AudioFileFormat, ChannelLayout, WaveFormat},
};
use std::{path::{Path, PathBuf}, sync::mpsc, thread};

pub struct ExportRequest {
    pub outputs: Vec<(PathBuf, Option<u64>)>,
    format: WaveFormat,
    file_format: AudioFileFormat,
    converter: Option<PathBuf>,
}

impl ExportRequest {
    pub fn from_project(project: &Project, project_path: Option<&Path>, converter: Option<&Path>) -> Result<Self, String> {
        let config = &project.render_config;
        crate::synthesis::mixing_options::validate_export_bypass(config)?;
        crate::synthesis::mixing_options::validate_project_mixer(
            project.extra.get("projectMixer"),
        )?;
        for track in &project.tracks {
            crate::synthesis::mixing_options::validate_mixer(&track.mixer)
                .map_err(|error| format!("音轨 {}：{error}", track.name))?;
        }
        let text = |key: &str| {
            config
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        };
        let number = |key: &str| {
            config
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        };
        let directory = export_directory(text("destination"), project_path)?;
        if !directory.is_dir() {
            return Err("请选择有效的目标文件夹".into());
        }
        let name = text("filename").trim();
        if name.is_empty()
            || name.ends_with(['.', ' '])
            || name
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        {
            return Err("文件名称为空或含有无效字符".into());
        }
        let file_format = AudioFileFormat::from_id(config.get("audioFormat").and_then(serde_json::Value::as_str).unwrap_or("wav"))?;
        let stem = name.rsplit_once('.').filter(|(_, extension)| {
            AudioFileFormat::ALL.iter().any(|format| extension.eq_ignore_ascii_case(format.id())) || extension.eq_ignore_ascii_case("aif")
        }).map_or(name, |(stem, _)| stem);
        if stem.is_empty() || reserved_name(stem) {
            return Err("请使用有效的文件名称".into());
        }
        let sample_rate = number("sampleRate");
        let bit_depth = number("bitDepth");
        if ![44100, 48000, 96000].contains(&sample_rate) || ![16, 24, 32].contains(&bit_depth) {
            return Err("请选择有效的采样率和位深度".into());
        }
        if file_format == AudioFileFormat::Flac && bit_depth == 32 {
            return Err("FLAC 请选择 16 位或 24 位".into());
        }
        if (file_format == AudioFileFormat::Opus && sample_rate != 48000)
            || (file_format == AudioFileFormat::Mp3 && sample_rate > 48000) {
            return Err("请选择此文件格式支持的采样率".into());
        }
        let layout = match number("numChannels") {
            1 => ChannelLayout::Mono,
            2 => ChannelLayout::Stereo,
            _ => return Err("请选择有效的声道数".into()),
        };
        if text("aspirationFormat") != "noAspiration" {
            return Err("暂不支持单独导出气声".into());
        }
        let mut outputs = Vec::new();
        if config
            .get("exportMixDown")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            outputs.push((directory.join(format!("{stem}.{}", file_format.id())), None));
        }
        for (index, track) in project.tracks.iter().enumerate() {
            if track
                .extra
                .get("renderEnabled")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                continue;
            }
            let safe: String = track
                .name
                .chars()
                .map(|c| {
                    if c.is_control() || "<>:\"/\\|?*".contains(c) {
                        '_'
                    } else {
                        c
                    }
                })
                .collect();
            outputs.push((
                directory.join(format!(
                    "{stem}-{:02}-{}.{}",
                    index + 1,
                    safe.trim_end_matches(['.', ' ']),
                    file_format.id(),
                )),
                Some(track.id),
            ));
        }
        if outputs.is_empty() {
            return Err("请勾选混音导出或至少一条音轨".into());
        }
        Ok(Self {
            outputs,
            file_format,
            converter: file_format.converter(converter)?,
            format: WaveFormat {
                sample_rate: sample_rate as u32,
                layout,
                bit_depth: bit_depth as u16,
            },
        })
    }
}

pub fn export_directory(destination: &str, project_path: Option<&Path>) -> Result<PathBuf, String> {
    let destination = Path::new(if destination.is_empty() { "." } else { destination });
    if destination.is_absolute() { return Ok(destination.to_owned()); }
    let project = project_path.ok_or("请先保存工程，或选择绝对路径作为导出文件夹")?;
    let project = std::path::absolute(project).map_err(|e| e.to_string())?;
    let parent = project.parent().ok_or("无法确定工程文件夹")?;
    Ok(parent.join(destination))
}

fn reserved_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (1..=9).any(|n| base == format!("COM{n}") || base == format!("LPT{n}"))
}

impl Runtime {
    pub fn export_ready(&self, revision: u64) -> bool {
        self.last_revision == Some(revision)
            && self.export.is_none()
            && self.recording_error.is_none()
            && self.recordings.pending.is_empty()
            && self.pending_voices.is_empty()
            && (self
                .planner
                .plan()
                .requests
                .keys()
                .all(|id| !self.routes.contains_key(&id.reference()) && !self.binding_errors.contains_key(&id.reference()))
                || (!self.is_busy() && self.stats.failed == 0 && self.error.is_none()))
            && (!self.export_owners.is_empty() || !self.recordings.layers.is_empty())
    }

    pub fn export_project(&mut self, request: ExportRequest, revision: u64) {
        if !self.export_ready(revision) {
            self.export_result = Some(Err("请等待当前工程合成完成后再导出".into()));
            return;
        }
        let owners = self.export_owners.clone();
        let recordings = self.recordings.layers.clone();
        let solo = self.solo_tracks.clone();
        let repaint = self.repaint.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.export_result = None;
        match thread::Builder::new()
            .name("audio-export".into())
            .spawn(move || {
                let _wake = WakeOnExit(repaint);
                let sender = tx;
                let result = (|| {

                    let mut sources = Vec::new();
                    for (path, track) in request.outputs {
                        let audio = if let Some(track) = track {
                            let stems: Vec<_> = owners
                                .iter()
                                .filter(|owner| owner.controls.track_id == track)
                                .cloned()
                                .collect();
                            let backing: Vec<_> = recordings
                                .iter()
                                .filter(|layer| layer.track_id == track)
                                .cloned()
                                .collect();
                            mixer::mix_with_recordings(&stems, &solo, &backing)?
                        } else {
                            mixer::mix_with_recordings(&owners, &solo, &recordings)?
                        };
                        let audio = audio.ok_or_else(|| {
                            format!(
                                "{} 没有可导出的音频，请检查音轨是否为空、静音或被独奏排除",
                                path.display()
                            )
                        })?;
                        sources.push((path, audio));
                    }
                    let mut written = Vec::new();
                    for (path, audio) in sources {
                        request.file_format.save(&path, &audio, request.format, request.converter.as_deref())
                            .map_err(|e| format!("{e}（此前已完成 {} 个文件）", written.len()))?;
                        written.push(path);
                    }
                    Ok(written)
                })();
                let _ = sender.send(result);
            }) {
            Ok(_) => self.export = Some(rx),
            Err(error) => self.export_result = Some(Err(format!("无法启动音频导出：{error}"))),
        }
    }
}
