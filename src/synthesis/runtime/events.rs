use super::worker::Event;
use eframe::egui;
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{SendError, Sender}}, time::Duration};

#[derive(Clone)]
pub(super) struct RenderWake {
    repaint: egui::Context,
    pending: Arc<AtomicBool>,
}

impl RenderWake {
    pub fn new(repaint: egui::Context) -> Self {
        Self { repaint, pending: Arc::new(AtomicBool::new(false)) }
    }

    pub fn begin_poll(&self) { self.pending.store(false, Ordering::Release); }

    pub fn request(&self) {
        if !self.pending.swap(true, Ordering::AcqRel) {
            self.repaint.request_repaint_after_for(Duration::from_millis(33), egui::ViewportId::ROOT);
        }
    }
}

pub(super) struct Events {
    sender: Sender<Event>,
    wake: RenderWake,
}

impl Events {
    pub fn new(sender: Sender<Event>, wake: RenderWake) -> Self {
        Self { sender, wake }
    }

    pub fn send(&self, event: Event) -> Result<(), SendError<Event>> {
        let urgent = matches!(&event, Event::Voice { .. } | Event::Ready | Event::ContextRequired { .. } | Event::Error { .. });
        self.sender.send(event)?;
        if urgent { self.wake.repaint.request_repaint_of(egui::ViewportId::ROOT); }
        else { self.wake.request(); }
        Ok(())
    }
}

pub(super) struct WakeOnExit(pub egui::Context);

impl Drop for WakeOnExit {
    fn drop(&mut self) {
        self.0.request_repaint();
    }
}
