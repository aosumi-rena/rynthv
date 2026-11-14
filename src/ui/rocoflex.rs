use crate::{app::App, i18n::t, model::{RocoflexTrack, Track},
    synthesis::rocoflex::{Provider, Settings, wire}};
use super::{icons::{Icon, IconButton}, rocoflex_surface::{Change, Surface}, voice_finetuning};
use eframe::{egui::*, egui_wgpu::RenderState};
use std::{collections::BTreeMap, sync::mpsc, thread};

#[derive(Default)]
pub struct Integration {
    gpu: Option<RenderState>,
    settings: Option<Settings>,
    pub provider: Option<Provider>,
    detection: Option<mpsc::Receiver<Result<Provider, String>>>,
    provider_changed: bool,
    error: Option<String>,
    tracks: BTreeMap<u64, TrackWindow>,
    raise: Option<u64>,
    lab: Surface,
    draft: Option<usize>,
}

#[derive(Default)]
struct TrackWindow {
    editor: Surface,
    collapsed: bool,
    position: Option<Pos2>,
    project_revision: Option<u64>,
}

impl Integration {
    pub fn new(gpu: Option<RenderState>) -> Self { Self { gpu, ..Default::default() } }

    pub fn refresh(&mut self, settings: &Settings, ctx: &Context) {
        self.settings = Some(settings.clone());
        self.provider_changed |= self.provider.take().is_some();
        self.error = None;
        for window in self.tracks.values_mut() { window.editor.disconnect(); }
        self.lab.disconnect();
        self.detection = None;
        if settings.disabled {
            self.clear();
            return;
        }
        let (tx, rx) = mpsc::channel();
        let settings = settings.clone();
        let ctx = ctx.clone();
        self.detection = Some(rx);
        if let Err(error) = thread::Builder::new().name("rocoflex-detection".into()).spawn(move || {
            if tx.send(settings.detect()).is_ok() { ctx.request_repaint_of(ViewportId::ROOT); }
        }) {
            self.detection = None;
            self.error = Some(error.to_string());
        }
    }

    pub fn poll(&mut self, settings: &Settings, ctx: &Context) -> Option<Option<Provider>> {
        if self.settings.as_ref() != Some(settings) { self.refresh(settings, ctx); }
        if let Some(result) = self.detection.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.detection = None;
            match result {
                Ok(provider) => { self.provider = Some(provider); self.provider_changed = true; }
                Err(error) => self.error = Some(error),
            }
        }
        std::mem::take(&mut self.provider_changed).then(|| self.provider.clone())
    }

    pub fn clear(&mut self) { self.tracks.clear(); self.raise = None; }

    pub fn owns_keyboard(&self, ctx: &Context) -> bool {
        self.tracks.values().any(|window| window.editor.owns_keyboard(ctx))
    }

    pub fn owns_drop(&self, ctx: &Context) -> bool {
        self.tracks.values().any(|window| window.editor.at_pointer(ctx, false))
    }

    fn open(&mut self, track: u64) -> &mut TrackWindow {
        self.raise = Some(track);
        self.tracks.entry(track).or_default()
    }
}

pub fn choose_executable(app: &mut App, ctx: &Context) {
    let mut dialog = rfd::FileDialog::new();
    #[cfg(windows)]
    { dialog = dialog.add_filter("EXE", &["exe"]); }
    if let Some(parent) = app.preferences.rocoflex.executable.as_deref().and_then(std::path::Path::parent) {
        dialog = dialog.set_directory(parent);
    }
    if let Some(file) = dialog.pick_file() {
        app.preferences.rocoflex.executable = Some(file);
        app.preferences.rocoflex.disabled = false;
        app.rocoflex.refresh(&app.preferences.rocoflex, ctx);
    }
}

pub fn can_unload(app: &App) -> bool {
    app.preferences.rocoflex.executable.is_some()
        || app.rocoflex.provider.is_some() || app.rocoflex.detection.is_some()
}

pub fn unload(app: &mut App, ctx: &Context) {
    app.preferences.rocoflex = Settings { disabled: true, executable: None };
    app.voice_finetuning.open = false;
    app.rocoflex.refresh(&app.preferences.rocoflex, ctx);
    app.synthesis.set_rocoflex_provider(None);
}

