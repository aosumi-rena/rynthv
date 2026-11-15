use eframe::egui::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Arrangement,
    Piano,
    Parameter(u64),
    Phonemes,
    InstrumentDynamics,
}
impl Target {
    fn id(self) -> Id {
        Id::new(("view-pan", self))
    }
}

struct Capture {
    target: Target,
    pointer: Pos2,
}

#[derive(Default)]
pub struct ViewPan {
    capture: Option<Capture>,
}
impl ViewPan {
    pub fn is_active(&self) -> bool {
        self.capture.is_some()
    }

    pub fn drag(&mut self, ui: &Ui, target: Target, rect: Rect, enabled: bool) -> Vec2 {
        let (pressed, held, other_button, focused, pointer) = ui.input(|i| {
            (
                i.pointer.button_pressed(PointerButton::Middle),
                i.pointer.button_down(PointerButton::Middle),
                i.pointer.button_down(PointerButton::Primary)
                    || i.pointer.button_down(PointerButton::Secondary),
                i.focused,
                i.pointer.interact_pos(),
            )
        });
        if !focused {
            return Vec2::ZERO;
        }
        let Some(pointer) = pointer else {
            return Vec2::ZERO;
        };
        if self.capture.is_none()
            && pressed
            && held
            && !other_button
            && enabled
            && ui.is_enabled()
            && ui.rect_contains_pointer(rect)
        {
            self.capture = Some(Capture { target, pointer });
        }
        let Some(capture) = &mut self.capture else {
            return Vec2::ZERO;
        };
        if capture.target != target {
            return Vec2::ZERO;
        }
        let delta = pointer - capture.pointer;
        capture.pointer = pointer;
        if held {

            ui.ctx().set_dragged_id(target.id());
        }
        delta
    }

    pub fn cancel(&mut self, ctx: &Context) {
        if let Some(capture) = self.capture.take()
            && ctx.dragged_id() == Some(capture.target.id())
        {
            ctx.stop_dragging();
        }
    }

    pub fn end_frame(&mut self, ctx: &Context) {
        if !self.is_active() {
            return;
        }
        if ctx.input(|i| !i.focused || !i.pointer.button_down(PointerButton::Middle)) {
            self.cancel(ctx);
        } else {
            ctx.set_cursor_icon(CursorIcon::Grabbing);
        }
    }
}
