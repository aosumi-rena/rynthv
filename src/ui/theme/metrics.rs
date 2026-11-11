#[derive(Clone, Copy, Debug, PartialEq)]
enum BlackKeyWidth {
    Pixels(f32),
    Fraction(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub menu_height: f32,
    pub transport_height: f32,
    pub workspace_inset: f32,
    pub column_gap: f32,
    pub editor_gap: f32,
    pub surface_radius: u8,
    pub surface_inset: f32,
    pub arrangement_title_height: f32,
    pub arrangement_header_padding: f32,
    pub track_height: f32,
    pub lane_tabs_height: f32,
    pub piano_group_strip_height: f32,
    pub piano_tool_width: f32,
    pub piano_toggle_width: f32,
    pub property_padding: i8,
    pub rail_width: f32,
    pub rail_button_size: f32,
    pub rail_horizontal_padding: f32,
    pub rail_top_padding: f32,
    pub rail_button_spacing: f32,
    pub rail_separator_before: f32,
    pub panel_header_height: f32,
    pub arrangement_header_height: f32,
    pub arrangement_gutter_width: f32,
    pub arrangement_gutter_fraction: f32,
    pub arrangement_min_canvas: f32,
    pub piano_header_height: f32,
    pub piano_title_width: f32,
    pub piano_title_height: f32,
    pub piano_title_padding: f32,
    pub piano_collapse_width: f32,
    pub piano_collapse_height: f32,
    pub piano_title_gap: f32,
    pub piano_control_height: f32,
    pub piano_tools_gap: f32,
    pub piano_toggle_gap: f32,
    pub piano_gutter_width: f32,
    pub piano_ruler_height: f32,
    black_key_width: BlackKeyWidth,
    pub keyboard_corner_radius_max: f32,
    pub keyboard_radius_tracks_size: bool,
    pub keyboard_square_white_left: bool,
    pub keyboard_white_stroke: f32,
}

impl Metrics {
    pub fn black_key_width(&self, white_width: f32) -> f32 {
        match self.black_key_width {
            BlackKeyWidth::Pixels(width) => width,
            BlackKeyWidth::Fraction(fraction) => white_width * fraction,
        }
    }

    pub fn key_radius(&self, width: f32, height: f32) -> f32 {
        if self.keyboard_radius_tracks_size {
            self.keyboard_corner_radius_max.min(width.min(height).max(0.0) * 0.5)
        } else {
            self.keyboard_corner_radius_max
        }
    }

    pub fn arrangement_gutter(&self, width: f32) -> f32 {
        self.arrangement_gutter_width
            .min((width * self.arrangement_gutter_fraction).max(0.0))
            .min((width - self.arrangement_min_canvas).max(0.0))
    }
}

pub const MODERN: Metrics = Metrics {
    menu_height: 24.0,
    transport_height: 44.0,
    workspace_inset: 2.0,
    column_gap: 0.0,
    editor_gap: 0.0,
    surface_radius: 4,
    surface_inset: 2.0,
    arrangement_title_height: 44.0,
    arrangement_header_padding: 8.0,
    track_height: 57.0,
    lane_tabs_height: 32.0,
    piano_group_strip_height: 20.0,
    piano_tool_width: 40.0,
    piano_toggle_width: 32.0,
    property_padding: 20,
    rail_width: 48.0,
    rail_button_size: 48.0,
    rail_horizontal_padding: 0.0,
    rail_top_padding: 8.0,
    rail_button_spacing: 0.0,
    rail_separator_before: 6.0,
    panel_header_height: 44.0,
    arrangement_header_height: 44.0,
    arrangement_gutter_width: 252.0,
    arrangement_gutter_fraction: 1.0,
    arrangement_min_canvas: 64.0,
    piano_header_height: 40.0,
    piano_title_width: 125.0,
    piano_title_height: 40.0,
    piano_title_padding: 10.0,
    piano_collapse_width: 10.0,
    piano_collapse_height: 40.0,
    piano_title_gap: 9.0,
    piano_control_height: 24.0,
    piano_tools_gap: 16.0,
    piano_toggle_gap: 8.0,
    piano_gutter_width: 50.0,
    piano_ruler_height: 32.0,
    black_key_width: BlackKeyWidth::Fraction(0.54),
    keyboard_corner_radius_max: 4.0,
    keyboard_radius_tracks_size: true,
    keyboard_square_white_left: true,
    keyboard_white_stroke: 0.0,
};

pub const CLASSIC: Metrics = Metrics {
    menu_height: 24.0,
    transport_height: 0.0,
    workspace_inset: 0.0,
    column_gap: 0.0,
    editor_gap: 0.0,
    surface_radius: 0,
    surface_inset: 0.0,
    arrangement_title_height: 26.0,
    arrangement_header_padding: 0.0,
    track_height: 50.0,
    lane_tabs_height: 26.0,
    piano_group_strip_height: 0.0,
    piano_tool_width: 24.0,
    piano_toggle_width: 24.0,
    property_padding: 20,
    rail_width: 48.0,
    rail_button_size: 48.0,
    rail_horizontal_padding: 0.0,
    rail_top_padding: 50.0,
    rail_button_spacing: 0.0,
    rail_separator_before: 0.0,
    panel_header_height: 50.0,
    arrangement_header_height: 50.0,
    arrangement_gutter_width: 250.0,
    arrangement_gutter_fraction: 1.0,
    arrangement_min_canvas: 64.0,
    piano_header_height: 26.0,
    piano_title_width: 126.0,
    piano_title_height: 26.0,
    piano_title_padding: 10.0,
    piano_collapse_width: 10.0,
    piano_collapse_height: 26.0,
    piano_title_gap: 9.0,
    piano_control_height: 16.0,
    piano_tools_gap: 12.0,
    piano_toggle_gap: 0.0,
    piano_gutter_width: 51.0,
    piano_ruler_height: 24.0,
    black_key_width: BlackKeyWidth::Fraction(0.5),
    keyboard_corner_radius_max: 2.0,
    keyboard_radius_tracks_size: false,
    keyboard_square_white_left: false,
    keyboard_white_stroke: 0.0,
};
