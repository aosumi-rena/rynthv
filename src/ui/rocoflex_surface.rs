use crate::{i18n::{message, t}, model::RocoflexTrack, synthesis::rocoflex::{Provider, editor::Editor, wire}};
use super::{rocoflex_input::Input, rocoflex_presentation::{Picture, rectangle},
    rocoflex_viewport::{CanvasViewport, Viewport}};
use eframe::egui_wgpu::RenderState;
use eframe::egui::*;
use std::time::{Duration, Instant};

const EDITOR_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, PartialEq)]
struct Request {
    viewport: CanvasViewport,
    focused: bool,
}

#[derive(Default)]
pub(super) struct Surface {
    editor: Option<Editor>,
    base: Option<RocoflexTrack>,
    baseline: Option<wire::State>,
    changed: bool,
    unsettled: bool,
    provider_busy: bool,
    edit_pending: bool,
    picture: Picture,
    input: Input,
    pending: Option<Instant>,
    shown: bool,
    due: Option<Instant>,
    sent: Option<Request>,
    shown_viewport: Option<CanvasViewport>,
    last_request: Option<Instant>,
    viewport: Viewport,
    error: Option<String>,
    pub rect: Option<(Rect, LayerId, ViewportId)>,
    pub state: Option<wire::State>,
}

pub(super) struct Change {
    pub state: wire::State,
    pub first: bool,
}

impl Surface {
    pub fn disconnect(&mut self) { *self = Self::default(); }

    fn hide(&mut self) {
        if self.editor.is_none() && self.rect.is_none() { return; }
        self.editor = None;
        self.picture = Picture::default();
        self.input = Input::default();
        self.rect = None;
        self.pending = None;
        self.sent = None;
        self.due = None;
        self.last_request = None;
        self.shown_viewport = None;
        self.viewport = Viewport::default();
        self.edit_pending = false;
        self.unsettled = false;
        self.provider_busy = false;
    }

    pub fn sync(&mut self, current: Option<&RocoflexTrack>) {
        if self.base.as_ref() != current {
            let preset_unchanged = self.base.as_ref().zip(current).is_some_and(|(old, new)|
                old.preset == new.preset && old.pregain_db == new.pregain_db && old.pitch_semitones == new.pitch_semitones);
            if !preset_unchanged { self.disconnect(); }
            self.base = current.cloned();
        }
    }

    pub fn applied(&self) -> bool {
        self.error.is_none() && (self.state.is_some() || self.base.is_some()) && !self.changed && !self.unsettled
            && !self.edit_pending && !self.input.has_edits()
    }

    pub fn confirmable(&self) -> bool {
        self.picture.ready() && self.error.is_none() && !self.unsettled && !self.edit_pending && !self.input.has_edits()
            && self.state.as_ref().is_some_and(|state| state.has_target)
    }

    fn busy(&self) -> bool {
        self.editor.is_some() && (self.unsettled || self.provider_busy || self.edit_pending || self.input.has_edits())
    }

    pub fn accept(&mut self, applied: RocoflexTrack) {
        self.base = Some(applied);
        self.baseline = self.state.clone();
        self.changed = false;
    }

