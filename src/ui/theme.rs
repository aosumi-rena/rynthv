use eframe::egui::*;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
mod fonts;
mod palette;
mod metrics;
pub mod menu;
pub mod native;
pub use fonts::install as fonts;
pub use palette::Palette;
pub use metrics::Metrics;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceTheme {
    #[default]
    Modern,
    Classic,
}

impl InterfaceTheme {
    pub const ALL: [Self; 2] = [Self::Modern, Self::Classic];

    pub fn label(self) -> &'static str {
        crate::i18n::t(match self {
            Self::Modern => "UI_THEME_MODERN",
            Self::Classic => "UI_THEME_CLASSIC",
        })
    }

    pub fn metrics(self) -> &'static Metrics {
        match self {
            Self::Modern => &metrics::MODERN,
            Self::Classic => &metrics::CLASSIC,
        }
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Modern => &palette::MODERN,
            Self::Classic => &palette::CLASSIC,
        }
    }
}

thread_local! {
    static CURRENT: Cell<InterfaceTheme> = const { Cell::new(InterfaceTheme::Modern) };
}

pub fn current() -> InterfaceTheme {
    CURRENT.with(Cell::get)
}

pub fn metrics() -> &'static Metrics {
    current().metrics()
}

pub fn palette() -> &'static Palette {
    current().palette()
}

macro_rules! colors {
    ($($name:ident => $field:ident),* $(,)?) => {
        $(pub fn $name() -> Color32 { palette().$field })*
    };
}

colors! {
    background_color => background, panel_color => panel, canvas_color => canvas,
    row_color => row, black_row_color => black_row, grid_color => grid,
    text_color => text, muted_color => muted, accent_color => accent,
    edge_color => edge, border_color => border, button_color => button,
    button_hover_color => button_hover, button_active_color => button_active,
    icon_tool_color => icon_tool,
}

pub fn section_title_font() -> FontId {
    native_font(if current() == InterfaceTheme::Classic {18.0} else {17.0})
}

pub fn native_font(height: f32) -> FontId {
    FontId::new(fonts::native_size(height, false), FontFamily::Name("NativeUI".into()))
}

pub fn native_heading(height: f32) -> FontId {
    FontId::new(fonts::native_size(height, true), FontFamily::Name("NativeHeading".into()))
}

pub fn install(ctx: &Context) {
    install_style(ctx, InterfaceTheme::Modern);
}

pub fn apply(ctx: &Context, theme: InterfaceTheme) {
    if current() != theme {
        install_style(ctx, theme);
        ctx.request_repaint();
    }
}

fn install_style(ctx: &Context, theme: InterfaceTheme) {
    CURRENT.with(|current| current.set(theme));
    let palette = theme.palette();
    let classic = theme == InterfaceTheme::Classic;
    let mut style = Style {
        visuals: Visuals::dark(),
        ..Default::default()
    };
    style.visuals.panel_fill = palette.panel;
    style.visuals.window_fill = palette.panel;
    style.visuals.extreme_bg_color = palette.background;
    style.visuals.override_text_color = Some(palette.text);
    style.visuals.selection.bg_fill = if classic {
        palette.selection.gamma_multiply(0.35)
    } else {
        palette.selection
    };
    style.visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    if classic {
        style.visuals.hyperlink_color = palette.accent;
        style.visuals.widgets.active.fg_stroke.color = palette.accent;
    }
    style.visuals.widgets.inactive.bg_fill = palette.button;
    style.visuals.widgets.inactive.weak_bg_fill = palette.button;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(0.5, palette.border);
    style.visuals.widgets.hovered.bg_fill = palette.button_hover;
    style.visuals.widgets.hovered.weak_bg_fill = palette.button_hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(0.5, palette.border);
    style.visuals.widgets.active.bg_fill = palette.button_active;
    style.visuals.widgets.active.weak_bg_fill = palette.button_active;
    style.visuals.widgets.active.bg_stroke = Stroke::new(0.5, palette.border);
    let radius = CornerRadius::same(if classic { 3 } else { 2 });
    style.visuals.widgets.inactive.corner_radius = radius;
    style.visuals.widgets.hovered.corner_radius = radius;
    style.visuals.widgets.active.corner_radius = radius;
    style.spacing.item_spacing = vec2(6.0, 6.0);
    style.spacing.button_padding = vec2(7.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    let font = native_font(if classic { 16.0 } else { 18.0 });
    style.text_styles.insert(TextStyle::Body, font.clone());
    style.text_styles.insert(TextStyle::Button, font);
    ctx.set_theme(Theme::Dark);
    ctx.set_style_of(Theme::Dark, style);
}
