use super::{MAX_FRAMES, Provider, SAMPLE_RATE, background_command};
use crate::{model::RocoflexTrack, synthesis::audio::{ChannelLayout, RenderedAudio}};
use serde::Deserialize;
use std::{collections::BTreeMap, fs::{self, File}, io::{self, Read, Write}, ops::Range, path::{Path, PathBuf},
    process::Stdio, sync::{atomic::{AtomicBool, Ordering}, mpsc}, thread, time::{Duration, Instant}};

pub(super) struct Scratch(pub PathBuf);
impl Scratch {
    pub fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("rynthv-rocoflex-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[derive(Deserialize)]
struct Receipt {
    version: u32,
    input_frames: usize,
    output_frames: usize,
    sample_rate: u32,
    channels: usize,
    quality: String,
}

pub struct InputUpdate<K, T> {
    pub until: usize,
    pub owners: BTreeMap<K, T>,
}

pub fn render<K: Ord + Send, T: Send>(provider: &Provider, preset: &RocoflexTrack, range: Range<usize>,
    updates: mpsc::Receiver<InputUpdate<K, T>>, cancel: &AtomicBool,
    mix: impl Fn(&BTreeMap<K, T>, Range<usize>, &mut [f32]) -> Result<(), String> + Send,
    retain: impl Fn(&T, usize) -> bool + Send,
    mut publish: impl FnMut(RenderedAudio),
) -> Result<(), String> {
    let frames = range.len();
    if frames == 0 { return Ok(()); }
    if frames > MAX_FRAMES { return Err("Rocoflex track exceeds the offline render limit".into()); }
    let scratch = Scratch::new()?;
    let request = scratch.0.join("request.json");
    let document = scratch.0.join("preset.json");
    let output = scratch.0.join("output.f32");
    let progress = scratch.0.join("progress.f32");
    let reply = scratch.0.join("reply.json");
    let diagnostic = scratch.0.join("error.txt");
    fs::write(&document, serde_json::to_vec(preset.preset.as_ref()).map_err(|e| e.to_string())?).map_err(io_error)?;
    fs::write(&request, serde_json::to_vec(&serde_json::json!({
        "version": super::wire::VERSION, "preset": document, "input_frames": frames,
        "output": output, "progress": progress, "pregain_db": preset.pregain_db,
        "pitch_semitones": preset.pitch_semitones
    })).map_err(|e| e.to_string())?).map_err(io_error)?;
    let mut child = background_command(&provider.executable).arg("offline-render").arg(request)
        .stdin(Stdio::piped()).stdout(File::create(&reply).map_err(io_error)?)
        .stderr(File::create(&diagnostic).map_err(io_error)?).spawn().map_err(io_error)?;
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Rocoflex input is unavailable".into());
    };
    let stop = AtomicBool::new(false);
    thread::scope(|scope| {
        let stop_ref = &stop;
        let input_range = range.clone();
        let input = scope.spawn(move || feed(stdin, input_range, updates, cancel, stop_ref, mix, retain));
        let mut reader = Progress::default();
        let result = (|| {
            loop {
                if cancel.load(Ordering::Relaxed) { return Err("Rocoflex render cancelled".into()); }
                if let Some(status) = child.try_wait().map_err(io_error)? {
                    if !status.success() {
                        let mut error = String::new();
                        let _ = File::open(&diagnostic).map(|file| file.take(4096).read_to_string(&mut error));
                        return Err(format!("Rocoflex render failed ({status}): {}", error.trim()));
                    }
                    let receipt: Receipt = serde_json::from_slice(&read_bounded(&reply, 64 * 1024)?)
                        .map_err(|e| e.to_string())?;
                    if receipt.version != super::wire::VERSION || receipt.sample_rate != SAMPLE_RATE || receipt.channels != 1
                        || receipt.input_frames != frames || receipt.output_frames != frames || receipt.quality != "highest"
                        || output.metadata().map_err(io_error)?.len() != frames as u64 * 4
                    {
                        return Err("Rocoflex returned an unexpected offline render format".into());
                    }
                    reader.read(&output, &range, true, &mut publish)?;
                    if reader.frames != frames { return Err("Incomplete Rocoflex audio".into()); }
                    return Ok(());
                }
                reader.read(&progress, &range, false, &mut publish)?;
                thread::sleep(Duration::from_millis(30));
            }
        })();
        if result.is_err() {
            stop.store(true, Ordering::Relaxed);
            let _ = child.kill();
            let _ = child.wait();
        }
        let fed = input.join().map_err(|_| "Rocoflex PCM input worker ended unexpectedly".to_owned())?;
        result.and(fed)
    })
}

fn feed<K: Ord, T>(mut out: impl Write, range: Range<usize>, updates: mpsc::Receiver<InputUpdate<K, T>>, cancel: &AtomicBool,
    stop: &AtomicBool, mix: impl Fn(&BTreeMap<K, T>, Range<usize>, &mut [f32]) -> Result<(), String>,
    retain: impl Fn(&T, usize) -> bool) -> Result<(), String>
{
    let mut at = range.start;
    let mut owners = BTreeMap::new();
    let mut samples = [0f32; 4096];
    let mut bytes = [0u8; 4096 * 4];
    while at < range.end {
        if cancel.load(Ordering::Relaxed) || stop.load(Ordering::Relaxed) { return Err("Rocoflex render cancelled".into()); }
        let update = match updates.recv_timeout(Duration::from_millis(50)) {
            Ok(update) => update,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err("Rocoflex source stopped before the end of the track".into()),
        };
        if update.until < at || update.until > range.end { return Err("Invalid Rocoflex source coverage".into()); }
        owners.extend(update.owners);
        while at < update.until {
            if cancel.load(Ordering::Relaxed) || stop.load(Ordering::Relaxed) { return Err("Rocoflex render cancelled".into()); }
            let end = (at + samples.len()).min(update.until);
            let count = end - at;
            let block = &mut samples[..count];
            block.fill(0.0);
            mix(&owners, at..end, block)?;
            for (word, sample) in bytes.chunks_exact_mut(4).zip(block) {
                if !sample.is_finite() { return Err("Non-finite Rocoflex input".into()); }
                word.copy_from_slice(&sample.to_le_bytes());
            }
            out.write_all(&bytes[..count * 4]).map_err(io_error)?;
            at = end;
            owners.retain(|_, owner| retain(owner, at));
        }
    }
    Ok(())
}

#[derive(Default)]
struct Progress {
    file: Option<File>,
    frames: usize,
    last_publication: Option<Instant>,
}

impl Progress {
    fn read(&mut self, path: &Path, range: &Range<usize>, finished: bool,
        publish: &mut impl FnMut(RenderedAudio)) -> Result<(), String>
    {
        if self.file.is_none() {
            self.file = match File::open(path) {
                Ok(file) => Some(file),
                Err(e) if !finished && e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e.to_string()),
            };
        }
        let file = self.file.as_mut().unwrap();
        let written = file.metadata().map_err(io_error)?.len() / 4;
        let available = usize::try_from(written).map_err(|e| e.to_string())?;
        if available > range.len() || available < self.frames { return Err("Invalid Rocoflex progress length".into()); }
        let unread = available - self.frames;
        if unread == 0 || !finished && self.last_publication.is_some_and(|at|
            unread < SAMPLE_RATE as usize / 2 && at.elapsed() < Duration::from_millis(200))
        {
            return Ok(());
        }
        while self.frames < available {
            let count = (available - self.frames).min(SAMPLE_RATE as usize);
            let mut bytes = vec![0; count * 4];
            file.read_exact(&mut bytes).map_err(io_error)?;
            let samples = bytes.chunks_exact(4).map(|v| f32::from_le_bytes(v.try_into().unwrap())).collect();
            let start = (range.start + self.frames) as f64 / f64::from(SAMPLE_RATE);
            let audio = RenderedAudio::new(SAMPLE_RATE, start, ChannelLayout::Mono, samples)
                .map_err(|e| e.to_string())?;
            self.frames += count;
            publish(audio);
        }
        self.last_publication = Some(Instant::now());
        Ok(())
    }
}

fn io_error(error: io::Error) -> String { error.to_string() }

pub(super) fn read_bounded(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path).and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit { return Err("Rocoflex response is too large".into()); }
    Ok(bytes)
}
