use crate::synthesis::rocoflex::wire;
use eframe::egui::*;

#[derive(Default)]
pub(super) struct Input {
    pub events: Vec<wire::Event>,
    pub files: Vec<std::path::PathBuf>,
    pub focused: bool,
    pub hovered: bool,
    buttons: u8,
    modifiers: u8,
    last_frame: Option<u64>,
    pointer: Option<[f32; 2]>,
    motion: [f32; 2],
}

impl Input {
    pub fn collect(&mut self, ui: &Ui, response: &Response, rect: Rect, ratio: f32) {
        let ctx = ui.ctx();
        let frame = ctx.cumulative_frame_nr();
        if self.last_frame == Some(frame) { return; }
        self.last_frame = Some(frame);
        let within = |p| rect.contains(p) && ctx.layer_id_at(p) == Some(ui.layer_id());
        let at = ctx.input(|input| input.pointer.hover_pos());
        if !self.focused && !self.hovered && self.buttons == 0 && !at.is_some_and(within) { return; }
        let (pointer, focused, modifiers, events, files) = ctx.input(|input| (input.pointer.clone(), input.focused,
            input.modifiers, input.raw.events.clone(), input.raw.dropped_files.clone()));
        {
            let local = |p: Pos2| { let d = (p - rect.min) / ratio; [d.x, d.y] };
            let hovered = pointer.hover_pos().is_some_and(within);
            if !focused || pointer.any_pressed() && !hovered && self.buttons == 0 || self.focused && !response.has_focus() && !hovered && self.buttons == 0 {
                self.focused = false;
                self.buttons = 0;
            } else if hovered && pointer.any_pressed() {
                self.focused = true;
            }
            if self.focused && !response.has_focus() { response.request_focus(); }
            if !self.focused && response.has_focus() { response.surrender_focus(); }
            if self.hovered && !hovered && self.buttons == 0 { self.leave(); }
            self.hovered = hovered;
            let mods = if self.focused { bits(modifiers) } else { 0 };
            if mods != self.modifiers { self.modifiers = mods; self.events.push(wire::Event::Modifiers(mods)); }
            for event in &events {
                use crate::synthesis::rocoflex::wire::Event as W;
                match event {
                    Event::PointerMoved(at) if self.buttons != 0 || within(*at) => self.push_move(local(*at)),
                    Event::PointerButton { pos, button, pressed, modifiers } => {
                        let bit = 1 << *button as u8;
                        if *pressed && within(*pos) || !pressed && self.buttons & bit != 0 {
                            if *pressed { self.buttons |= bit; } else { self.buttons &= !bit; }
                            self.events.push(W::Button(local(*pos), *button as u8, *pressed, bits(*modifiers)));
                        }
                    }
                    Event::PointerGone if self.focused || hovered || self.buttons != 0 => self.leave(),
                    Event::MouseMoved(delta) if self.buttons != 0 => self.push_motion([delta.x, delta.y]),
                    Event::MouseWheel { unit, delta, phase, modifiers } if hovered => {
                        let step = match unit { MouseWheelUnit::Point => *delta / ratio, _ => *delta };
                        let unit = match unit { MouseWheelUnit::Point => 0, MouseWheelUnit::Line => 1, MouseWheelUnit::Page => 2 };
                        self.events.push(W::Scroll([step.x, step.y], unit, phase_index(*phase), bits(*modifiers)));
                    }
                    Event::Zoom(v) if hovered => self.events.push(W::Zoom(*v)),
                    Event::Rotate(v) if hovered => self.events.push(W::Rotate(*v)),
                    Event::Touch { device_id, id, phase, pos, force } if hovered || self.buttons != 0 => {
                        self.events.push(W::Touch(device_id.0, id.0, phase_index(*phase), local(*pos), *force));
                    }
                    Event::Key { key, physical_key, pressed, repeat, modifiers } if self.focused => {
                        self.events.push(W::Key(key.name().into(), physical_key.map(|key| key.name().into()), *pressed, *repeat, bits(*modifiers)));
                    }
                    Event::Copy if self.focused => self.events.push(W::Copy),
                    Event::Cut if self.focused => self.events.push(W::Cut),
                    Event::Paste(s) if self.focused => self.events.push(W::Paste(s.clone())),
                    Event::Text(s) if self.focused => self.events.push(W::Text(s.clone())),
                    Event::Ime(ImeEvent::Preedit { text, active_range_chars }) if self.focused =>
                        self.events.push(W::Preedit(text.clone(), active_range_chars.clone())),
                    Event::Ime(ImeEvent::Commit(text)) if self.focused => self.events.push(W::Commit(text.clone())),
                    Event::Ime(ImeEvent::DeleteSurrounding { before_chars, after_chars }) if self.focused =>
                        self.events.push(W::DeleteSurrounding(*before_chars, *after_chars)),
                    _ => {}
                }
            }
            if hovered {
                self.files.extend(files.iter().map(|file| file.path().to_owned()));
            }
        }
    }

    pub fn reset_pointer(&mut self) { self.pointer = None; }

    pub fn has_events(&self) -> bool { !self.events.is_empty() || !self.files.is_empty() || self.motion != [0.0; 2] }

    pub fn is_pointer_idle(&self) -> bool { self.buttons == 0 && !self.has_events() }

    pub fn has_edits(&self) -> bool {
        self.buttons != 0 || !self.files.is_empty() || self.events.iter().any(|event| matches!(event,
            wire::Event::Button(..) | wire::Event::Scroll(..) | wire::Event::Zoom(..) | wire::Event::Rotate(..)
            | wire::Event::Touch(..) | wire::Event::Key(..) | wire::Event::Text(..) | wire::Event::Cut
            | wire::Event::Paste(..) | wire::Event::Commit(..) | wire::Event::DeleteSurrounding(..)))
    }

    pub fn blur(&mut self) {
        if !self.focused && !self.hovered { return; }
        self.focused = false;
        self.hovered = false;
        self.buttons = 0;
        self.modifiers = 0;
        self.leave();
        self.events.push(wire::Event::Modifiers(0));
    }

    fn leave(&mut self) {
        self.pointer = None;
        self.events.push(wire::Event::Leave);
    }

    fn push_move(&mut self, at: [f32; 2]) {
        if self.pointer == Some(at) { return; }
        self.pointer = Some(at);
        if let Some(wire::Event::Move(end)) = self.events.last_mut() { *end = at; }
        else { self.events.push(wire::Event::Move(at)); }
    }

    fn push_motion(&mut self, delta: [f32; 2]) {
        self.motion[0] += delta[0];
        self.motion[1] += delta[1];
    }

    pub fn take_events(&mut self) -> Vec<wire::Event> {
        let mut events = std::mem::take(&mut self.events);
        let motion = std::mem::take(&mut self.motion);
        if motion != [0.0; 2] { events.push(wire::Event::Motion(motion)); }
        events
    }
}

fn bits(m: Modifiers) -> u8 {
    u8::from(m.alt) | u8::from(m.ctrl) << 1 | u8::from(m.shift) << 2 | u8::from(m.mac_cmd) << 3 | u8::from(m.command) << 4
}

fn phase_index(phase: TouchPhase) -> u8 {
    match phase { TouchPhase::Start => 0, TouchPhase::Move => 1, TouchPhase::End => 2, TouchPhase::Cancel => 3 }
}