pub fn status(app: &App) -> &str {
    if app.preferences.rocoflex.disabled { return t("ROCOFLEX_UNLOADED"); }
    if let Some(provider) = &app.rocoflex.provider {
        provider.executable.file_name().and_then(|p| p.to_str()).unwrap_or("Rocoflex")
    } else if app.rocoflex.detection.is_some() { t("ROCOFLEX_DETECTING") }
    else { t("ROCOFLEX_NOT_FOUND") }
}

pub fn diagnostic(app: &App) -> Option<&str> { app.rocoflex.error.as_deref() }

pub fn track_menu(app: &mut App, ui: &mut Ui, track: &Track) {
    if track.is_instrumental() { return; }
    let available = app.rocoflex.provider.is_some();
    ui.separator();
    let mut enabled = track.rocoflex.as_ref().is_some_and(|r| r.enabled);
    if ui.add_enabled(available, Checkbox::new(&mut enabled, t("ROCOFLEX_APPLY")))
        .on_disabled_hover_text(t("ROCOFLEX_UNAVAILABLE")).changed()
    {
        if enabled {
            app.rocoflex.open(track.id);
        } else if let Some(current) = &track.rocoflex {
            update_track(app, track.id, RocoflexTrack { enabled: false, ..current.clone() });
        }
        ui.close();
    }
    if ui.add_enabled(available, Button::new(t("ROCOFLEX_OPEN")))
        .on_disabled_hover_text(t("ROCOFLEX_UNAVAILABLE")).clicked()
    {
        app.rocoflex.open(track.id);
        ui.close();
    }
}

fn update_track(app: &mut App, id: u64, settings: RocoflexTrack) {
    if app.editor.project.tracks.iter().find(|t| t.id == id).and_then(|t| t.rocoflex.as_ref()) == Some(&settings) { return; }
    app.editor.finish();
    app.editor.begin("ROCOFLEX_APPLY");
    if let Some(track) = app.editor.project.tracks.iter_mut().find(|track| track.id == id) {
        track.rocoflex = Some(settings);
    }
    app.editor.finish();
}

fn preset(state: wire::State, enabled: bool) -> RocoflexTrack {
    RocoflexTrack { enabled, preset: state.document, pregain_db: state.pregain_db, pitch_semitones: state.pitch_semitones }
}

fn header(ui: &mut Ui, id: Id, title: &str, collapsed: &mut bool, ready: bool, confirmed: bool) -> (bool, bool) {
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    ui.interact(bar, id.with("drag"), Sense::drag());
    let toggle = Rect::from_min_size(bar.min, vec2(28.0, bar.height()));
    let close = Rect::from_min_size(pos2(bar.right() - 28.0, bar.top()), vec2(28.0, bar.height()));
    let font = TextStyle::Button.resolve(ui.style());
    let text = t("ROCOFLEX_CONFIRM");
    let width = ui.painter().layout_no_wrap(text.into(), font, Color32::WHITE).size().x + 18.0;
    let confirm = Rect::from_min_size(pos2(close.left() - width - 4.0, bar.top()), vec2(width, bar.height()));
    let tick = Rect::from_min_max(pos2(confirm.left() - 24.0, bar.top()), confirm.left_bottom());
    let title_clip = Rect::from_min_max(toggle.right_top(), tick.left_bottom());
    ui.painter().with_clip_rect(title_clip).text(
        title_clip.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER,
        title, TextStyle::Heading.resolve(ui.style()), ui.visuals().strong_text_color(),
    );
    let fold = if *collapsed { Icon::ChevronRight } else { Icon::ChevronDown };
    if ui.place(toggle, IconButton::new(fold, t(if *collapsed { "ROCOFLEX_EXPAND" } else { "ROCOFLEX_COLLAPSE" }))).clicked() {
        *collapsed = !*collapsed;
        ui.ctx().request_repaint();
    }
    if confirmed {
        let p = tick.center();
        ui.painter().add(Shape::line(
            vec![p + vec2(-5.0, 0.0), p + vec2(-1.0, 4.0), p + vec2(6.0, -4.0)],
            Stroke::new(1.6, ui.visuals().selection.stroke.color),
        ));
    }
    let commit = ui.add_enabled_ui(ready, |ui| ui.place(confirm, Button::new(text))).inner;
    let closed = ui.place(close, IconButton::new(Icon::Close, t("关闭"))).clicked();
    (closed, ready && commit.clicked())
}

