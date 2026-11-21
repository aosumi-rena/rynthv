use super::theme::*;
use eframe::egui::*;

#[derive(Clone, Copy)]
pub enum Choice {
    Cancel,
    Discard,
    Save,
}

pub(super) fn action_button(
    ui: &mut Ui,
    button: Button<'_>,
    size: Vec2,
    enabled: bool,
    fill: Color32,
) -> Response {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        for (visuals, color) in [
            (&mut widgets.inactive, fill),
            (&mut widgets.hovered, fill.lerp_to_gamma(Color32::WHITE, 0.16)),
            (&mut widgets.active, fill.lerp_to_gamma(Color32::BLACK, 0.16)),
        ] {
            visuals.bg_fill = color;
            visuals.weak_bg_fill = color;
        }
        if !enabled {
            ui.disable();
        }
        ui.add_sized(size, button)
    }).inner
}

pub fn preset_name(ctx:&Context,id:&str,text:&mut String,focus:&mut bool)->Option<bool> {
    let mut answer=None;
    let response=Modal::new(Id::new(id))
        .frame(Frame::new().fill(Color32::from_rgb(61,59,64)).corner_radius(4).inner_margin(24))
        .show(ctx,|ui|{
            ui.set_width(340.);ui.spacing_mut().item_spacing.y=12.;
            ui.label(RichText::new(crate::i18n::t("保存为新的预设")).font(section_title_font()));
            ui.label(crate::i18n::t("预设名称"));
            let input=ui.add_sized(vec2(ui.available_width(),28.),TextEdit::singleline(text));
            if *focus {
                input.request_focus();
                if let Some(mut state)=TextEdit::load_state(ctx,input.id){
                    state.cursor.set_char_range(Some(text::CCursorRange::two(text::CCursor::new(0),text::CCursor::new(text.chars().count()))));
                    state.store(ctx,input.id);
                }
                *focus=false;
            }
            ui.add_space(8.);
            let valid=!text.trim().is_empty()&&!text.chars().any(char::is_control);
            ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
                let accent = accent(ui);
                let confirm = Button::new(RichText::new(crate::i18n::t("确定")).color(CANVAS)).corner_radius(4);
                if action_button(ui, confirm, vec2(80., 28.), valid, accent).clicked(){answer=Some(true);}
                if ui.add_sized(vec2(80.,28.),Button::new(crate::i18n::t("取消"))).clicked(){answer=Some(false);}
            });
            if valid&&ui.input(|i|i.key_pressed(Key::Enter)){answer=Some(true);}
        });
    if (response.is_top_modal && !response.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)))&&answer.is_none(){answer=Some(false);}
    answer
}

pub fn unsaved(ctx: &Context, title: &str) -> Option<Choice> {
    choices(
        ctx,
        "unsaved",
        title,
        crate::i18n::t("是否放弃未保存的更改？"),
        [
            crate::i18n::t("取消"),
            crate::i18n::t("放弃更改"),
            crate::i18n::t("保存"),
        ],
        true,
    )
    .map(|i| [Choice::Cancel, Choice::Discard, Choice::Save][i])
}

