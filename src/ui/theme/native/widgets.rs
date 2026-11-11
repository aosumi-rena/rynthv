use super::*;
use crate::ui::icons::Icon;
use eframe::egui::widget_style::{CheckboxStyle, Classes};

pub fn combo(id: impl std::hash::Hash + std::fmt::Debug) -> ComboBox {
    ComboBox::from_id_salt(id).icon(|ui,_rect,_visuals,_open| {
        let mut outer=ui.min_rect().expand2(ui.spacing().button_padding);
        outer.set_height(outer.height().max(ui.spacing().interact_size.y));
        let classic=current()==InterfaceTheme::Classic;
        let icon=if classic {
            Rect::from_min_size(pos2(outer.right()-20.0,outer.top()+(outer.height()*0.5).trunc()-3.0),vec2(12.0,7.0))
        } else {
            Rect::from_min_size(pos2(outer.left()+(outer.width()-24.0).max(0.0)+2.5,outer.center().y-2.5),vec2(7.0,4.5))
        };
        let tint=if classic {Color32::from_rgb(151,151,151)} else {Color32::from_rgba_unmultiplied(135,135,135,if ui.is_enabled(){230}else{51})};
        Icon::Dropdown.paint(ui,icon,tint);
    }).popup_style(crate::ui::theme::menu::style.into())
}

pub fn settings_combo(id: impl std::hash::Hash + std::fmt::Debug) -> ComboBox {
    ComboBox::from_id_salt(id).icon(|ui,_rect,visuals,_open| {
        let mut outer=ui.min_rect().expand2(ui.spacing().button_padding);
        outer.set_height(outer.height().max(ui.spacing().interact_size.y));
        let icon=Rect::from_min_size(pos2(outer.right()-22.0,outer.center().y-3.0),vec2(10.0,6.0));
        Icon::SettingsDropdown.paint(ui,icon,visuals.text_color());
    }).popup_style(settings_popup_style.into())
}

fn settings_popup_style(style: &mut Style) {
    crate::ui::theme::menu::style(style);
    style.override_font_id=None;
    for text in [TextStyle::Body, TextStyle::Button, TextStyle::Small] {
        style.text_styles.insert(text, FontId::new(13.0, FontFamily::Name("NativeSettings".into())));
    }
}

pub struct Checkbox<'a> {
    checked: &'a mut bool,
    atoms: Atoms<'a>,
    indeterminate: bool,
    settings: bool,
}
impl<'a> Checkbox<'a> {
    pub fn new(checked: &'a mut bool, atoms: impl IntoAtoms<'a>) -> Self {
        Self{checked,atoms:atoms.into_atoms(),indeterminate:false,settings:false}
    }
    pub fn indeterminate(mut self, value: bool) -> Self {self.indeterminate=value;self}
    pub fn settings(mut self) -> Self {self.settings=true;self}
}

