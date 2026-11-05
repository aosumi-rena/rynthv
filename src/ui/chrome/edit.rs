use crate::{
    app::App,
    ui::{lanes, phonemes, piano},
};
use eframe::egui::{Context, Event, ViewportCommand};

fn anchors(app: &App) -> bool {
    app.piano_tool == piano::Tool::PitchAnchor
}

pub(crate) fn request_group_paste(app:&mut App,ctx:&Context,track:crate::model::EntityId,at:i64,linked:bool){
    app.commit_lyric();app.editor.select_track(track);app.editor.group_focus=true;
    ctx.data_mut(|d|d.insert_temp(eframe::egui::Id::new("arrangement-clipboard-target"),(track,at,linked)));
    ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
}

pub(super) fn selected(app: &App) -> bool {
    if app.editor.track_focus{return !app.editor.selected_tracks.is_empty();}
    if app.editor.group_focus{return !app.editor.selected_groups.is_empty();}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        false
    } else if lanes::parameter_active(app) {
        lanes::parameter_has_selection(app)
    } else if anchors(app) {
        piano::anchor_has_selection(app)
    } else {
        !app.editor.selected.is_empty()
    }
}

pub(crate) fn can_paste(app: &App) -> bool {
    if !rynthv::clipboard::can_paste(){return false;}
    if app.editor.track_focus{return false;}
    if app.editor.group_focus{return !app.editor.is_editing();}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        false
    } else if lanes::parameter_active(app) {
        lanes::parameter_can_paste(app)
    } else if anchors(app) {
        app.editor.group().is_some() && !app.editor.is_editing()
    } else {
        !app.editor.is_editing()
    }
}

pub(super) fn select_all(app: &mut App) {
    if app.editor.track_focus {app.editor.selected_tracks=app.editor.project.tracks.iter().map(|t|t.id).collect();return;}
    if app.editor.group_focus{app.editor.select_all_groups();return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        return;
    }
    if lanes::parameter_active(app) {
        lanes::parameter_select_all(app);
    } else if anchors(app) {
        piano::anchor_select_all(app);
    } else {
        app.editor.select_all_notes();
    }
}

pub(super) fn clear(app: &mut App) {
    if app.editor.track_focus {app.editor.selected_tracks.clear();return;}
    if app.editor.group_focus{app.editor.selected_groups.clear();return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        phonemes::phoneme_deactivate(app);
    } else if lanes::parameter_active(app) {
        lanes::parameter_clear(app);
    } else if anchors(app) {
        piano::anchor_clear(app);
    } else {
        app.editor.selected.clear();
    }
}

pub(super) fn delete(app: &mut App) {
    if app.editor.track_focus {app.editor.delete_tracks();return;}
    if app.editor.group_focus{app.editor.delete_clipboard_groups();return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        return;
    }
    if lanes::parameter_active(app) {
        lanes::parameter_delete(app);
    } else if anchors(app) {
        piano::anchor_delete(app);
    } else {
        app.editor.delete_selected();
    }
}

pub(crate) fn copy(app: &mut App, ctx: &Context) {
    if app.editor.track_focus{return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        return;
    }
    if app.editor.group_focus {copy_notes(app,ctx);}
    else if lanes::parameter_active(app) {
        lanes::parameter_copy(app, ctx);
    } else if anchors(app) {
        piano::anchor_copy(app,ctx);
    } else {
        copy_notes(app, ctx);
    }
}

pub(crate) fn cut(app: &mut App, ctx: &Context) {
    if app.editor.track_focus{return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        return;
    }
    if app.editor.group_focus {if copy_notes(app,ctx){app.editor.delete_clipboard_groups();}}
    else if lanes::parameter_active(app) {
        lanes::parameter_cut(app, ctx);
    } else if anchors(app) {
        piano::anchor_cut(app,ctx);
    } else if copy_notes(app, ctx) {
        if app.editor.group_focus{app.editor.delete_clipboard_groups();}else{app.editor.delete_selected();}
    }
}

pub(crate) fn paste(app: &mut App, ctx: &Context) {
    if app.editor.group_focus && can_paste(app){ctx.send_viewport_cmd(ViewportCommand::RequestPaste);return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        return;
    }
    if lanes::parameter_active(app) {
        lanes::parameter_paste(app, ctx);
    } else if anchors(app) {
        ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
    } else if can_paste(app) {

        ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
    }
}

fn copy_notes(app: &mut App, ctx: &Context) -> bool {
    match app.editor.copy(app.preferences.sync_note_parameters) {
        Ok(Some(text)) => {
            ctx.copy_text(text);
            app.status = crate::i18n::t("已复制音符").into();
            true
        }
        Ok(None) => false,
        Err(error) => {
            app.status = error;
            false
        }
    }
}

pub(super) fn clipboard_events(app: &mut App, ctx: &Context) -> bool {
    let events = ctx.input_mut(|input| {
        let mut commands = Vec::new();
        input.events.retain(|event| {
            if matches!(event, Event::Copy | Event::Cut | Event::Paste(_)) {
                commands.push(event.clone());
                false
            } else {
                true
            }
        });
        commands
    });
    let handled = !events.is_empty();
    if app.editor.track_focus || (!app.editor.group_focus && phonemes::phoneme_active(app)) {
        return handled;
    }
    for event in events {
        match event {
            Event::Copy => copy(app, ctx),
            Event::Cut => cut(app, ctx),
            Event::Paste(text) => {
                let target=ctx.data_mut(|d|d.remove_temp::<(crate::model::EntityId,i64,bool)>(eframe::egui::Id::new("arrangement-clipboard-target")));
                if !text.starts_with("SVR2CLIP"){continue;}
                if let Some((track,at,linked))=target {
                    let result=app.editor.paste_arrangement_text(&text,track,at,linked);report_paste(app,result);continue;
                }
                if !app.editor.group_focus && lanes::parameter_active(app) {
                    lanes::parameter_paste_text(app, &text);
                } else if anchors(app) && !app.editor.group_focus {
                    piano::anchor_paste_text(app,&text);
                } else {
                    let at = app.editor.transport.position;
                    let result = app.paste_text(&text, at);
                    report_paste(app, result);
                }
            }
            _ => unreachable!(),
        }
    }
    handled
}

pub(super) fn duplicate(app: &mut App) {
    if app.editor.track_focus {app.editor.duplicate_tracks(false);return;}
    if !app.editor.group_focus && phonemes::phoneme_active(app) {
        return;
    }
    if lanes::parameter_active(app) {
        lanes::parameter_duplicate(app);
        return;
    }
    let result = if app.editor.group_focus{app.editor.duplicate_clipboard_groups()}else{app.editor.duplicate_selected(app.preferences.sync_note_parameters)};
    report_paste(app, result);
}

fn report_paste(app: &mut App, result: Result<usize, String>) {
    match result {
        Ok(0) => {}
        Ok(_) => app.status = crate::i18n::t("粘贴").into(),
        Err(error) => app.status = error,
    }
}

pub(super) fn transpose(app: &mut App, up: bool) {
    if app.editor.track_focus{return;}
    if lanes::parameter_active(app) || phonemes::phoneme_active(app) {
        return;
    }
    if anchors(app) {
        piano::anchor_shift_octave(app, up);
    } else {
        app.editor.transpose(if up { 12 } else { -12 });
    }
}

pub(super) fn can_transpose(app: &App) -> bool {
    !app.editor.track_focus && !lanes::parameter_active(app) && !phonemes::phoneme_active(app) && selected(app)
}
