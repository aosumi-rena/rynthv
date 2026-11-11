


use super::*;
use crate::ui::{child, icons::Icon};

pub fn control_height() -> f32 { if current() == InterfaceTheme::Classic {24.0} else {28.0} }
pub fn label_font() -> FontId { crate::ui::theme::native_font(if current() == InterfaceTheme::Classic {16.0} else {18.0}) }
pub fn panel_title_color() -> Color32 { Color32::from_rgb(232,232,232) }
pub fn property_label_color() -> Color32 {
    if current() == InterfaceTheme::Classic {Color32::from_rgb(148,148,148)}
    else {Color32::from_rgb(163,163,163)}
}
pub fn property_value_color() -> Color32 {
    if current() == InterfaceTheme::Classic {Color32::from_rgb(189,189,189)}
    else {Color32::from_rgb(239,232,242)}
}
pub fn property_list_fill() -> Color32 {
    if current() == InterfaceTheme::Classic {Color32::from_rgb(45,45,45)}
    else {Color32::from_rgb(31,27,30)}
}
pub fn panel_border() -> Color32 {
    if current() == InterfaceTheme::Classic {Color32::from_rgb(45,45,45)}
    else {Color32::from_black_alpha(64)}
}
pub fn section_divider() -> Color32 {
    if current() == InterfaceTheme::Classic {Color32::from_rgb(45,45,45)}
    else {Color32::from_black_alpha(102)}
}
pub fn expression_fill() -> Color32 {
    if current() == InterfaceTheme::Classic {background_color()}
    else {Color32::from_rgb(27,26,28)}
}
pub fn expression_ink() -> Color32 {
    if current() == InterfaceTheme::Classic {text_color()}
    else {Color32::from_rgb(239,232,242)}
}
pub fn expression_caption() -> Color32 {
    if current() == InterfaceTheme::Classic {muted_color()}
    else {Color32::from_rgb(163,157,166)}
}
pub fn phoneme_gutter() -> Color32 {
    if current() == InterfaceTheme::Classic {Color32::from_rgba_unmultiplied(47,47,47,205)}
    else {Color32::from_rgb(45,43,46)}
}

pub fn label(ui: &mut Ui, text: &str) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).text(
        rect.left_center(), Align2::LEFT_CENTER, text, label_font(), property_label_color(),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), text));
}



pub fn section(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, title: &str,
    header_action: impl FnOnce(&mut Ui), content: impl FnOnce(&mut Ui)) {
    let id=ui.make_persistent_id(id);
    let mut open=ui.ctx().data_mut(|d|d.get_temp::<bool>(id).unwrap_or(true));
    let top=ui.next_widget_position();
    ui.painter().hline(ui.available_rect_before_wrap().x_range(),top.y,Stroke::new(1.0,section_divider()));
    let (header,_) = ui.allocate_exact_size(vec2(ui.available_width(),30.0),Sense::hover());
    let header=Rect::from_min_max(header.min+vec2(20.0,0.0),header.max-vec2(20.0,0.0));
    let button=ui.interact(header,id.with("title"),Sense::click());
    if button.clicked(){open=!open;ui.ctx().data_mut(|d|d.insert_temp(id,open));}
    let classic=current()==InterfaceTheme::Classic;
    if !classic {
        let glyph=if open {
            Rect::from_min_size(header.min+vec2(2.5,17.5),vec2(7.0,4.5))
        }else{
            Rect::from_min_size(header.min+vec2(3.75,16.0),vec2(4.5,7.0))
        };
        (if open {Icon::ChevronDown}else{Icon::ChevronRight}).paint(ui,glyph,Color32::from_rgb(163,157,166));
    }
    let ink=if classic {
        Color32::from_gray(if button.hovered(){207}else{189})
    }else if button.hovered(){Color32::from_gray(238)}else{Color32::from_rgb(239,232,242)};
    let caption=if classic {header.min+vec2(0.0,13.0)}else{header.min+vec2(20.0,14.0)};
    ui.painter().text(caption,Align2::LEFT_CENTER,title,crate::ui::theme::native_font(if classic {18.0}else{17.0}),ink);
    let mut action=child(ui,Rect::from_min_max(pos2((header.right()-32.0).max(header.left()),header.top()),header.max),id.with("action"));
    header_action(&mut action);
    if open {
        ui.add_space(8.0);
        Frame::new().inner_margin(Margin{left:20,right:20,top:0,bottom:24}).show(ui,|ui|{
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y=12.0;
            ui.spacing_mut().interact_size.y=control_height();
            content(ui);
        });
    }
}

mod widgets;
pub use widgets::{Checkbox,checkbox,settings_checkbox,combo,settings_combo,collapsing};
