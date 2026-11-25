use crate::app::App;
use eframe::egui::{Context, Event, Key, Modifiers};

#[derive(Clone, Copy)]
enum Action {
    Toggle,
    Stop,
    End,
}

fn action(key: Key, modifiers: Modifiers) -> Option<Action> {
    match key {
        Key::Space if modifiers.matches_exact(Modifiers::COMMAND) => Some(Action::Stop),
        Key::Space if modifiers.matches_exact(Modifiers::NONE) => Some(Action::Toggle),
        Key::End if modifiers.matches_exact(Modifiers::COMMAND) => Some(Action::End),
        _ => None,
    }
}

pub fn shortcuts(app: &mut App, ctx: &Context) {
    if !ctx.input(|input| input.focused)
        || ctx.text_edit_focused() || app.lyric.is_some() || app.rocoflex.text_edit_focused(ctx)
    {
        return;
    }
    let actions = ctx.input_mut(|input| {
        let mut actions = Vec::new();
        let mut space = false;
        input.events.retain(|event| {
            if let Event::Key { key, modifiers, pressed, repeat, .. } = event
                && let Some(action) = action(*key, *modifiers)
            {
                space |= *key == Key::Space;
                if *pressed && !*repeat {
                    actions.push(action);
                }
                return false;
            }
            true
        });
        if space {
            input.events.retain(|event| !matches!(event, Event::Text(text) if text == " "));
        }
        actions
    });
    for action in actions {
        match action {
            Action::Toggle => app.editor.transport.toggle(),
            Action::Stop => app.editor.transport.stop(),
            Action::End => app.editor.transport.position = app.editor.project.end(),
        }
    }
}
