use super::Provider;
use eframe::egui::Context;
use std::{sync::mpsc, thread};

#[derive(Default)]
pub(crate) enum RenderCheck {
    #[default]
    Idle,
    Pending(mpsc::Receiver<Result<(), String>>),
    Done(Result<(), String>),
}

impl RenderCheck {
    pub fn request(&mut self, provider: &Provider, repaint: &Context, fresh: bool) {
        if matches!(self, Self::Pending(_)) || !fresh && !matches!(self, Self::Idle) { return; }
        let (send, receive) = mpsc::sync_channel(1);
        let provider = provider.clone();
        let repaint = repaint.clone();
        *self = match thread::Builder::new().name("rocoflex-model-check".into()).spawn(move || {
            if send.send(provider.check_render_models()).is_ok() { repaint.request_repaint(); }
        }) {
            Ok(_) => Self::Pending(receive),
            Err(error) => Self::Done(Err(error.to_string())),
        };
    }

    pub fn poll(&mut self) -> bool {
        let Self::Pending(receive) = self else { return false; };
        *self = Self::Done(match receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Err("Rocoflex could not check its rendering models.".into()),
        });
        true
    }

    pub fn ready(&self) -> bool { matches!(self, Self::Done(Ok(()))) }

    pub fn error(&self) -> Option<&str> {
        if let Self::Done(Err(error)) = self { Some(error) } else { None }
    }
}