    pub fn show(&mut self, ui: &mut Ui, size: Vec2, provider: &Provider, gpu: &RenderState) -> Option<Change> {
        let (room, _) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_sizing_pass() || !ui.is_rect_visible(room) { return None; }
        self.shown = true;
        if self.editor.is_none() && self.error.is_none() {
            let state = self.state.clone().or_else(|| self.base.as_ref().map(|b| wire::State {
                document: b.preset.clone(), pregain_db: b.pregain_db, pitch_semitones: b.pitch_semitones, has_target: false,
            }));
            match Editor::open(provider, state, ui.ctx()) {
                Ok(editor) => self.editor = Some(editor),
                Err(error) => self.error = Some(error),
            }
        }
        let change = self.receive(ui.ctx(), gpu);
        let (desired, settle) = self.viewport.update(size, ui.ctx().pixels_per_point());
        if let Some(delay) = settle { ui.ctx().request_repaint_after(delay); }
        let displayed = self.shown_viewport.unwrap_or(desired);
        let ratio = displayed.zoom(size);
        let rect = Rect::from_center_size(room.center(), Vec2::from(displayed.size) * ratio);
        let pointer_busy = ui.input(|input| input.pointer.any_down() || input.pointer.any_released());
        let target = if self.pending.is_some() || pointer_busy || !self.input.is_pointer_idle() { displayed } else { desired };
        let resizing = target != displayed;
        let response = ui.interact(rect, ui.id().with("rocoflex-canvas"), Sense::click_and_drag());
        self.rect = Some((rect, ui.layer_id(), ui.ctx().viewport_id()));
        if self.editor.is_some() {
            if !resizing { self.input.collect(ui, &response, rect, ratio); }
            self.send(ui.ctx(), target, false);
        }
        if self.picture.draw(ui, rect) {
            if self.input.hovered && let Some(cursor) = CursorIcon::ALL.get(self.picture.cursor).copied() { ui.ctx().set_cursor_icon(cursor); }
            if self.input.focused && let Some((edit, caret, interrupt)) = self.picture.ime {
                let transform = emath::TSTransform::new(rect.min.to_vec2(), ratio);
                ui.ctx().output_mut(|out| out.ime = Some(output::IMEOutput { purpose: IMEPurpose::Normal, rect: transform * rectangle(edit),
                    cursor_rect: transform * rectangle(caret), should_interrupt_composition: interrupt }));
            }
        } else {
            let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::centered_and_justified(Direction::TopDown)));
            if self.error.is_none() {
                child.horizontal(|ui| { ui.spinner(); ui.label(t("ROCOFLEX_OPENING")); });
            }
        }
        if let Some(error) = self.error.clone() {
            let mut error_ui = ui.new_child(UiBuilder::new().max_rect(rect.shrink(24.0)).layout(Layout::top_down(Align::Center)));
            error_ui.label(message(&error));
            if error_ui.button(t("ROCOFLEX_RETRY")).clicked() {
                let base = self.base.clone();
                let baseline = self.baseline.clone();
                let state = self.state.clone();
                self.disconnect();
                self.base = base;
                self.baseline = baseline;
                self.state = state;
                ui.ctx().request_repaint();
            }
        }
        change
    }

    pub fn concealed(&mut self) {
        if !self.shown { self.hide(); }
        self.shown = false;
    }

    pub fn pause(&mut self, ctx: &Context, gpu: &RenderState) {
        self.shown = false;
        if self.editor.is_none() { return; }
        self.rect = None;
        self.input.blur();
        self.receive(ctx, gpu);
        if self.busy() || self.pending.is_some() {
            if let Some(request) = self.sent { self.send(ctx, request.viewport, true); }
        } else {
            self.hide();
        }
    }

    pub fn owns_keyboard(&self, ctx: &Context) -> bool {
        self.input.focused && self.at_pointer(ctx, true)
    }

    pub fn text_edit_focused(&self, ctx: &Context) -> bool {
        self.picture.ime.is_some() && self.owns_keyboard(ctx)
    }

    pub fn at_pointer(&self, ctx: &Context, keyboard: bool) -> bool {
        self.rect.is_some_and(|(rect, layer, viewport)| {
            if viewport != ctx.viewport_id() { return false; }
            let (inside, pressed, focused) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.any_pressed(), i.focused));
            focused && ((keyboard && !pressed) || inside.is_some_and(|p| rect.contains(p) && ctx.layer_id_at(p) == Some(layer)))
        })
    }

    pub fn receive(&mut self, ctx: &Context, gpu: &RenderState) -> Option<Change> {
        let Some(editor) = &self.editor else { return None; };
        let result = match editor.replies.try_recv() {
            Ok(v) => v,
            Err(std::sync::mpsc::TryRecvError::Empty) if !self.pending.is_some_and(|sent| sent.elapsed() >= EDITOR_TIMEOUT) => return None,
            Err(_) => Err(t("ROCOFLEX_NO_RESPONSE").into()),
        };
        self.pending = None;
        self.edit_pending = false;
        let viewport = self.sent?.viewport;
        match result.and_then(|(frame, data)| {
            if frame.size != viewport.size { return Err("Rocoflex returned an invalid canvas size".into()); }
            self.picture.update(gpu, &frame, &data, viewport.scale)?;
            Ok(frame)
        }) {
            Ok(frame) => {
                if self.shown_viewport != Some(viewport) { self.input.reset_pointer(); }
                self.shown_viewport = Some(viewport);
                self.due = frame.wait_ms.map(|wait| Instant::now() + Duration::from_millis(u64::from(wait)));
                self.unsettled = frame.unsettled;
                self.provider_busy = frame.busy;
                if self.input.focused && let Some(text) = frame.clipboard { ctx.copy_text(text); }
                frame.state.map(|state| {
                    let first = self.state.is_none();
                    if self.baseline.is_none() { self.baseline = Some(state.clone()); }
                    self.changed = self.baseline.as_ref() != Some(&state);
                    self.state = Some(state.clone());
                    Change { state, first }
                })
            }
            Err(error) => { self.error = Some(error); self.editor = None; None }
        }
    }

    fn send(&mut self, ctx: &Context, viewport: CanvasViewport, background: bool) {
        if let Some(since) = self.pending {
            ctx.request_repaint_after(EDITOR_TIMEOUT.saturating_sub(since.elapsed()));
            return;
        }
        let Some(editor) = &self.editor else { return; };
        let identity = Request { viewport, focused: self.input.focused };
        if self.sent == Some(identity) && !self.input.has_events() {
            let Some(due) = self.due else { return; };
            if let Some(delay) = due.checked_duration_since(Instant::now()) {
                ctx.request_repaint_after(delay);
                return;
            }
        }
        if !background && self.picture.ready() && let Some(last) = self.last_request {
            let delay = REQUEST_INTERVAL.saturating_sub(last.elapsed());
            if !delay.is_zero() {
                ctx.request_repaint_after(delay);
                return;
            }
        }
        self.edit_pending = self.input.has_edits();
        let request = wire::Input { size: viewport.size, scale: viewport.scale, focused: self.input.focused,
            events: self.input.take_events(), files: std::mem::take(&mut self.input.files) };
        match editor.request(request) {
            Ok(()) => {
                self.sent = Some(identity);
                let now = Instant::now();
                self.last_request = Some(now);
                self.pending = Some(now);
                ctx.request_repaint_after(EDITOR_TIMEOUT);
            }
            Err(error) => { self.error = Some(error); self.editor = None; }
        }
    }
}
