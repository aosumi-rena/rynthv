use serde::{Deserialize, Serialize};
use std::{
    fs::File, path::{Path, PathBuf}, process::{Command, ExitStatus, Stdio},
    thread, time::{Duration, Instant, SystemTime},
};
mod render;
mod model_check;
pub mod editor;
pub mod wire;
pub use render::{InputUpdate, render};
pub(crate) use model_check::RenderCheck;

pub const SAMPLE_RATE: u32 = 48_000;
pub const MAX_FRAMES: usize = SAMPLE_RATE as usize * 60 * 30;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub executable: Option<PathBuf>,
    pub disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Provider {
    pub executable: PathBuf,
    revision: (u64, SystemTime),
}

#[derive(Deserialize)]
struct Capabilities {
    version: u32,
    offline_render: bool,
    #[serde(default)]
    streaming: bool,
    editor: bool,
    #[serde(default)]
    render_check: bool,
}

pub fn background_command(executable: &Path) -> Command {
    let mut command = command(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_4000);
    }
    command
}

pub fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

impl Settings {
    pub fn detect(&self) -> Result<Provider, String> {
        if self.disabled { return Err("Rocoflex is unloaded".into()); }
        let path = self.executable.clone().or_else(|| {
            let executable = std::env::current_exe().ok()?;
            Some(executable.parent()?.join(format!("rocoflex{}", std::env::consts::EXE_SUFFIX)))
        }).ok_or("Rocoflex was not found")?;
        let executable = path.canonicalize().map_err(|_| "Rocoflex was not found")?;
        if !executable.is_file() {
            return Err("Rocoflex was not found".into());
        }
        let scratch = render::Scratch::new()?;
        let reply = scratch.0.join("reply.json");
        let diagnostic = scratch.0.join("error.txt");
        let status = bounded_command(command(&executable).arg("integration-info"), &reply, &diagnostic, Duration::from_secs(5))?;
        let info: Capabilities = serde_json::from_slice(&render::read_bounded(&reply, 4096)?)
            .map_err(|_| "This Rocoflex version does not support integration")?;
        if !status.success() || info.version != wire::VERSION || !info.offline_render || !info.editor
            || !info.streaming || !info.render_check
        {
            return Err("Update Rocoflex to use it with RynthV.".into());
        }
        let metadata = executable.metadata().map_err(|e| e.to_string())?;
        let revision = (metadata.len(), metadata.modified().map_err(|e| e.to_string())?);
        Ok(Provider { executable, revision })
    }
}

impl Provider {
    pub(crate) fn check_render_models(&self) -> Result<(), String> {
        let scratch = render::Scratch::new()?;
        let reply = scratch.0.join("reply.json");
        let diagnostic = scratch.0.join("error.txt");
        let status = bounded_command(background_command(&self.executable).arg("integration-check-render"),
            &reply, &diagnostic, Duration::from_secs(3))?;
        if status.success() { return Ok(()); }
        let error = render::read_bounded(&diagnostic, 4096).unwrap_or_default();
        let error = String::from_utf8_lossy(&error);
        let error = error.trim();
        Err(if error.is_empty() { "Rocoflex could not check its rendering models." } else { error }.to_owned())
    }
}

fn bounded_command(command: &mut Command, reply: &Path, diagnostic: &Path, budget: Duration) -> Result<ExitStatus, String> {
    let io = |error: std::io::Error| error.to_string();
    let mut child = command.stdin(Stdio::null()).stdout(File::create(reply).map_err(io)?)
        .stderr(File::create(diagnostic).map_err(io)?).spawn().map_err(io)?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if start.elapsed() < budget => thread::sleep(Duration::from_millis(25)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().map_or_else(|| "Rocoflex did not respond".into(), io));
            }
        }
    }
}

