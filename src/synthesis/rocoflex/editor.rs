use super::{Provider, background_command, wire};
use eframe::egui::Context;
use std::{io::{BufReader, BufWriter, Write}, process::{Child, Stdio},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc}, thread};

pub struct Editor {
    requests: mpsc::SyncSender<wire::Input>,
    pub replies: mpsc::Receiver<Result<(wire::Frame, Vec<u8>), String>>,
    child: Arc<Mutex<Process>>,
    visible: Arc<AtomicBool>,
}

#[derive(Default)]
struct Process { child: Option<Child>, closed: bool }

impl Editor {
    pub fn open(provider: &Provider, state: Option<wire::State>, ctx: &Context) -> Result<Self, String> {
        let (requests, inputs) = mpsc::sync_channel(1);
        let (output, replies) = mpsc::channel();
        let child = Arc::new(Mutex::new(Process::default()));
        let owner = child.clone();
        let executable = provider.executable.clone();
        let viewport = ctx.viewport_id();
        let ctx = ctx.clone();
        let visible = Arc::new(AtomicBool::new(true));
        let notify = Arc::clone(&visible);
        thread::Builder::new().name("rocoflex-canvas".into()).spawn(move || {
            let active = || -> Result<(), String> {
                let mut child = background_command(&executable).arg("integration-editor").stdin(Stdio::piped())
                    .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(io)?;
                let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Rocoflex editor pipes are unavailable".into());
                };
                let mut stdin = BufWriter::new(stdin);
                let mut stdout = BufReader::new(stdout);
                {
                    let mut process = owner.lock().unwrap();
                    if process.closed { let _ = child.kill(); let _ = child.wait(); return Ok(()); }
                    process.child = Some(child);
                }
                send(&mut stdin, &wire::Open { version: wire::VERSION, state }, wire::DOCUMENT_LIMIT + 4096)?;
                for input in inputs {
                    send(&mut stdin, &input, wire::INPUT_LIMIT)?;
                    let header = wire::read(&mut stdout, wire::FRAME_LIMIT).map_err(io)?
                        .ok_or("Rocoflex closed the editor")?;
                    let frame: wire::Frame = serde_json::from_slice(&header).map_err(|e| e.to_string())?;
                    let payload = wire::read(&mut stdout, wire::FRAME_LIMIT - header.len()).map_err(io)?
                        .ok_or("Rocoflex returned an incomplete frame")?;
                    if let Some(error) = frame.error { return Err(error); }
                    if output.send(Ok((frame, payload))).is_err() { break; }
                    if notify.load(Ordering::Acquire) { ctx.request_repaint_of(viewport); }
                }
                Ok(())
            };
            let result = active();
            if let Some(mut child) = owner.lock().unwrap().child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Err(error) = result {
                let _ = output.send(Err(error));
                if notify.load(Ordering::Acquire) { ctx.request_repaint_of(viewport); }
            }
        }).map_err(io)?;
        Ok(Self { requests, replies, child, visible })
    }

    pub fn request(&self, input: wire::Input) -> Result<(), String> {
        self.requests.try_send(input).map_err(|_| "Rocoflex editor is unavailable".into())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.visible.store(false, Ordering::Release);
        let mut process = self.child.lock().unwrap();
        process.closed = true;
        if let Some(child) = &mut process.child { let _ = child.kill(); }
    }
}

fn send(writer: &mut impl Write, request: &impl serde::Serialize, limit: usize) -> Result<(), String> {
    let bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    wire::write(writer, &bytes, limit).and_then(|()| writer.flush()).map_err(io)
}

fn io(error: std::io::Error) -> String { error.to_string() }
