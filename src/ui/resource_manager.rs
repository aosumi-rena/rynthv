use super::{
    child,
    icons::{Icon, IconButton},
    theme::*,
};
use eframe::egui::*;
pub const SURFACE: Color32 = Color32::from_rgb(27, 27, 27);
pub const CARD: Color32 = Color32::from_rgb(38, 38, 38);
pub const ACCENT: Color32 = Color32::from_rgb(120, 214, 24);
pub const INK: Color32 = Color32::from_rgb(242, 242, 242);
pub const HEADER_HEIGHT: f32 = 68.;
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Default,
    Name,
    NameReverse,
    Version,
}
impl Sort {
    pub fn label(self) -> &'static str {
        crate::i18n::t(match self {
            Self::Default => "默认",
            Self::Name => "名称 A–Z",
            Self::NameReverse => "名称 Z–A",
            Self::Version => "版本最新",
        })
    }
}
pub fn dialog<T>(
    ctx: &Context,
    id: &str,
    data: &mut T,
    mut header: impl FnMut(&mut T, &mut Ui, Rect) -> bool,
    mut content: impl FnMut(&mut T, &mut Ui),
) -> bool {
    let size = (ctx.content_rect().size() - vec2(32., 40.))
        .min(vec2(1120., 650.))
        .max(vec2(260., 240.));
    let mut close = false;
    let modal = Modal::new(Id::new(id))
        .backdrop_color(Color32::from_black_alpha(105))
        .frame(
            Frame::new()
                .fill(SURFACE)
                .corner_radius(5)
                .stroke(Stroke::new(1., Color32::from_rgb(50, 50, 50))),
        )
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_HEIGHT));
            close = header(data, ui, head);
            ui.painter().hline(
                head.x_range(),
                head.bottom(),
                Stroke::new(1., Color32::from_rgb(20, 20, 20)),
            );
            let margin = (rect.width() * 0.08).clamp(16., 90.);
            let body = Rect::from_min_max(
                pos2(rect.left() + margin, head.bottom() + 26.),
                rect.max - vec2(margin, 16.),
            );
            let mut body = child(ui, body, (id, "content"));
            ScrollArea::vertical()
                .id_salt((id, "scroll"))
                .auto_shrink([false, false])
                .show(&mut body, |ui| {
                    ui.set_width(ui.available_width());
                    content(data, ui);
                });
        });
    close || modal.should_close()
}
pub fn header(
    ui: &mut Ui,
    rect: Rect,
    category: &str,
    close_label: &'static str,
    settings: impl FnOnce(&mut Ui),
) -> bool {
    let main = Rect::from_min_max(
        rect.min + vec2(22., 12.),
        pos2(rect.right() - 18., rect.top() + 54.),
    );
    let mut bar = child(ui, main, "resource-manager-header");
    bar.spacing_mut().interact_size.y = 34.;
    let mut close = false;
    bar.horizontal(|ui| {
        if rect.width() >= 420. {
            let (logo, _) = ui.allocate_exact_size(Vec2::splat(27.), Sense::hover());
            Icon::Brand.paint(ui, logo, Color32::WHITE);
        }
        ui.label(
            RichText::new("RynthV")
                .font(section_title_font())
                .size(18.)
                .color(INK),
        );
        if rect.width() >= 800. {
            ui.label(RichText::new(category).size(13.).color(MUTED));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            close = ui
                .add(
                    IconButton::new(Icon::Close, close_label)
                        .size(Vec2::splat(28.))
                        .icon_size(15.),
                )
                .clicked();
            let resources = ui.add(
                IconButton::new(Icon::Settings, crate::i18n::t("资源目录设置"))
                    .size(Vec2::splat(28.))
                    .icon_size(18.),
            );
            Popup::from_toggle_button_response(&resources)
                .width((ui.ctx().content_rect().width() - 48.).clamp(180., 380.))
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 8.;
                        ScrollArea::vertical()
                            .id_salt("resource-manager-directories")
                            .max_height((ui.ctx().content_rect().height() - 100.).max(120.))
                            .show(ui, settings);
                    });
                });
        });
    });
    close
}
pub fn search(ui: &mut Ui, query: &mut String, width: f32, hint: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 40.), Sense::hover());
    ui.painter()
        .rect_filled(rect, 5, Color32::from_rgb(43, 43, 43));
    let center = rect.left_center() + vec2(19., -1.);
    ui.painter()
        .circle_stroke(center, 5.5, Stroke::new(1.8, TEXT));
    ui.painter().line_segment(
        [center + vec2(4., 4.), center + vec2(9., 9.)],
        Stroke::new(1.8, TEXT),
    );
    let has_text = !query.is_empty();
    let mut input = child(
        ui,
        Rect::from_min_max(
            rect.min + vec2(35., 0.),
            rect.max - vec2(if has_text { 30. } else { 8. }, 0.),
        ),
        ui.id().with("resource-search"),
    );
    input.add_sized(
        input.max_rect().size(),
        TextEdit::singleline(query)
            .frame(Frame::NONE)
            .font(FontId::proportional(14.))
            .hint_text(hint)
            .vertical_align(Align::Center)
            .margin(vec2(0., 9.)),
    );
    if has_text {
        let mut clear = child(
            ui,
            Rect::from_center_size(rect.right_center() - vec2(16., 0.), Vec2::splat(24.)),
            "clear-resource-search",
        );
        if clear
            .add(
                IconButton::new(Icon::Close, crate::i18n::t("清除搜索"))
                    .size(Vec2::splat(24.))
                    .icon_size(12.),
            )
            .clicked()
        {
            query.clear();
        }
    }
}
pub fn product<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .corner_radius(13)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui)
        })
        .inner
}
pub fn toolbar_style(ui: &mut Ui) {
    ui.spacing_mut().interact_size.y = 36.;
    ui.visuals_mut().widgets.inactive.bg_fill = CARD;
    ui.visuals_mut().widgets.inactive.weak_bg_fill = CARD;
    ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(12);
}
pub fn sorting(ui: &mut Ui, sort: &mut Sort) {
    let reverse=ui.layout().main_dir()==Direction::RightToLeft;
    if !reverse {ui.label(RichText::new(crate::i18n::t("排序方式")).color(MUTED));}
    ComboBox::from_id_salt("resource-manager-sort")
        .width(90.)
        .selected_text(sort.label())
        .show_ui(ui, |ui| {
            for s in [Sort::Default, Sort::Name, Sort::NameReverse, Sort::Version] {
                ui.selectable_value(sort, s, s.label());
            }
        });
    if reverse {ui.label(RichText::new(crate::i18n::t("排序方式")).color(MUTED));}
}
pub fn more(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui)) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 13, BORDER);
    }
    for offset in [-5., 0., 5.] {
        ui.painter()
            .circle_filled(rect.center() + vec2(0., offset), 1.5, MUTED);
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    Popup::menu(&response).show(content);
}
pub fn action_button(label: &str, enabled: bool, outline: bool) -> Button<'static> {
    Button::new(RichText::new(label).size(13.).color(if !enabled {
        MUTED
    } else if outline {
        ACCENT
    } else {
        SURFACE
    }))
    .corner_radius(13)
    .fill(if !enabled || outline { CARD } else { ACCENT })
    .stroke(Stroke::new(0.8, if enabled { ACCENT } else { BORDER }))
}
