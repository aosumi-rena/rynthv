use crate::{
    app::App,
    editor::{LyricSelection, split_lyrics},
    ui::theme::*,
};
use eframe::egui::{
    text::{CCursor, CCursorRange},
    *,
};

pub struct FillLyrics {
    selection: LyricSelection,
    text: String,
    repeat: bool,
    by_character: bool,
    focus: bool,
}

pub fn open(app: &mut App) {
    super::tempo_input::finish(app);
    app.commit_lyric();
    app.editor.finish();
    if let Some((selection, text)) = app.editor.lyric_selection() {
        app.fill_lyrics = Some(FillLyrics {
            selection,
            text,
            repeat: false,
            by_character: true,
            focus: true,
        });
    }
}

pub fn show(app: &mut App, ctx: &Context) {
    let Some(mut edit) = app.fill_lyrics.take() else {
        return;
    };
    let width = (ctx.content_rect().width() - 80.0).clamp(200.0, 400.0);
    let input_height = (ctx.content_rect().height() - 320.0).clamp(80.0, 260.0);
    let mut submit = false;
    let mut cancel = false;
    let mut words = Vec::new();
    let modal = Modal::new(Id::new("fill-lyrics"))
        .frame(
            Frame::new()
                .fill(Color32::from_rgb(55, 55, 57))
                .stroke(Stroke::new(1.0, Color32::from_rgb(73, 72, 76)))
                .corner_radius(4)
                .inner_margin(24),
        )
        .backdrop_color(Color32::from_black_alpha(30))
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 12.0;
            ui.label(
                RichText::new(crate::i18n::t("填入歌词"))
                    .font(FontId::new(17.0, FontFamily::Name("SectionTitle".into()))),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(crate::i18n::t("用空格隔开字词。\n每个音符对应一个字词。")).color(MUTED));
            let input_id = Id::new("fill-lyrics-text");
            if edit.focus {
                TextEdit::store_state(ctx, input_id, Default::default());
            }
            Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, MUTED))
                .corner_radius(4)
                .inner_margin(5)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("fill-lyrics-scroll")
                        .max_height(input_height)
                        .min_scrolled_height(input_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let input = ui.add(
                                TextEdit::multiline(&mut edit.text)
                                    .id(input_id)
                                    .desired_width(f32::INFINITY)
                                    .min_size(vec2(0.0, input_height))
                                    .frame(Frame::NONE)
                                    .margin(0.0),
                            );
                            if edit.focus {
                                input.request_focus();
                                if let Some(mut state) = TextEdit::load_state(ctx, input.id) {
                                    state.cursor.set_char_range(Some(CCursorRange::two(
                                        CCursor::new(0),
                                        CCursor::new(edit.text.chars().count()),
                                    )));
                                    state.store(ctx, input.id);
                                }
                                edit.focus = false;
                                ctx.request_repaint();
                            }
                        });
                });
            ui.checkbox(&mut edit.repeat, crate::i18n::t("循环填入"));
            ui.checkbox(&mut edit.by_character, crate::i18n::t("逐字填入"));
            words = split_lyrics(&edit.text, edit.by_character);
            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = vec2(106.0, 34.0);
                submit = ui
                    .add_enabled(
                        !words.is_empty(),
                        Button::new(RichText::new(crate::i18n::t("确定")).color(CANVAS))
                            .fill(GREEN)
                            .min_size(size),
                    )
                    .on_hover_text(crate::ui::chrome::shortcut_text(crate::i18n::t("Ctrl+Enter 确定")))
                    .clicked();
                cancel = ui
                    .add_sized(size, Button::new(crate::i18n::t("取消")).fill(BUTTON_HOVER))
                    .clicked();
            });
        });
    if modal.is_top_modal && !modal.any_popup_open {
        cancel |= ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        submit |= ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
    }
    if cancel {
        ctx.memory_mut(|m| m.surrender_focus(Id::new("fill-lyrics-text")));
        return;
    }
    if submit && !words.is_empty() {
        let changed = app.editor.fill_lyrics(&edit.selection, &words, edit.repeat);
        app.status = crate::lformat!("已更新 {changed} 个音符的歌词");
        ctx.memory_mut(|m| m.surrender_focus(Id::new("fill-lyrics-text")));
    } else {
        app.fill_lyrics = Some(edit);
    }
}
