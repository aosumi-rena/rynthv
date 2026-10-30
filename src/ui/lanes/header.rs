use crate::{
    app::App,
    model::Parameter,
    ui::{
        icons::{Icon, IconButton},
        state::{CurveTool, DragPayload, Lane},
        theme::*,
    },
};
use eframe::egui::*;

pub(super) fn show(app: &mut App, ui: &mut Ui, lane: &mut Lane) {
    let mut modes = app
        .voice_inventory()
        .and_then(|v| v.configuration.as_ref())
        .map(|v| v.vocal_mode_names().to_vec())
        .unwrap_or_default();
    let settings=app.synthesis.library.settings.experiments;
    let mut extra=Vec::new();
    if settings.active(settings.dds_controls) {
        if let Ok((_,controls))=app.editor.experiment_controls() {
            if controls.dds.default_as_style {extra.push(Parameter::Experimental {key:"ddsDefaultStyle".into(),label:"Default vocal mode offset".into()});}
            for (index,slot) in controls.dds.phonemes.iter().enumerate() {
                extra.push(Parameter::Experimental {key:format!("ddsPhoneMix_{}",slot.id),label:format!("{} · {}",index+1,slot.token)});
            }
        }
    }
    if settings.active(settings.growl) {
        for (key,label) in [("growl_amplitude","嘶吼幅度"),("growl_frequency","嘶吼频率")] {
            extra.push(Parameter::Experimental {key:key.into(),label:label.into()});
        }
    }
    if settings.active(settings.component_curves) {
        for name in &modes {for (key,label) in [("pitch","音高"),("timbre","音色"),("pronunciation","发音")] {
            if key!="timbre" && app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|c.family!=crate::synthesis::voice::VoiceFamily::Sv2) {continue;}
            extra.push(Parameter::Experimental {key:format!("vocalModeComponent_{key}:{name}"),label:format!("{name} · {}",crate::i18n::t(label))});
        }}
    }
    if settings.active(settings.voice_blending) {
        if let Ok((_,controls))=app.editor.experiment_controls() {for target in controls.voices {
            let name=app.synthesis.library.entry(&target.voice).map_or_else(||target.voice.revision.to_string(),|e|format!("{} · {}",e.display_name,e.revision));
            extra.push(Parameter::Experimental {key:format!("voiceBlend_{}:{}",target.voice.database_id,target.voice.revision),label:format!("{} · {name}",crate::i18n::t("歌声混合"))});
        }}
    }
    if !settings.enabled && app.voice_inventory().and_then(|v|v.configuration.as_ref()).is_some_and(|c|c.family==crate::synthesis::voice::VoiceFamily::Sv1) {modes.clear();}
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.horizontal(|ui| {
        let (r, response) = ui.allocate_exact_size(vec2(48.0, 30.0), Sense::drag());
        Icon::Curve.paint(ui, r.shrink2(vec2(17.0, 8.0)), MUTED);
        response.dnd_set_drag_payload(DragPayload::Lane(lane.id));
        selector(ui, &mut lane.foreground, &modes,&extra);
        if ui
            .add(
                IconButton::new(Icon::Swap, crate::i18n::t("交换前景与背景参数"))
                    .size(vec2(24.0, 30.0))
                    .icon_size(14.0),
            )
            .clicked()
        {
            std::mem::swap(&mut lane.foreground, &mut lane.background);
        }
        selector(ui, &mut lane.background, &modes,&extra);

        ui.add_enabled_ui(lane.foreground != Parameter::Rap, |ui| {
        ui.add_space(10.0);
        for (tool, tip) in [
            (CurveTool::Point, crate::i18n::t("控制点工具")),
            (CurveTool::Pencil, crate::i18n::t("铅笔工具")),
            (CurveTool::Line, crate::i18n::t("直线工具")),
        ] {
            if tool_button(ui, lane.tool == tool, tip, |ui, r, c| {
                paint_tool(ui, r, c, tool)
            })
            .clicked()
            {
                lane.tool = tool;
            }
        }
        ui.add_space(6.0);
        let mode = app
            .editor
            .group()
            .and_then(|g| g.automation(&lane.foreground))
            .map_or("cubic", |c| c.mode.as_str())
            .to_owned();
        for (value, tip) in [
            ("linear", crate::i18n::t("线性插值")),
            ("cosine", crate::i18n::t("余弦插值")),
            ("cubic", crate::i18n::t("三次样条插值")),
        ] {
            if ui
                .add_enabled_ui(app.editor.reference.is_some(), |ui| {
                    tool_button(ui, mode == value, tip, |ui, r, c| {
                        paint_interpolation(ui, r, c, value)
                    })
                })
                .inner
                .clicked()
            {
                app.editor.begin(crate::i18n::t("参数插值模式"));
                app.editor
                    .group_mut()
                    .unwrap()
                    .automation_mut(&lane.foreground)
                    .mode = value.into();
                app.editor.finish();
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new(crate::i18n::t("缩放")).color(MUTED));
        ui.add(
            DragValue::new(&mut lane.zoom)
                .range(50.0..=200.0)
                .speed(10.0)
                .suffix("%"),
        );
        });
    });
}

