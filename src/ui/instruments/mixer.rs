mod header;
mod plan;
mod strip;
mod strip_header;
use crate::{
    app::App,
    synthesis::instruments::microphones::Configuration,
    ui::{theme::*, track_color::TrackColors},
};
use eframe::egui::*;

pub fn show(app: &mut App, ui: &mut Ui) {
    if ui.button(crate::i18n::t("Mic Mixer")).clicked() {
        app.instrument_mixer = true;
    }
}
pub fn dialog(app: &mut App, ctx: &Context) {
    if !app.instrument_mixer || app.settings {
        return;
    }
    let Some(entry) = super::current(app).cloned() else {
        app.instrument_mixer = false;
        return;
    };
    let mut config = match Configuration::read(
        app.editor.current_track().mixer.get("micParams"),
        &entry.microphones,
        entry.room.as_deref(),
    ) {
        Ok(v) => v,
        Err(e) => {
            app.error = Some(e);
            app.instrument_mixer = false;
            return;
        }
    };
    let pack = app
        .synthesis
        .library
        .settings
        .instruments
        .room_pack()
        .map(|p| crate::synthesis::instruments::rooms::load(&p))
        .transpose();
    let pack = match pack {
        Ok(v) => v,
        Err(e) => {
            app.error = Some(e);
            None
        }
    };
    let before = config.clone();
    let mut actions = header::Actions {
        close: false,
        applied: false,
        navigate: None,
    };
    let viewport = ctx.content_rect().shrink(16.);
    let fit = (viewport.width() / 1059.)
        .min(viewport.height() / 502.)
        .clamp(0.1, 1.);
    let scale = 0.75 * fit;
    let size = vec2(1059., 502.) * fit;
    let color = TrackColors::new(app.editor.current_track()).accent;
    let editing = Id::new("microphone-mixer-edit");
    let owned = ctx.data(|d| d.get_temp::<u64>(editing)) == Some(app.editor.track);
    Window::new(crate::i18n::t("Mic Mixer"))
        .id(Id::new("instrument-mic-mixer"))
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .constrain_to(viewport)
        .default_pos(viewport.center() - size * 0.5)
        .fixed_size(size)
        .frame(Frame::new().fill(PANEL).corner_radius(10).inner_margin(0))
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.spacing_mut().item_spacing = vec2(3., 3.) * fit;
            let header_rect = Rect::from_min_size(rect.min, vec2(size.x, 44. * fit));
            actions = header::show(app, ui, header_rect, color, fit);
            ui.painter()
                .hline(rect.x_range(), header_rect.bottom(), Stroke::new(1., EDGE));
            let body = Rect::from_min_max(
                rect.min + vec2(9., 54.) * fit,
                rect.max - vec2(9., 10.) * fit,
            );
            let mut content = crate::ui::child(ui, body, "microphone-body");
            content.add_enabled_ui(
                !actions.applied && (!app.editor.is_editing() || owned),
                |ui| {
                    let plan_rect = Rect::from_min_size(body.min, vec2(381. * fit, body.height()));
                    plan::show(
                        ui,
                        &entry,
                        &mut config,
                        pack.as_deref(),
                        plan_rect,
                        color,
                        scale,
                    );
                    let mut x = plan_rect.right() + 3. * fit;
                    for group in ["Close", "Decca", "Wide", "Ambi"] {
                        let width = strip::width(group, &entry.microphones) * fit;
                        if width > 0. {
                            let strip_rect = Rect::from_min_size(
                                pos2(x, body.top()),
                                vec2(width, body.height()),
                            );
                            strip::show(
                                ui,
                                strip_rect,
                                group,
                                &entry.microphones,
                                &mut config,
                                scale,
                                color,
                            );
                            x += width + 3. * fit;
                        }
                    }
                },
            );
        });
    if config != before && !actions.applied && (!app.editor.is_editing() || owned) {
        if !owned {
            app.commit_lyric();
            app.editor.begin(crate::i18n::t("Mic Mixer"));
            ctx.data_mut(|d| d.insert_temp(editing, app.editor.track));
        }
        let track = app
            .editor
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == app.editor.track)
            .unwrap();
        track.mixer.insert(
            "micParams".into(),
            serde_json::to_value(config).expect("finite microphone settings"),
        );
    }
    if ctx.data(|d| d.get_temp::<u64>(editing)).is_some() && !ctx.input(|i| i.pointer.any_down()) {
        app.editor.finish();
        ctx.data_mut(|d| d.remove::<u64>(editing));
    }
    if let Some(track) = actions.navigate {
        app.commit_lyric();
        app.editor.select_track(track);
    }
    if actions.close
        || (!Popup::is_any_open(ctx)
            && !app.control_presets.is_naming()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)))
    {
        app.instrument_mixer = false;
    }
}
