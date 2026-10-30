use super::{RenderedAudio, WaveFormat, save_wav_as};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioFileFormat {
    #[default]
    Wav,
    Flac,
    Mp3,
    Vorbis,
    Opus,
    Aac,
    Aiff,
}

impl AudioFileFormat {
    pub const ALL: [Self; 7] = [Self::Wav, Self::Flac, Self::Mp3, Self::Vorbis, Self::Opus, Self::Aac, Self::Aiff];

    pub fn id(self) -> &'static str {
        match self {
            Self::Wav => "wav", Self::Flac => "flac", Self::Mp3 => "mp3",
            Self::Vorbis => "ogg", Self::Opus => "opus", Self::Aac => "m4a", Self::Aiff => "aiff",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Wav => "WAV", Self::Flac => "FLAC", Self::Mp3 => "MP3",
            Self::Vorbis => "Ogg Vorbis", Self::Opus => "Opus", Self::Aac => "M4A (AAC)", Self::Aiff => "AIFF",
        }
    }

    pub fn from_id(id: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|format| format.id() == id)
            .ok_or_else(|| "请选择有效的文件格式".into())
    }

    pub fn converter(self, configured: Option<&Path>) -> Result<Option<PathBuf>, String> {
        if self == Self::Wav { return Ok(None); }
        if let Some(path) = configured {
            return path.is_file().then(|| path.to_owned()).map(Some)
                .ok_or_else(|| "音频转换器路径无效，请在设置中选择 FFmpeg。".into());
        }
        let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        let bundled = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join(name)));
        let path = bundled.into_iter().chain(
            std::env::var_os("PATH").into_iter().flat_map(|paths|
                std::env::split_paths(&paths).map(|p| p.join(name)).collect::<Vec<_>>())
        ).find(|p| p.is_file());
        path.map(Some).ok_or_else(|| "未找到 FFmpeg，请在设置中选择音频转换器。".into())
    }

    pub fn save(self, path: &Path, audio: &RenderedAudio, wave: WaveFormat, converter: Option<&Path>) -> Result<(), String> {
        if self == Self::Wav { return save_wav_as(path, audio, wave).map_err(|e| e.to_string()); }
        let converter = converter.ok_or("未找到 FFmpeg，请在设置中选择音频转换器。")?;
        let parent = path.parent().ok_or("无效的导出路径")?;
        let stage = parent.join(format!(".rynthv-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&stage).map_err(|e| e.to_string())?;
        let temporary = ConversionFiles { wave: stage.join("input.wav"), output: stage.join(format!("output.{}", self.id())), directory: stage };
        save_wav_as(&temporary.wave, audio, wave).map_err(|e| e.to_string())?;
        let mut command = Command::new(converter);
        command.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&temporary.wave).args(["-map", "0:a:0", "-vn", "-map_metadata", "-1"]);
        match self {
            Self::Flac => {
                command.args(["-c:a", "flac", "-sample_fmt", if wave.bit_depth == 16 { "s16" } else { "s32" }]);
            }
            Self::Mp3 => { command.args(["-c:a", "libmp3lame", "-q:a", "2"]); }
            Self::Vorbis => { command.args(["-c:a", "libvorbis", "-q:a", "6"]); }
            Self::Opus => { command.args(["-c:a", "libopus", "-b:a", "192k", "-ar", "48000"]); }
            Self::Aac => { command.args(["-c:a", "aac", "-b:a", "256k", "-movflags", "+faststart"]); }
            Self::Aiff => {
                command.args(["-c:a", match wave.bit_depth { 16 => "pcm_s16be", 24 => "pcm_s24be", _ => "pcm_f32be" }]);
            }
            Self::Wav => unreachable!(),
        }
        command.arg(&temporary.output).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let result = command.output().map_err(|e| format!("无法启动音频转换器：{e}"))?;
        if !result.status.success() {
            return Err(format!("音频转换失败：{}", String::from_utf8_lossy(&result.stderr).trim()));
        }
        if fs::metadata(&temporary.output).map_err(|e| e.to_string())?.len() == 0 {
            return Err("音频转换器未生成有效文件".into());
        }
        fs::rename(&temporary.output, path).map_err(|e| e.to_string())
    }
}

struct ConversionFiles { wave: PathBuf, output: PathBuf, directory: PathBuf }
impl Drop for ConversionFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.wave);
        let _ = fs::remove_file(&self.output);
        let _ = fs::remove_dir(&self.directory);
    }
}
