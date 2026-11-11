mod geometry;
mod commands;
mod selection;
mod point_drag;
pub use commands::*;
pub use selection::ParameterSelection;
use selection::{active_lane, editable_bounds, reconcile, editable};
mod gesture;
mod header;
mod rap;
use super::{child, label, state::CurveTool, theme::*, timeline::*};
use crate::{app::App, model::*};
use eframe::egui::*;
pub use gesture::CurveGesture;
pub use rap::RapGesture;

fn selected_family(app:&App)->Option<crate::synthesis::voice::VoiceFamily> {
    app.voice_selection().and_then(|key|app.synthesis.library.entry(&key)).and_then(|entry|entry.family)
        .or_else(||app.voice_inventory().and_then(|voice|voice.configuration.as_ref()).map(|config|config.family))
}

pub(super) fn standard_expression_limit(app:&App, parameter:&Parameter)->Option<f64> {
    use crate::synthesis::voice::VoiceFamily;
    if !matches!(parameter,Parameter::Tension|Parameter::Breathiness|Parameter::Gender) {return None;}
    match selected_family(app) {
        // The adapter carries f32 controls into a strict f64 +/-1.2 check.
        // Round inward so an endpoint selected in the UI remains admissible.
        Some(VoiceFamily::R1Std)=>Some(f64::from(f32::from_bits(1.2_f32.to_bits()-1))),
        Some(VoiceFamily::R2Std)=>Some(2.0),
        _=>None,
    }
}

pub(super) fn accepted_parameter_range(app:&App, parameter:&Parameter)->(f64,f64) {
    let (low,high)=parameter.range();
    standard_expression_limit(app,parameter).map_or((low,high),|limit|(low.max(-limit),high.min(limit)))
}

/// Display/edit capability only; imported curves remain intact for validation.
pub(super) fn parameter_unavailable(app: &App, parameter: &Parameter) -> Option<&'static str> {
    use crate::synthesis::voice::VoiceFamily;
    match (selected_family(app), parameter) {
        (Some(VoiceFamily::R1Std | VoiceFamily::R2Std), Parameter::ToneShift) =>
            Some("Tone shift is unavailable for this voice"),
        (Some(VoiceFamily::R1Std), Parameter::Voicing) =>
            Some("Voicing is unavailable for this voice"),
        _ => None,
    }
}

pub fn end_frame(app: &mut App, ctx: &Context) {
    gesture::end_frame(app,ctx);
    rap::end_frame(app,ctx);
    if app.parameter_lane.owner.as_ref().is_some_and(|owner|parameter_unavailable(app,&owner.target).is_some()) {
        selection::finish_drag(app,false);
        parameter_clear(app);
    }
    selection::end_frame(app,ctx);
}

