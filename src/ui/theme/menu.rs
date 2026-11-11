use super::palette;
use eframe::egui::{self, containers::menu::{MenuButton, MenuConfig}, *};

pub fn config() -> MenuConfig {
    MenuConfig::new().style(style)
}

pub fn popup_style(style: &mut Style) {
    let palette = palette();
    style.visuals.window_fill = palette.popup_background;
    style.visuals.override_text_color = None;
    style.visuals.widgets.noninteractive.fg_stroke.color = palette.popup_text;
    style.visuals.widgets.inactive.fg_stroke.color = palette.popup_text;
    for state in [&mut style.visuals.widgets.hovered, &mut style.visuals.widgets.active, &mut style.visuals.widgets.open] {
        state.bg_fill = palette.popup_highlight;
        state.weak_bg_fill = palette.popup_highlight;
        state.fg_stroke.color = palette.popup_highlight_text;
    }
    style.visuals.selection.bg_fill = palette.popup_highlight;
    style.visuals.selection.stroke.color = palette.popup_highlight_text;
    let font=crate::ui::theme::native_font(if super::current()==super::InterfaceTheme::Classic {17.0}else{18.0});
    style.text_styles.insert(TextStyle::Body,font.clone());
    style.text_styles.insert(TextStyle::Button,font);
}

pub fn style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    popup_style(style);
}

pub fn frame(style: &Style) -> Frame {
    let frame=Frame::popup(style).fill(palette().popup_background);
    if super::current() == super::InterfaceTheme::Classic {frame.corner_radius(0).inner_margin(4)} else {frame}
}

pub fn popup(response: &Response) -> Popup<'_> {
    Popup::menu(response).style(style)
}

pub fn toggle(response: &Response) -> Popup<'_> {
    Popup::from_toggle_button_response(response).style(popup_style)
}

pub fn context_menu(response: &Response, contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>> {
    Popup::context_menu(response).style(style).show(contents)
}

pub fn button<'a, R>(ui: &mut Ui, label: impl IntoAtoms<'a>, contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    if egui::containers::menu::is_in_menu(ui) {
        let size=if super::current()==super::InterfaceTheme::Classic {17.0_f32}else{18.0_f32}*0.6;
        let mut atoms=label.into_atoms();
        atoms.push_right(Atom::custom(Id::new("native-submenu-arrow"),vec2(size.trunc(),size)));
        let (response,inner)=egui::containers::menu::SubMenuButton::from_button(Button::new(atoms)).config(config()).ui(ui,contents);
        let inset=(response.rect.width()/10.0).trunc().min(5.0);
        let x=response.rect.right()-inset-size.trunc();
        let y=response.rect.top()+(response.rect.height()*0.5).trunc();
        let points=[pos2(x,y-size*0.5),pos2(x+size*0.6,y),pos2(x,y+size*0.5)];
        let n0=(points[1]-points[0]).normalized().rot90();
        let n1=(points[2]-points[1]).normalized().rot90();
        let miter=(n0+n1)/(1.0+n0.dot(n1));
        let color=ui.style().interact(&response).text_color();
        for points in [vec![points[0]+n0,points[1]+miter,points[1]-miter,points[0]-n0],
            vec![points[1]+miter,points[2]+n1,points[2]-n1,points[1]-miter]] {
            ui.painter().add(Shape::convex_polygon(points,color,Stroke::NONE));
        }
        return InnerResponse::new(inner.map(|response|response.inner),response);
    }
    let (response, inner) = MenuButton::new(label).config(config()).ui(ui, contents);
    InnerResponse::new(inner.map(|response| response.inner), response)
}