pub fn show(app: &mut App, ctx: &Context) {
    let integration = &mut app.rocoflex;
    let (Some(provider), Some(gpu)) = (&integration.provider, &integration.gpu) else {
        app.voice_finetuning.open = false;
        return;
    };
    let revision = app.editor.revision;
    if integration.tracks.is_empty() && !app.voice_finetuning.open {
        if integration.draft.take().is_some() { integration.lab.disconnect(); }
        return;
    }
    integration.tracks.retain(|id, _| app.editor.project.tracks.iter().any(|track| track.id == *id));
    let mut changes = Vec::new();
    let mut close = Vec::new();
    for (&id, window) in &mut integration.tracks {
        let track = app.editor.project.tracks.iter().find(|track| track.id == id).unwrap();
        if window.project_revision != Some(revision) {
            window.editor.sync(track.rocoflex.as_ref());
            window.project_revision = Some(revision);
        }
        let enabled = track.rocoflex.as_ref().is_some_and(|r| r.enabled);
        let window_id = Id::new(("rocoflex-track", id));
        let raise = integration.raise == Some(id);
        if raise { window.collapsed = false; }
        let room = (ctx.content_rect().size() - vec2(40.0, 64.0)).max(vec2(240.0, 170.0));
        let default = room.min(vec2(920.0, 600.0));
        let mut popup = Window::new(format!("Rocoflex — {}", track.name)).id(window_id)
            .title_bar(false).movable(false).collapsible(false)
            .default_size(default).min_width(room.x.min(460.0)).max_size(room);
        if window.collapsed {
            popup = popup.auto_sized().fixed_size(vec2(default.x, 28.0));
        } else {
            popup = popup.min_height(room.y.min(320.0));
        }
        if let Some(position) = window.position
            && ctx.read_response(window_id.with("drag")).is_some_and(|r| r.dragged_by(PointerButton::Primary))
        {
            popup = popup.current_pos(position + ctx.input(|i| i.pointer.delta()));
        }
        let mut closed = false;
        let mut confirm = false;
        let response = popup.show(ctx, |ui| {
            if !ui.is_sizing_pass() && !window.collapsed { window.editor.receive(ui.ctx(), gpu); }
            let available = window.editor.confirmable();
            let confirmed = enabled && window.editor.applied();
            (closed, confirm) = header(ui, window_id, &track.name, &mut window.collapsed,
                available && !confirmed, confirmed);
            if !window.collapsed && !closed {
                let size = ui.available_size().min(room).max(vec2(160.0, 100.0));
                window.editor.show(ui, size, provider, gpu);
            }
        });
        if let Some(response) = response {
            window.position = Some(response.response.rect.min);
            let dragging = ctx.read_response(window_id.with("drag")).is_some_and(|r| r.dragged());
            if raise || dragging { ctx.move_to_top(response.response.layer_id); }
        }
        if window.collapsed { window.editor.pause(ctx, gpu); }
        else { window.editor.concealed(); }
        if !closed && confirm && window.editor.confirmable() && let Some(state) = window.editor.state.clone() {
            let accepted = preset(state, true);
            window.editor.accept(accepted.clone());
            changes.push((id, accepted));
        }
        if closed { close.push(id); }
    }
    integration.raise = None;
    for id in close { integration.tracks.remove(&id); }

    if app.voice_finetuning.open {
        let empty = crate::synthesis::voice::VoiceCatalog::default();
        let catalog = app.synthesis.library.catalog().unwrap_or(&empty);
        voice_finetuning::show(ctx, &mut app.voice_finetuning, catalog, |ui, index, draft| {
            if integration.draft != Some(index) { integration.lab.disconnect(); integration.draft = Some(index); }
            let lab = &mut integration.lab;
            lab.sync(draft.morph.as_ref());
            let size = ui.available_size().max(vec2(160.0, 100.0));
            let visible = lab.show(ui, size, provider, gpu);
            lab.concealed();
            if let Some(Change { state, first: false }) = visible {
                let value = preset(state, false);
                lab.accept(value.clone());
                draft.morph = Some(value);
            }
        });
    }
    if !app.voice_finetuning.open { integration.lab.disconnect(); integration.draft = None; }
    for (id, settings) in changes { update_track(app, id, settings); }
}