pub fn show(app: &mut App, ui: &mut Ui, id: u64) {
    let Some(index) = app.view.lanes.iter().position(|l| l.id == id) else {
        return;
    };
    let mut lane = app.view.lanes[index].clone();
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0, panel_color());
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), 30.0));
    let mut header_ui = child(ui, header, ("lane-header", id));
    header::show(app, &mut header_ui, &mut lane);
    app.view.lanes[index] = lane.clone();
    let unavailable = parameter_unavailable(app, &lane.foreground);
    let canvas = Rect::from_min_max(pos2(rect.left() + gutter(), header.bottom()), rect.max);
    if canvas.height() < 5.0 {
        return;
    }
    ui.painter().rect_filled(canvas, 0, canvas_color());
    let response = ui.interact(
        canvas,
        Id::new(("parameter-canvas", id)),
        Sense::click_and_drag(),
    );
    if response.clicked() || response.drag_started() || response.secondary_clicked(){app.editor.track_focus=false;}
    scroll(ui, &response, canvas, &mut app.view.piano,10.0..=500.0);
    let pan=app.view_pan.drag(ui,super::view_pan::Target::Parameter(id),canvas,
        app.gesture.is_none() && app.group_gesture.is_none());
    app.view.piano.start=(app.view.piano.start-f64::from(pan.x)/f64::from(app.view.piano.beat_width)).max(0.0);
    grid(
        ui,
        canvas,
        &app.view.piano,
        &app.editor.project.time,
        app.view.step(),
    );
    let (min, max) = geometry::lane_range(app,&lane);
    let y_for =
        |value: f64| canvas.bottom() - ((value - min) / (max - min)) as f32 * canvas.height();
    let (low_label, high_label) = geometry::labels(&lane.foreground, min, max);
    for (value, text) in [(min, low_label), (max, high_label)] {
        let y = y_for(value).clamp(canvas.top() + 10.0, canvas.bottom() - 10.0);
        label(ui, pos2(rect.left() + 4.0, y), text, muted_color());
    }
    if lane.foreground != Parameter::Rap {ui.painter().hline(
        canvas.x_range(),
        y_for(lane.foreground.default_value()),
        Stroke::new(0.5, grid_color()),
    );}
    let Some(reference) = app.editor.active_ref().cloned() else {
        return;
    };
    if matches!(lane.foreground, Parameter::VocalMode(_))
        && app
            .editor
            .group()
            .is_some_and(|g| g.automation(&lane.foreground).is_none())
    {
        ui.painter().text(
            canvas.center(),
            Align2::CENTER_CENTER,
            crate::lformat!(
                "尚未为 \"{}\" 设置参数编辑。\n点击开始编辑此唱法。",
                crate::ui::parameter_label(&lane.foreground)
            ),
            FontId::proportional(12.0),
            muted_color(),
        );
        if response.clicked() {
            app.editor.begin(crate::i18n::t("开始编辑唱法曲线"));
            app.editor
                .group_mut()
                .unwrap()
                .automation_mut(&lane.foreground);
            app.editor.finish();
        }
        return;
    }
    reconcile(app);
    if response.clicked() || response.drag_started() || response.secondary_clicked() {
        selection::activate(app, &lane);
        ui.ctx().memory_mut(|memory| {
            if let Some(id)=memory.focused() {memory.surrender_focus(id);}
        });
    }
    let offset = reference.blick_offset;
    let group_clip = canvas;
    if lane.background == Parameter::Rap {rap::show(app,ui,&response,canvas,false);}
    for (parameter, color, editable) in [
        (&lane.background, muted_color().gamma_multiply(0.5), false),
        (&lane.foreground, accent_color(), true),
    ] {
        if *parameter == Parameter::Rap {continue;}
        let Some(curve) = app.editor.group().unwrap().automation(parameter) else {
            continue;
        };
        let (low, high) = geometry::range(app,parameter, lane.zoom);
        let map = |(t, v): (i64, f64)| {
            pos2(
                x_at(canvas, &app.view.piano, beats(t.saturating_add(offset))),
                canvas.bottom() - ((v - low) / (high - low)) as f32 * canvas.height(),
            )
        };
        let painter = ui
            .painter()
            .with_clip_rect(group_clip.intersect(ui.clip_rect()));
        if curve.points.is_empty() {
            let y = map((0, parameter.default_value())).y;
            painter.hline(canvas.x_range(), y, Stroke::new(1.0, color));
        } else {
            let first = map(curve.points[0]);
            let last = map(*curve.points.last().unwrap());
            painter.hline(
                canvas.left()..=first.x.max(canvas.left()).min(canvas.right()),
                first.y,
                Stroke::new(1.0, color),
            );
            painter.hline(
                last.x.min(canvas.right()).max(canvas.left())..=canvas.right(),
                last.y,
                Stroke::new(1.0, color),
            );
            for pair in curve.points.windows(2) {
                let a = map(pair[0]);
                let b = map(pair[1]);
                if curve.mode == "cosine" {
                    let points: Vec<_> = (0..=24)
                        .map(|i| {
                            let t = i as f32 / 24.0;
                            pos2(
                                a.x + (b.x - a.x) * t,
                                a.y + (b.y - a.y) * (1.0 - (t * std::f32::consts::PI).cos()) * 0.5,
                            )
                        })
                        .collect();
                    painter.add(Shape::line(points, Stroke::new(1.2, color)));
                } else if curve.mode == "cubic" {
                    let steps = ((b.x - a.x).abs().ceil() as usize).clamp(2, 1024);
                    let points =
                        (0..=steps)
                            .map(|i| {
                                let time = pair[0].0 as f64
                                    + (i128::from(pair[1].0) - i128::from(pair[0].0)) as f64 * i as f64 / steps as f64;
                                map((
                                    time.round() as i64,
                                    f64::from(curve.sample(
                                        time.round() as i64,
                                        parameter.default_value() as f32,
                                    )),
                                ))
                            })
                            .collect();
                    painter.add(Shape::line(points, Stroke::new(1.2, color)));
                } else {
                    painter.line_segment([a, b], Stroke::new(1.2, color));
                }
            }
            if editable && unavailable.is_none() && lane.tool == CurveTool::Point {
                for &point in &curve.points {
                    painter.circle_filled(map(point), if selection::contains(app,id,point.0) {4.0} else {2.0}, accent_color());
                }
            }
        }
    }
    if let Some(reason) = unavailable {
        ui.painter().rect_filled(Rect::from_min_size(canvas.min,vec2(canvas.width(),30.0)),0,panel_color());
        ui.painter().text(canvas.left_top()+vec2(8.0,15.0),Align2::LEFT_CENTER,
            crate::i18n::t(reason),FontId::proportional(12.0),muted_color());
        response.on_hover_text(crate::i18n::t(reason));
        playhead_line(ui,canvas,&app.view.piano,app.editor.transport.position);
        return;
    }
    if standard_expression_limit(app,&lane.foreground).is_some()
        && let Some(curve)=app.editor.group().and_then(|group|group.automation(&lane.foreground)) {
        let (low,high)=accepted_parameter_range(app,&lane.foreground);
        if curve.points.iter().any(|(_,value)|*value<low||*value>high) {
            ui.painter().text(canvas.left_top()+vec2(8.0,15.0),Align2::LEFT_CENTER,
                crate::i18n::t("This curve contains values outside this voice's supported range"),FontId::proportional(12.0),Color32::LIGHT_RED);
        }
    }
    if lane.foreground == Parameter::Rap {
        let y=canvas.center().y+1.0;
        ui.painter().extend(Shape::dotted_line(&[pos2(canvas.left(),y),pos2(canvas.right(),y)], muted_color(), 2.5, 0.5));
        rap::show(app,ui,&response,canvas,true);
        playhead_line(ui,canvas,&app.view.piano,app.editor.transport.position);
        return;
    }
    let point_from = |p: Pos2, app: &App| {
        let time = ticks(beat_at(canvas, &app.view.piano, p.x)).max(0).saturating_sub(offset);
        let (allowed_low,allowed_high)=accepted_parameter_range(app,&lane.foreground);
        let value = (min + (canvas.bottom() - p.y) as f64 / canvas.height() as f64 * (max - min))
            .clamp(allowed_low,allowed_high);
        (time, value as f32 as f64)
    };
    let nearest = |p: Pos2, app: &App| {
        app.editor
            .group()
            .unwrap()
            .automation(&lane.foreground)
            .and_then(|curve| {
                curve
                    .points
                    .iter()
                    .find(|(t, v)| {
                        pos2(x_at(canvas, &app.view.piano, beats(t.saturating_add(offset))), y_for(*v))
                                .distance(p)
                                < 8.0
                    })
                    .map(|p| p.0)
            })
    };
    gesture::interaction(app, ui, &response, canvas, &lane, group_clip);
    if lane.tool == CurveTool::Point
        && response.double_clicked()
        && let Some(p) = response.interact_pointer_pos()
        && group_clip.contains(p)
    {
        let (t, v) = point_from(p, app);
        app.editor.begin(crate::i18n::t("添加参数控制点"));
        app.editor
            .group_mut()
            .unwrap()
            .automation_mut(&lane.foreground)
            .insert(t, v);
        app.editor.finish();
    }
    if lane.tool == CurveTool::Point && response.clicked()
        && let Some(p)=response.interact_pointer_pos() {
        selection::select(app, nearest(p,app), ui.input(|i|i.modifiers.shift));
    }
    if lane.tool == CurveTool::Point && response.drag_started_by(PointerButton::Primary)
        && let Some(p)=ui.input(|i|i.pointer.press_origin())
        && group_clip.contains(p) && let Some(time)=nearest(p,app) {
        point_drag::begin(app,id,time);
    }
    if lane.tool == CurveTool::Point && response.dragged_by(PointerButton::Primary)
        && let Some(p)=response.interact_pointer_pos() {
        let point=point_from(p,app);point_drag::update(app,id,point);
    }
    if lane.tool == CurveTool::Point && response.drag_stopped_by(PointerButton::Primary) {
        selection::finish_drag(app,true);
    }
    crate::ui::theme::menu::context_menu(&response,|ui| {
        selection::menu(app,ui);
        if ui.button(crate::i18n::t("清空此参数的控制点")).clicked() {
            app.editor.begin(crate::i18n::t("清空参数"));
            app.editor
                .group_mut()
                .unwrap()
                .automation_mut(&lane.foreground)
                .points
                .clear();
            app.editor.finish();
            ui.close();
        }
    });
    playhead_line(ui, canvas, &app.view.piano, app.editor.transport.position);
}
