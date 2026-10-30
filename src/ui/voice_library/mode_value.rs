use eframe::egui::{text::{CCursor, CCursorRange}, *};

#[derive(Clone)]
struct Entry {
    text: String,
    source: [Option<u64>;3],
    frame: u64,
    focus: bool,
}

pub(super) fn show(ui: &mut Ui, rect: Rect, id: Id, source: [Option<f64>;3],
    value: f64, enabled: bool) -> (bool, Option<f64>)
{
    let input_id = id.with("integer-input");
    let frame = ui.ctx().cumulative_frame_nr();
    let bits = source.map(|number| number.map(f64::to_bits));
    let mut entry = ui.ctx().data(|data| data.get_temp::<Entry>(input_id))
        .filter(|entry| enabled && entry.source == bits
            && (entry.frame == frame || entry.frame.checked_add(1) == Some(frame)));
    if entry.is_none() {
        ui.ctx().data_mut(|data| data.remove::<Entry>(input_id));
        let response = ui.interact(rect,id.with("value"),if enabled {Sense::click()} else {Sense::hover()});
        if response.double_clicked() {
            entry = Some(Entry {text:format!("{value:.0}"),source:bits,frame,focus:true});
        }
    }
    let Some(mut entry) = entry else {return (false,None)};
    let mut child = crate::ui::child(ui,rect,id.with("value-editor"));
    let response = child.add_sized(rect.size(), TextEdit::singleline(&mut entry.text)
        .id(input_id).font(FontId::proportional(12.0)).margin(vec2(2.0,0.0))
        .horizontal_align(Align::Center).vertical_align(Align::Center).char_limit(4));
    if entry.focus {
        response.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(),input_id) {
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0),CCursor::new(entry.text.chars().count()))));
            state.store(ui.ctx(),input_id);
        }
        entry.focus = false;
        ui.ctx().request_repaint();
    }
    let escape = ui.input(|input| input.key_pressed(Key::Escape));
    let enter = ui.input(|input| input.key_pressed(Key::Enter));
    if escape {
        response.surrender_focus();
        ui.ctx().data_mut(|data| data.remove::<Entry>(input_id));
        return (true,None);
    }
    if enter || response.lost_focus() {
        if let Ok(number) = entry.text.trim().parse::<i32>() {
            let (low,high)=crate::synthesis::experiments::ExperimentalSettings::ui().style_range();
            if (low..=high).contains(&f64::from(number)) {
                response.surrender_focus();
                ui.ctx().data_mut(|data| data.remove::<Entry>(input_id));
                return (true,Some(f64::from(number)));
            }
        }
        if response.lost_focus() && !enter {
            ui.ctx().data_mut(|data| data.remove::<Entry>(input_id));
            return (true,None);
        }
        response.request_focus();
    }
    entry.frame = frame;
    ui.ctx().data_mut(|data| data.insert_temp(input_id,entry));
    (true,None)
}
