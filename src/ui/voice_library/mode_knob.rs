use eframe::egui::*;

const ARC: Color32 = Color32::from_rgb(71,69,74);
const ACTIVE: Color32 = Color32::from_rgb(239,232,242);
const KNOB: Color32 = Color32::from_rgb(63,60,64);
const CAPTION: Color32 = Color32::from_rgb(163,157,166);

#[derive(Clone)]
struct Draft {
    source: [Option<u64>;3],
    values: [f64;3],
    raw_values: [f64;3],
    frame: u64,
    cancelled: bool,
}

fn arc(painter: &Painter, center: Pos2, radius: f32, value: f64, color: Color32) {
    let (low,high)=crate::synthesis::experiments::ExperimentalSettings::ui().style_range();
    let sweep = ((value-low)/(high-low)).clamp(0.0,1.0) as f32*270.0;
    if sweep == 0.0 {return}
    let steps = (sweep/3.0).ceil() as usize;
    let points: Vec<_> = (0..=steps).map(|step| {
        let angle = (135.0+sweep*step as f32/steps as f32).to_radians();
        center+vec2(angle.cos(),angle.sin())*radius
    }).collect();
    painter.add(Shape::line(points,Stroke::new(2.0,color)));
}

pub(super) fn show(ui: &mut Ui, rect: Rect, id: Id, source: [Option<f64>;3],
    component: Option<usize>, label: &str, snap: bool, unavailable: Option<&str>) -> Option<[f64;3]>
{
    let enabled = unavailable.is_none();
    let (low,high)=crate::synthesis::experiments::ExperimentalSettings::ui().style_range();
    let center = pos2(rect.center().x,rect.top()+if component.is_some() {18.0} else {32.0});
    let caption_rect = Rect::from_center_size(pos2(rect.center().x,
        rect.top()+if component.is_some(){48.0}else{64.0}),vec2(rect.width(),24.0));
    let mut disk_rect = Rect::from_center_size(center,Vec2::splat(if component.is_some(){36.0}else{64.0}));

    disk_rect.max.y = disk_rect.max.y.min(caption_rect.top());
    let response = ui.interact(disk_rect,id,if enabled {Sense::click_and_drag()} else {Sense::hover()});
    let frame = ui.ctx().cumulative_frame_nr();
    let source_bits = source.map(|value| value.map(f64::to_bits));
    let original = source.map(|value| value.unwrap_or(0.0).clamp(low,high));
    let mut explicit_commit = None;
    if response.double_clicked() {
        ui.ctx().data_mut(|data| data.remove::<Draft>(id));
        explicit_commit = Some(std::array::from_fn(|index| {
            if component.is_none_or(|selected| selected == index) {0.0} else {original[index]}
        }));
    }
    let mut pending = ui.ctx().data(|data| data.get_temp::<Draft>(id))
        .filter(|draft| enabled && draft.source == source_bits
            && (draft.frame == frame || draft.frame.checked_add(1) == Some(frame))
            && (ui.ctx().dragged_id() == Some(id) || ui.ctx().drag_stopped_id() == Some(id)));
    if response.dragged() && enabled {
        let mut draft = pending.take().unwrap_or(Draft {source:source_bits,values:original,raw_values:original,frame,cancelled:false});
        draft.cancelled |= ui.input(|input| input.key_pressed(Key::Escape));

        let movement = response.drag_delta();
        let delta = f64::from(movement.x-movement.y)*1.5;
        draft.raw_values = std::array::from_fn(|index| {
            if !draft.cancelled && component.is_none_or(|selected| selected == index) {
                (draft.raw_values[index]+delta).clamp(low,high)
            } else {original[index]}
        });
        if snap && component.is_none() {
            let maximum = draft.raw_values.into_iter().fold(f64::NEG_INFINITY,f64::max);
            let snap_delta = (maximum/5.0).round()*5.0-maximum;
            draft.values = draft.raw_values.map(|value| (value+snap_delta).clamp(low,high));
        } else {
            draft.values = std::array::from_fn(|index| {
                let value = draft.raw_values[index];
                if snap && component == Some(index) {(value/5.0).round()*5.0} else {value}
            });
        }
        draft.frame = frame;
        ui.ctx().data_mut(|data| data.insert_temp(id,draft.clone()));
        pending = Some(draft);
    }
    if let Some(draft) = pending.as_mut() {
        draft.cancelled |= ui.input(|input| input.key_pressed(Key::Escape));
    }
    let mut values = explicit_commit.unwrap_or_else(|| pending.as_ref()
        .filter(|draft| !draft.cancelled).map_or(original,|draft| draft.values));
    let hovered = response.hovered() || response.dragged() || ui.rect_contains_pointer(caption_rect);
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
    let mut value = component.map_or_else(|| values.into_iter().fold(f64::NEG_INFINITY,f64::max),|index| values[index]);
    let (editing, precise) = super::mode_value::show(ui,caption_rect,id,source,value,enabled);
    if let Some(precise) = precise {
        ui.ctx().data_mut(|data| data.remove::<Draft>(id));
        values = std::array::from_fn(|index| {
            if component.is_none() {(original[index]+precise-value).clamp(low,high)}
            else if component == Some(index) {precise} else {original[index]}
        });
        explicit_commit = Some(values);
        value = component.map_or_else(|| values.into_iter().fold(f64::NEG_INFINITY,f64::max),|index| values[index]);
    }
    if component.is_some() {
        arc(&painter,center,16.0,high,ARC);
        arc(&painter,center,16.0,value,ACTIVE);
    } else {

        for (radius,value) in [20.0,25.0,30.0].into_iter().zip(values) {
            arc(&painter,center,radius,high,ARC);
            arc(&painter,center,radius,value,ACTIVE);
        }
    }
    let radius = if component.is_some() {12.0} else {16.0};
    painter.circle(center,radius,KNOB,Stroke::new(0.8,ARC));
    let angle = (135.0+((value-low)/(high-low)) as f32*270.0).to_radians();
    painter.line_segment([center,center+vec2(angle.cos(),angle.sin())*(radius-2.0)],Stroke::new(2.0,ACTIVE));
    let caption = if hovered {format!("{value:.0} %")} else {label.to_owned()};
    if !editing {painter.text(caption_rect.center(),
        Align2::CENTER_CENTER,caption,FontId::proportional(12.0),CAPTION);}
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Slider,enabled,label));
    if let Some(reason) = unavailable {response.clone().on_hover_text(crate::i18n::message(reason));}
    let commit = if explicit_commit.is_some() {explicit_commit}
        else if response.drag_stopped() && pending.as_ref().is_some_and(|draft| !draft.cancelled)
        && values != original {Some(values)} else {None};
    if !response.dragged() {ui.ctx().data_mut(|data| data.remove::<Draft>(id));}
    commit
}
