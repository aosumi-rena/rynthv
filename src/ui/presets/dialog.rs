use super::*;
pub fn show(app:&mut App,ctx:&Context) {
    let mut state=std::mem::take(&mut app.control_presets);
    if let Some(mut naming)=state.naming.take(){
        if naming.target.valid(app){
            match crate::ui::prompt::preset_name(ctx,"save-control-preset",&mut naming.name,&mut naming.focus){
                Some(true)=>if let Err(error)=save(app,&mut state,&naming.target,&naming.name){app.error=Some(error);},
                Some(false)=>(),None=>state.naming=Some(naming),
            }
        }
    }
    app.control_presets=state;
}
