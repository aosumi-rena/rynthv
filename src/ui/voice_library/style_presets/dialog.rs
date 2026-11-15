use super::*;
pub(crate) fn show(app:&mut App,ctx:&Context) {
    let mut state=std::mem::take(&mut app.preset_library);
    let Some(mut naming)=state.naming.take() else {app.preset_library=state;return;};
    let valid=app.voice_selection().is_some_and(|s|s.database_id.to_string()==naming.bank&&s.revision.to_string()==naming.revision)
        && app.editor.voice_preset(&naming.revision).is_ok_and(|s|s.0==naming.reference);
    if !valid {app.preset_library=state;return;}
    let mut close=false;let mut submit=false;
    match crate::ui::prompt::preset_name(ctx,"save-voice-preset",&mut naming.name,&mut naming.focus) {
        Some(true)=>submit=true,Some(false)=>close=true,None=>(),
    }
    if submit {
        let catalogue=app.voice_inventory().and_then(|v|v.configuration.as_ref()).map(|c|c.vocal_mode_names().to_vec()).unwrap_or_default();
        match save(app,&mut state,&naming.bank,&naming.revision,naming.reference,&naming.name,&catalogue) {
            Ok(())=>close=true,Err(error)=>{app.error=Some(error);close=true;},
        }
    }
    if !close {state.naming=Some(naming);}
    app.preset_library=state;
}