pub fn choices(
    ctx: &Context,
    id: &str,
    title: &str,
    message: &str,
    labels: [&str; 3],
    destructive_middle: bool,
) -> Option<usize> {
    let mut choice = None;
    let response = Modal::new(Id::new(id))
        .frame(
            Frame::NONE
                .fill(Color32::from_rgb(61, 59, 64))
                .corner_radius(4)
                .stroke(Stroke::new(0.5, MUTED)),
        )
        .show(ctx, |ui| {
            let font = FontId::new(14.0, FontFamily::Name("SectionTitle".into()));
            let widths = labels.map(|s| {
                (ui.painter()
                    .layout_no_wrap(s.to_owned(), font.clone(), TEXT)
                    .size()
                    .x
                    + 24.0)
                    .max(100.0)
            });
            let width = (widths.iter().sum::<f32>() + 120.0).max(420.0);
            let (r, _) = ui.allocate_exact_size(vec2(width, 172.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(
                Rect::from_min_size(r.min, vec2(40.0, 172.0)),
                CornerRadius {
                    nw: 4,
                    ne: 0,
                    sw: 4,
                    se: 0,
                },
                Color32::from_rgb(48, 70, 72),
            );
            p.text(
                r.min + vec2(20.0, 86.0),
                Align2::CENTER_CENTER,
                "?",
                FontId::proportional(29.0),
                Color32::from_rgb(44, 157, 166),
            );
            p.text(
                r.min + vec2(64.0, 35.0),
                Align2::LEFT_CENTER,
                title,
                FontId::new(16.0, FontFamily::Name("SectionTitle".into())),
                TEXT,
            );
            let body = Rect::from_min_max(r.min + vec2(64.0, 52.0), r.max - vec2(24.0, 64.0));
            let mut text = super::child(ui, body, (id, "message"));
            text.add(Label::new(RichText::new(message).size(12.0).color(MUTED)).wrap());
            let mut x = r.right() - 24.0 - widths.iter().sum::<f32>() - 16.0;
            for (i, label) in labels.into_iter().enumerate() {
                let rect = Rect::from_min_size(pos2(x, r.top() + 116.0), vec2(widths[i], 32.0));
                x += widths[i] + 8.0;
                let mut button = super::child(ui, rect, (id, "choice", i));
                let color = if i == 2 {
                    Color32::from_rgb(30, 30, 30)
                } else if i == 1 && destructive_middle {
                    Color32::from_rgb(255, 91, 38)
                } else {
                    TEXT
                };
                let fill = if i == 2 {
                    accent(&button)
                } else {
                    BUTTON
                };
                let action = Button::new(RichText::new(label).font(font.clone()).color(color)).corner_radius(4);
                if action_button(&mut button, action, rect.size(), true, fill).clicked() {
                    choice = Some(i);
                }
            }
        });
    if response.is_top_modal && !response.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        choice = Some(0);
    }
    choice
}
pub fn error(ctx: &Context, message: &str) -> bool {
    notice(ctx, crate::i18n::t("操作未完成"), message)
}

pub fn notice(ctx: &Context, title: &str, message: &str) -> bool {
    let message = crate::i18n::message(message);
    let mut accepted = false;
    let response = Modal::new(Id::new("error"))
        .frame(
            Frame::NONE
                .fill(Color32::from_rgb(61, 59, 64))
                .corner_radius(4)
                .stroke(Stroke::new(0.5, MUTED)),
        )
        .show(ctx, |ui| {
            let text =
                ui.painter()
                    .layout(message.into(), FontId::proportional(12.0), MUTED, 336.0);
            let height = (text.size().y + 130.0).max(172.0);
            let (r, _) = ui.allocate_exact_size(vec2(420.0, height), Sense::hover());
            let p = ui.painter();
            p.rect_filled(
                Rect::from_min_size(r.min, vec2(40.0, height)),
                CornerRadius {
                    nw: 4,
                    ne: 0,
                    sw: 4,
                    se: 0,
                },
                Color32::from_rgb(115, 66, 49),
            );
            p.text(
                pos2(r.left() + 20.0, r.center().y),
                Align2::CENTER_CENTER,
                "!",
                FontId::proportional(29.0),
                Color32::from_rgb(255, 91, 38),
            );
            p.text(
                r.min + vec2(64.0, 35.0),
                Align2::LEFT_CENTER,
                title,
                FontId::new(16.0, FontFamily::Name("SectionTitle".into())),
                TEXT,
            );
            p.galley(r.min + vec2(64.0, 62.0), text, MUTED);
            let rect =
                Rect::from_min_size(pos2(r.left() + 296.0, r.bottom() - 56.0), vec2(100.0, 32.0));
            let mut button = super::child(ui, rect, "error-acknowledgement");
            let fill = accent(&button);
            let action = Button::new(
                RichText::new(crate::i18n::t("确定"))
                    .font(FontId::new(14.0, FontFamily::Name("SectionTitle".into())))
                    .color(CANVAS),
            ).corner_radius(4);
            accepted = action_button(&mut button, action, rect.size(), true, fill).clicked();
        });
    accepted || (response.is_top_modal && !response.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)))
}
