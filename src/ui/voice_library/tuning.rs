use crate::{app::App,synthesis::parameters::TUNING_CONTROLS,ui::theme::*};
use eframe::egui::*;
pub(super) fn show(app:&mut App,ui:&mut Ui) {
    let cents=app.editor.voice_parameter(TUNING_CONTROLS[0]).map_or(0.0,|(_,v)|v);
    let semitones=app.editor.voice_parameter(TUNING_CONTROLS[1]).map_or(0.0,|(_,v)|v);
    let hz=440.0_f64*2.0_f64.powf((f64::from(semitones)+f64::from(cents)/100.0)/12.0);
    let mut frequency=format!("{hz:.3}");
    while frequency.ends_with('0'){frequency.pop();}
    if frequency.ends_with('.') {frequency.pop();}
    let names=["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
    let key=names[(semitones.round() as i32).rem_euclid(12) as usize];
    let (rect,_)=ui.allocate_exact_size(vec2(ui.available_width(),28.0),Sense::hover());
    ui.painter().rect_filled(rect,4,Color32::from_rgb(58,56,59));
    ui.painter().text(rect.left_center()+vec2(5.0,0.0),Align2::LEFT_CENTER,
        crate::lformat!("调性: {key}，A4 = {frequency} Hz"),FontId::proportional(12.0),TEXT);
    ui.add_space(10.0);
    ui.spacing_mut().item_spacing.y=0.0;
    for spec in TUNING_CONTROLS {super::parameters::control(app,ui,spec);}
}