fn selector(ui: &mut Ui, parameter: &mut Parameter, modes: &[String],extra:&[Parameter]) {
    ui.menu_button(RichText::new(crate::ui::parameter_label(parameter)).color(TEXT), |ui| {
        let font = TextStyle::Button.resolve(ui.style());

        let text_width = Parameter::ALL
            .iter()
            .map(|p|crate::ui::parameter_label(p).into_owned())
            .chain(modes.iter().cloned())
            .chain(extra.iter().map(|p|crate::ui::parameter_label(p).into_owned()))
            .chain([crate::i18n::t("基本参数"), crate::i18n::t("唱法")].map(str::to_owned))
            .map(|label| {
                ui.painter()
                    .layout_no_wrap(label.into(), font.clone(), TEXT)
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max);
        let width = (text_width + 2.0 * ui.spacing().button_padding.x + 32.0)
            .max(10.0 * font.size);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.label(RichText::new(crate::i18n::t("基本参数")).color(MUTED));
                for p in Parameter::ALL {
                    let label = crate::ui::parameter_label(&p).into_owned();
                    parameter_option(ui, parameter, p, &label, width);
                }
            });
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.label(RichText::new(crate::i18n::t("唱法")).color(MUTED));
                for name in modes {
                    parameter_option(
                        ui,
                        parameter,
                        Parameter::VocalMode(format!("vocalMode_{name}")),
                        name,
                        width,
                    );
                }
                if !extra.is_empty() {
                    ui.separator();ui.label(RichText::new(crate::i18n::t("实验功能")).color(MUTED));
                    for value in extra {parameter_option(ui,parameter,value.clone(),&crate::ui::parameter_label(value),width);}
                }
            });
        });
    });
}
fn parameter_option(
    ui: &mut Ui,
    parameter: &mut Parameter,
    value: Parameter,
    label: &str,
    width: f32,
) {
    let size = vec2(width, ui.spacing().interact_size.y);
    if ui
        .add_sized(
            size,
            Button::selectable(*parameter == value, (label, Atom::grow()))
                .min_size(size)
                .wrap_mode(TextWrapMode::Extend),
        )
        .clicked()
    {
        *parameter = value;
        ui.close();
    }
}
fn tool_button(
    ui: &mut Ui,
    selected: bool,
    tip: &str,
    paint: impl FnOnce(&Ui, Rect, Color32),
) -> Response {
    let (r, response) = ui.allocate_exact_size(vec2(34.0, 30.0), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            r.shrink(1.0),
            2,
            if selected {
                BUTTON_ACTIVE
            } else {
                BUTTON_HOVER
            },
        );
    }
    paint(
        ui,
        r.shrink2(vec2(10.0, 8.0)),
        if selected { TEXT } else { MUTED },
    );
    response.on_hover_text(tip)
}
fn paint_tool(ui: &Ui, r: Rect, c: Color32, tool: CurveTool) {
    match tool {
        CurveTool::Point => Icon::PitchAnchor.paint(ui, r, c),
        CurveTool::Pencil => {
            let a = r.left_bottom() + vec2(1.0, -1.0);
            let b = r.right_top() + vec2(-2.0, 1.0);
            let n = vec2(-1.8, -1.8);
            ui.painter().add(Shape::closed_line(
                vec![a, b, b + n, a + n],
                Stroke::new(1.4, c),
            ));
        }
        CurveTool::Line => {
            ui.painter()
                .line_segment([r.left_bottom(), r.right_top()], Stroke::new(1.4, c));
            for p in [r.left_bottom(), r.right_top()] {
                ui.painter().circle_filled(p, 2.5, c);
            }
        }
    }
}
fn paint_interpolation(ui: &Ui, r: Rect, c: Color32, mode: &str) {
    let points = (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let y = match mode {
                "linear" => 1.0 - (2.0 * x - 1.0).abs(),
                "cosine" => (1.0 - (x * std::f32::consts::PI).cos()) * 0.5,
                _ => (x * std::f32::consts::PI).sin(),
            };
            pos2(r.left() + r.width() * x, r.bottom() - r.height() * y)
        })
        .collect();
    ui.painter().add(Shape::line(points, Stroke::new(1.3, c)));
}
