use super::theme::*;
use eframe::egui::*;
pub(super) fn show(ui:&mut Ui,area:Rect,rendering:bool) {
    if !rendering || area.width()<140.0 || area.height()<30.0 {return;}
    let rect=Rect::from_center_size(area.center(),vec2(124.0,26.0));
    let mut overlay=ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    Frame::new().fill(BUTTON).corner_radius(3).inner_margin(vec2(12.0,3.0)).show(&mut overlay,|ui| {
        ui.add(Spinner::new().size(16.0).color(TEXT));
        ui.label(RichText::new(crate::i18n::t("渲染歌声")).color(TEXT));
    });
}
