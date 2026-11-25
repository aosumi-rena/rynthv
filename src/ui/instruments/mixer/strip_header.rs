use crate::{
    synthesis::instruments::microphones::Group,
    ui::icons::{Icon, IconButton},
};
use eframe::egui::*;

pub(super) fn show(
    ui: &mut Ui,
    rect: Rect,
    name: &str,
    group: &mut Group,
    linked_channels: bool,
    scale: f32,
) {
    let font_scale = scale / 0.75;
    let mut header = crate::ui::child(ui, rect, ("microphone-heading", name));
    header.spacing_mut().interact_size = Vec2::ZERO;
    header.spacing_mut().button_padding = Vec2::ZERO;
    let gap = 4. * scale;
    let mut right = rect.right();
    for (value, label) in [(&mut group.solo, "S"), (&mut group.mute, "M")] {
        let button = Rect::from_center_size(
            pos2(right - 11.5 * scale, rect.center().y),
            vec2(23., 24.) * scale,
        );
        if header
            .put(
                button,
                Button::new(RichText::new(label).size(12. * font_scale)).selected(*value),
            )
            .clicked()
        {
            *value = !*value;
        }
        right = button.left() - gap;
    }
    if linked_channels {
        let button = Rect::from_center_size(
            pos2(right - 14. * scale, rect.center().y),
            vec2(28., 24.) * scale,
        );
        if header
            .put(
                button,
                IconButton::new(Icon::Link, crate::i18n::t("Link"))
                    .size(button.size())
                    .icon_size(18. * scale)
                    .selected(group.link),
            )
            .clicked()
        {
            group.link = !group.link;
        }
        right = button.left() - gap;
    }
    let title = Rect::from_min_max(rect.min, pos2(right, rect.bottom()));
    let mut label = crate::ui::child(&mut header, title, ("microphone-title", name));
    label.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.add(
            Label::new(
                RichText::new(super::super::native_label(&format!("Mic_{name}")))
                    .size(14. * font_scale)
                    .strong(),
            )
            .truncate(),
        );
    });
}