pub fn checkbox<'a>(ui: &mut Ui, checked: &'a mut bool, atoms: impl IntoAtoms<'a>) -> Response {
    ui.add(Checkbox::new(checked,atoms))
}
pub fn settings_checkbox<'a>(ui: &mut Ui, checked: &'a mut bool, atoms: impl IntoAtoms<'a>) -> Response {
    ui.add(Checkbox::new(checked,atoms).settings())
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self{checked,mut atoms,indeterminate,settings}=self;
        let id=ui.next_auto_id();
        let state=ui.ctx().read_response(id).map(|r|r.widget_state()).unwrap_or_default();
        let CheckboxStyle{frame,text_style,..}=ui.style().checkbox_style(&Classes::default(),state);
        let classic=current()==InterfaceTheme::Classic;
        let menu=eframe::egui::containers::menu::is_in_menu(ui);
        let box_size=if settings {28.0} else if menu && !classic {18.0} else {14.0};
        let min_size=vec2(ui.spacing().interact_size.y,ui.spacing().interact_size.y.max(box_size));
        let rect_id=Id::new("native-checkbox-icon");
        atoms.push_left(Atom::custom(rect_id,vec2(box_size,min_size.y)));
        let text=atoms.text().map(String::from);
        let spacing=ui.spacing().item_spacing.x;ui.spacing_mut().item_spacing.x=5.0;
        let font=ui.style().override_font_id.clone();
        if !settings && !menu {ui.style_mut().override_font_id=Some(crate::ui::theme::native_font(if classic {16.0}else{18.0}));}
        let mut prepared=AtomLayout::new(atoms).sense(Sense::click()).min_size(min_size).frame(frame).allocate(ui);
        ui.style_mut().override_font_id=font;
        ui.spacing_mut().item_spacing.x=spacing;
        if prepared.response.clicked(){*checked=!*checked;prepared.response.mark_changed();}
        prepared.response.widget_info(||if indeterminate {
            WidgetInfo::labeled(WidgetType::Checkbox,ui.is_enabled(),text.as_deref().unwrap_or(""))
        } else {WidgetInfo::selected(WidgetType::Checkbox,ui.is_enabled(),*checked,text.as_deref().unwrap_or(""))});
        if !ui.is_rect_visible(prepared.response.rect){return prepared.response;}
        prepared.fallback_text_color=if settings || menu {text_style.color} else if classic {Color32::from_rgba_unmultiplied(151,151,151,if ui.is_enabled(){255}else{128})} else {
            if !ui.is_enabled(){Color32::from_rgb(113,108,115)}else{Color32::from_rgb(163,157,166)}
        };
        let opacity=ui.opacity();
        if !settings && !menu {ui.set_opacity(1.0);}
        let painted=prepared.paint(ui);
        if let Some(rect)=painted.rect(rect_id) {
            let icon=Rect::from_center_size(pos2(rect.left()+box_size*0.5,rect.center().y),Vec2::splat(box_size));
            let icon=if menu && !classic {Rect::from_center_size(icon.center(),vec2(18.0,15.0))}else{icon};
            let painter=ui.ctx().layer_painter(ui.layer_id()).with_clip_rect(ui.clip_rect());
            let native_color=if ui.is_enabled(){Color32::from_rgb(239,232,242)}else{Color32::from_rgb(113,108,115)};
            if !menu {
                if settings {
                    ui.painter().rect(icon,4,Color32::from_white_alpha(18),Stroke::new(0.5,Color32::from_white_alpha(18)),StrokeKind::Inside);
                } else if classic {
                    let outer=Color32::from_rgba_unmultiplied(151,151,151,if *checked||indeterminate {255} else {128});
                    painter.rect_filled(icon,2,outer);
                    if !*checked && !indeterminate {painter.rect_filled(icon.shrink(1.0),2,Color32::from_rgb(54,55,55));}
                } else if indeterminate {
                    painter.rect_filled(icon,2,native_color);
                } else if !*checked {
                    let color=if ui.is_enabled(){Color32::from_rgb(163,157,166)}else{native_color};
                    let pixel=1.0/ui.ctx().pixels_per_point();
                    painter.rect_stroke(icon.shrink(pixel*0.5),2,Stroke::new(pixel,color),StrokeKind::Middle);
                }
            }
            if indeterminate {
                ui.painter().hline(icon.shrink(3.0).x_range(),icon.center().y,Stroke::new(1.0,if settings||menu {text_color()} else {panel_color()}));
            } else if *checked {
                let tint=if settings {Color32::WHITE} else if menu {ui.style().interact(&painted.response).text_color()} else if classic {Color32::from_rgb(54,55,55)}else{native_color};
                if menu && classic {
                    for points in classic_menu_tick(painted.response.rect,1.0) {
                        ui.painter().add(Shape::convex_polygon(points.to_vec(),tint,Stroke::NONE));
                    }
                } else {
                    let icon=if settings {Rect::from_center_size(icon.center(),Vec2::splat(16.0))}else{icon};
                    (if settings {Icon::SettingsCheck} else if menu {Icon::MenuCheck} else {Icon::Check}).paint(ui,icon,tint);
                }
            }
        }
        if !settings && !menu {ui.set_opacity(opacity);}
        painted.response
    }
}


pub fn collapsing(text: impl Into<WidgetText>) -> CollapsingHeader {
    CollapsingHeader::new(text).icon(|ui,open,response| {
        let size=if open>0.5 {vec2(7.0,4.5)}else{vec2(4.5,7.0)};
        let icon=Rect::from_center_size(response.rect.center(),size);
        (if open>0.5 {Icon::ChevronDown}else{Icon::ChevronRight}).paint(ui,icon,property_label_color());
    })
}

fn classic_menu_tick(item: Rect, scale: f32) -> [[Pos2; 4]; 2] {
    let width=item.width();
    let height=(item.height()-2.0).max(0.0);
    let inset=(width/10.0).trunc().min(5.0);
    let available=(width-2.0-2.0*inset).max(0.0);
    let slot_width=((height/1.2999999523162842_f32)*scale).round_ties_even().min(available);
    let q=((height as f64)/6.0) as f32;
    let rect=Rect::from_min_size(pos2(item.left()+inset,item.top()+1.0+q),vec2(slot_width,(height-2.0*q).max(0.0)));
    let point=|x:f64,y:f64| rect.min+vec2((rect.width() as f64*x) as f32,(rect.height() as f64*y) as f32);
    let points=[point(0.2,0.5),point(0.4,0.8),point(0.8,0.2)];
    let segment=|a:Pos2,b:Pos2| {
        let delta=b-a;
        let length=delta.x.hypot(delta.y);
        let normal=if length>0.0 {vec2(-delta.y,delta.x)*(0.5/length)}else{Vec2::ZERO};
        [a+normal,a-normal,b-normal,b+normal]
    };
    [segment(points[0],points[1]),segment(points[1],points[2])]
}
