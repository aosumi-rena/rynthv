use super::theme::*;
use eframe::egui::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    ChevronDown,
    ChevronRight,
    Plus,
    Close,
    Play,
    Pause,
    Stop,
    Record,
    Rewind,
    Forward,
    Loop,
    Metronome,
    Return,
    Microphone,
    Warning,
    Notes,
    Dictionary,
    Render,
    Script,
    Cloud,
    Settings,
    NoteSelect,
    NoteDraw,
    PitchAnchor,
    PitchDraw,
    Pitch,
    Waveform,
    Retake,
    Scale,
    Magnet,
    TempoTracks,
    Pan,
    Curve,
    Swap,
    Phonemes,
    Brand,
    AllRetakes,
    PitchRetake,
    TimbreRetake,
    TimingRetake,
    ChoirShuffle,
    Freehand,
    LineTool,
    ControlPoint,
    LinearInterpolation,
    CosineInterpolation,
    SplineInterpolation,
    SettingsGeneral,
    Speaker,
    SettingsEditing,
    SettingsRecording,
    SettingsRendering,
    Keyboard,
    SettingsCheck,
    SettingsDropdown,
    Dropdown,
    Check,
    InputPitch,
    OutputPitch,
    ModeNote,
    ModePitch,
    MultiEdit,
    Folder,
    Menu,
    MenuCheck,
}
impl Icon {
    pub fn image(self) -> Image<'static> {
        if current() == InterfaceTheme::Classic {
            match self {
                Self::Microphone => return Image::new(include_image!("../../assets/icons/classic/microphone.svg")),
                Self::Notes => return Image::new(include_image!("../../assets/icons/classic/notes.svg")),
                Self::Dictionary => return Image::new(include_image!("../../assets/icons/classic/dictionary.svg")),
                Self::Render => return Image::new(include_image!("../../assets/icons/classic/render.svg")),
                Self::Cloud => return Image::new(include_image!("../../assets/icons/classic/cloud.svg")),
                Self::Settings => return Image::new(include_image!("../../assets/icons/classic/settings.svg")),
                Self::Dropdown => return Image::new(include_image!("../../assets/icons/classic/dropdown.svg")),
                Self::Check => return Image::new(include_image!("../../assets/icons/classic/check.svg")),
                Self::InputPitch => return Image::new(include_image!("../../assets/icons/classic/input-pitch.svg")),
                Self::OutputPitch => return Image::new(include_image!("../../assets/icons/classic/output-pitch.svg")),
                Self::Pitch => return Image::new(include_image!("../../assets/icons/classic/pitch.svg")),
                Self::Waveform => return Image::new(include_image!("../../assets/icons/classic/waveform.svg")),
                Self::Retake => return Image::new(include_image!("../../assets/icons/classic/retake.svg")),
                Self::Phonemes => return Image::new(include_image!("../../assets/icons/classic/phonemes.svg")),
                Self::ModeNote => return Image::new(include_image!("../../assets/icons/classic/mode-note.svg")),
                Self::ModePitch => return Image::new(include_image!("../../assets/icons/classic/mode-pitch.svg")),
                Self::MultiEdit => return Image::new(include_image!("../../assets/icons/classic/multi-edit.svg")),
                Self::ControlPoint => return Image::new(include_image!("../../assets/icons/classic/control-point.svg")),
                Self::Freehand => return Image::new(include_image!("../../assets/icons/classic/freehand.svg")),
                Self::LineTool => return Image::new(include_image!("../../assets/icons/classic/line-tool.svg")),
                Self::LinearInterpolation => return Image::new(include_image!("../../assets/icons/classic/linear-interpolation.svg")),
                Self::CosineInterpolation => return Image::new(include_image!("../../assets/icons/classic/cosine-interpolation.svg")),
                Self::SplineInterpolation => return Image::new(include_image!("../../assets/icons/classic/spline-interpolation.svg")),
                Self::Play => return Image::new(include_image!("../../assets/icons/classic/play.svg")),
                Self::Pause => return Image::new(include_image!("../../assets/icons/classic/pause.svg")),
                Self::Stop => return Image::new(include_image!("../../assets/icons/classic/stop.svg")),
                Self::Record => return Image::new(include_image!("../../assets/icons/classic/record.svg")),
                Self::Rewind => return Image::new(include_image!("../../assets/icons/classic/rewind.svg")),
                Self::Forward => return Image::new(include_image!("../../assets/icons/classic/forward.svg")),
                Self::Return => return Image::new(include_image!("../../assets/icons/classic/return.svg")),
                Self::Loop => return Image::new(include_image!("../../assets/icons/classic/loop.svg")),
                Self::Metronome => return Image::new(include_image!("../../assets/icons/classic/metronome.svg")),
                Self::Magnet => return Image::new(include_image!("../../assets/icons/classic/magnet.svg")),
                Self::Swap => return Image::new(include_image!("../../assets/icons/classic/swap.svg")),
                Self::Folder => return Image::new(include_image!("../../assets/icons/classic/folder.svg")),
                _ => {}
            }
        }
        match self {
            Self::ChevronDown => Image::new(include_image!("../../assets/icons/modern/section-chevron-down-native.svg")),
            Self::ChevronRight => Image::new(include_image!("../../assets/icons/modern/section-chevron-right-native.svg")),
            Self::Plus => Image::new(include_image!("../../assets/icons/modern/add-tracks-header24.svg")),
            Self::Close => Image::new(include_image!("../../assets/icons/modern/close-native-glyph.svg")),
            Self::Play => Image::new(include_image!("../../assets/icons/modern/play-control.svg")),
            Self::Pause => Image::new(include_image!("../../assets/icons/modern/pause-control.svg")),
            Self::Stop => Image::new(include_image!("../../assets/icons/modern/stop-control.svg")),
            Self::Record => Image::new(include_image!("../../assets/icons/modern/record-control.svg")),
            Self::Rewind => Image::new(include_image!("../../assets/icons/modern/rewind-control.svg")),
            Self::Forward => Image::new(include_image!("../../assets/icons/modern/forward-control.svg")),
            Self::Loop => Image::new(include_image!("../../assets/icons/modern/loop-control.svg")),
            Self::Metronome => Image::new(include_image!("../../assets/icons/modern/metronome-control.svg")),
            Self::Return => Image::new(include_image!("../../assets/icons/modern/return-playhead-control.svg")),
            Self::Microphone => Image::new(include_image!("../../assets/icons/modern/microphone.svg")),
            Self::Warning => Image::new(include_image!("../../assets/icons/modern/warning.svg")),
            Self::Notes => Image::new(include_image!("../../assets/icons/modern/notes.svg")),
            Self::Dictionary => Image::new(include_image!("../../assets/icons/modern/dictionary.svg")),
            Self::Render => Image::new(include_image!("../../assets/icons/modern/render.svg")),
            Self::Script => Image::new(include_image!("../../assets/icons/script.svg")),
            Self::Cloud => Image::new(include_image!("../../assets/icons/modern/cloud.svg")),
            Self::Settings => Image::new(include_image!("../../assets/icons/modern/settings.svg")),
            Self::NoteSelect => Image::new(include_image!("../../assets/icons/modern/note-select-control.svg")),
            Self::NoteDraw => Image::new(include_image!("../../assets/icons/modern/note-draw-control.svg")),
            Self::PitchAnchor => Image::new(include_image!("../../assets/icons/modern/pitch-anchor-control.svg")),
            Self::PitchDraw => Image::new(include_image!("../../assets/icons/modern/pitch-draw-control.svg")),
            Self::Pitch => Image::new(include_image!("../../assets/icons/modern/pitch-visualization-control.svg")),
            Self::Waveform => Image::new(include_image!("../../assets/icons/modern/waveform-visualization-control.svg")),
            Self::Retake => Image::new(include_image!("../../assets/icons/modern/retake-control.svg")),
            Self::Scale => Image::new(include_image!("../../assets/icons/modern/scale-control.svg")),
            Self::Magnet => Image::new(include_image!("../../assets/icons/modern/magnet-control.svg")),
            Self::TempoTracks => Image::new(include_image!("../../assets/icons/modern/tempo-tracks-header24.svg")),
            Self::Pan => Image::new(include_image!("../../assets/icons/modern/pan-label-native.svg")),
            Self::Curve => Image::new(include_image!("../../assets/icons/modern/parameter-curve-glyph.svg")),
            Self::Swap => Image::new(include_image!("../../assets/icons/modern/swap-parameters-control.svg")),
            Self::Phonemes => Image::new(include_image!("../../assets/icons/modern/phonemes-glyph.svg")),
            Self::Brand => Image::new(include_image!("../../assets/icons/brand.svg")),
            Self::AllRetakes => Image::new(include_image!("../../assets/icons/modern/all-retakes-control.svg")),
            Self::PitchRetake => Image::new(include_image!("../../assets/icons/modern/pitch-retake-control.svg")),
            Self::TimbreRetake => Image::new(include_image!("../../assets/icons/modern/timbre-retake-control.svg")),
            Self::TimingRetake => Image::new(include_image!("../../assets/icons/modern/timing-retake-control.svg")),
            Self::ChoirShuffle => Image::new(include_image!("../../assets/icons/modern/choir-shuffle.svg")),
            Self::Freehand => Image::new(include_image!("../../assets/icons/modern/freehand-control.svg")),
            Self::LineTool => Image::new(include_image!("../../assets/icons/modern/parameter-line-control.svg")),
            Self::ControlPoint => Image::new(include_image!("../../assets/icons/modern/parameter-point-control.svg")),
            Self::LinearInterpolation => Image::new(include_image!("../../assets/icons/modern/linear-interpolation-control.svg")),
            Self::CosineInterpolation => Image::new(include_image!("../../assets/icons/modern/cosine-interpolation-control.svg")),
            Self::SplineInterpolation => Image::new(include_image!("../../assets/icons/modern/spline-interpolation-control.svg")),
            Self::SettingsGeneral => Image::new(include_image!("../../assets/icons/modern/settings-general.svg")),
            Self::Speaker => Image::new(include_image!("../../assets/icons/modern/speaker.svg")),
            Self::SettingsEditing => Image::new(include_image!("../../assets/icons/modern/settings-editor.svg")),
            Self::SettingsRecording => Image::new(include_image!("../../assets/icons/modern/settings-recording.svg")),
            Self::SettingsRendering => Image::new(include_image!("../../assets/icons/modern/settings-rendering.svg")),
            Self::Keyboard => Image::new(include_image!("../../assets/icons/modern/keyboard.svg")),
            Self::SettingsCheck => Image::new(include_image!("../../assets/icons/modern/checkbox-checked.svg")),
            Self::SettingsDropdown => Image::new(include_image!("../../assets/icons/modern/chevron-down.svg")),
            Self::InputPitch => Image::new(include_image!("../../assets/icons/classic/input-pitch.svg")),
            Self::OutputPitch => Image::new(include_image!("../../assets/icons/classic/output-pitch.svg")),
            Self::ModeNote => Image::new(include_image!("../../assets/icons/classic/mode-note.svg")),
            Self::ModePitch => Image::new(include_image!("../../assets/icons/classic/mode-pitch.svg")),
            Self::MultiEdit => Image::new(include_image!("../../assets/icons/classic/multi-edit.svg")),
            Self::Folder => Image::new(include_image!("../../assets/icons/classic/folder.svg")),
            Self::Menu => Image::new(include_image!("../../assets/icons/classic/menu.svg")),
            Self::Dropdown => Image::new(include_image!("../../assets/icons/modern/dropdown.svg")),
            Self::Check => Image::new(include_image!("../../assets/icons/modern/check.svg")),
            Self::MenuCheck => Image::new(include_image!("../../assets/icons/modern/menu-check.svg")),
        }
    }
    fn view_size(self) -> Vec2 {
        if current() == InterfaceTheme::Classic {
            match self {
                Self::Microphone => return vec2(11.999991, 20.000001),
                Self::Notes => return vec2(18.0, 19.0),
                Self::Dictionary => return vec2(16.0, 18.0),
                Self::Render => return vec2(18.0, 18.0),
                Self::Cloud => return vec2(17.999563, 15.999999),
                Self::Settings => return vec2(18.0, 18.0),
                Self::Dropdown => return vec2(12.0, 7.0),
                Self::Check => return vec2(14.0, 14.0),
                Self::InputPitch => return vec2(24.0, 16.0),
                Self::OutputPitch => return vec2(24.0, 16.0),
                Self::Pitch => return vec2(24.0, 16.0),
                Self::Waveform => return vec2(24.0, 16.0),
                Self::Retake => return vec2(24.0, 16.0),
                Self::Phonemes => return vec2(24.0, 16.0),
                Self::ModeNote => return vec2(24.0, 16.0),
                Self::ModePitch => return vec2(24.0, 16.0),
                Self::MultiEdit => return vec2(24.0, 16.0),
                Self::ControlPoint => return vec2(24.0, 16.0),
                Self::Freehand => return vec2(24.0, 16.0),
                Self::LineTool => return vec2(24.0, 16.0),
                Self::LinearInterpolation => return vec2(24.0, 16.0),
                Self::CosineInterpolation => return vec2(24.0, 16.0),
                Self::SplineInterpolation => return vec2(24.0, 16.0),
                Self::Play => return vec2(20.0, 24.0),
                Self::Pause => return vec2(20.0, 24.0),
                Self::Stop => return vec2(20.0, 24.0),
                Self::Record => return vec2(20.0, 24.0),
                Self::Rewind => return vec2(20.0, 24.0),
                Self::Forward => return vec2(20.0, 24.0),
                Self::Return => return vec2(24.0, 16.0),
                Self::Loop => return vec2(24.0, 16.0),
                Self::Metronome => return vec2(24.0, 16.0),
                Self::Magnet => return vec2(18.0, 16.0),
                Self::Swap => return vec2(14.0, 24.0),
                Self::Folder => return vec2(16.0, 16.0),
                _ => {}
            }
        }
        match self {
            Self::ChevronDown => vec2(7.0, 4.5),
            Self::ChevronRight => vec2(4.5, 7.0),
            Self::Plus => vec2(24.0, 24.0),
            Self::Close => vec2(10.0, 10.0),
            Self::Play => vec2(24.0, 28.0),
            Self::Pause => vec2(24.0, 28.0),
            Self::Stop => vec2(24.0, 28.0),
            Self::Record => vec2(24.0, 28.0),
            Self::Rewind => vec2(24.0, 28.0),
            Self::Forward => vec2(24.0, 28.0),
            Self::Loop => vec2(24.0, 28.0),
            Self::Metronome => vec2(41.0, 28.0),
            Self::Return => vec2(41.0, 28.0),
            Self::Microphone => vec2(10.0, 12.0),
            Self::Warning => vec2(14.0, 12.0),
            Self::Notes => vec2(16.0, 16.0),
            Self::Dictionary => vec2(14.0, 16.0),
            Self::Render => vec2(16.0, 17.0),
            Self::Script => vec2(24.0, 24.0),
            Self::Cloud => vec2(20.0, 14.0),
            Self::Settings => vec2(16.0, 16.0),
            Self::NoteSelect => vec2(40.0, 24.0),
            Self::NoteDraw => vec2(40.0, 24.0),
            Self::PitchAnchor => vec2(40.0, 24.0),
            Self::PitchDraw => vec2(40.0, 24.0),
            Self::Pitch => vec2(32.0, 24.0),
            Self::Waveform => vec2(32.0, 24.0),
            Self::Retake => vec2(32.0, 24.0),
            Self::Scale => vec2(32.0, 24.0),
            Self::Magnet => vec2(32.0, 24.0),
            Self::TempoTracks => vec2(24.0, 24.0),
            Self::Pan => vec2(11.0, 6.0),
            Self::Curve => vec2(16.42776247859001, 11.375),
            Self::Swap => vec2(14.0, 24.0),
            Self::Phonemes => vec2(13.981521370820701, 9.610766410827637),
            Self::Brand => vec2(48.0, 48.0),
            Self::AllRetakes => vec2(98.0, 88.0),
            Self::PitchRetake => vec2(98.0, 88.0),
            Self::TimbreRetake => vec2(98.0, 88.0),
            Self::TimingRetake => vec2(98.0, 88.0),
            Self::ChoirShuffle => vec2(16.131369806826115, 16.0),
            Self::Freehand => vec2(32.0, 24.0),
            Self::LineTool => vec2(32.0, 24.0),
            Self::ControlPoint => vec2(32.0, 24.0),
            Self::LinearInterpolation => vec2(32.0, 24.0),
            Self::CosineInterpolation => vec2(32.0, 24.0),
            Self::SplineInterpolation => vec2(32.0, 24.0),
            Self::SettingsGeneral => vec2(21.0, 17.0),
            Self::Speaker => vec2(20.0, 20.0),
            Self::SettingsEditing => vec2(20.0, 20.0),
            Self::SettingsRecording => vec2(20.0, 20.0),
            Self::SettingsRendering => vec2(20.0, 20.0),
            Self::Keyboard => vec2(20.0, 20.0),
            Self::SettingsCheck => vec2(16.0, 16.0),
            Self::SettingsDropdown => vec2(10.0, 6.0),
            Self::Dropdown => vec2(7.0, 4.5),
            Self::Check => vec2(14.0, 14.0),
            Self::InputPitch => vec2(24.0, 16.0),
            Self::OutputPitch => vec2(24.0, 16.0),
            Self::ModeNote => vec2(24.0, 16.0),
            Self::ModePitch => vec2(24.0, 16.0),
            Self::MultiEdit => vec2(24.0, 16.0),
            Self::Folder => vec2(16.0, 16.0),
            Self::Menu => vec2(24.0, 24.0),
            Self::MenuCheck => vec2(18.0, 15.0),
        }
    }
    pub fn paint(self, ui: &Ui, rect: Rect, tint: Color32) {
        let tint = if matches!(self, Self::SettingsGeneral | Self::Speaker | Self::SettingsEditing |
            Self::SettingsRecording | Self::SettingsRendering | Self::Keyboard | Self::SettingsCheck |
            Self::SettingsDropdown) || (current()==InterfaceTheme::Classic && matches!(self, Self::Magnet | Self::Swap)) { Color32::from_white_alpha(tint.a()) } else { tint };
        let size = self.view_size();
        let mut scale = (rect.width()/size.x).min(rect.height()/size.y);
        if matches!(self, Self::ChevronDown | Self::ChevronRight | Self::Dropdown | Self::SettingsDropdown | Self::Check | Self::MenuCheck | Self::Pan) {
            scale = scale.min(1.0);
        }
        let target = Rect::from_center_size(rect.center(),size*scale);
        self.image().tint(tint).paint_at(ui, target);
    }
    pub fn paint_control(self, ui: &Ui, rect: Rect, tint: Color32) {
        let tint = if current()==InterfaceTheme::Classic && matches!(self,Self::Magnet | Self::Swap) {Color32::from_white_alpha(tint.a())} else {tint};
        let size = if current() == InterfaceTheme::Classic && matches!(self,Self::Play | Self::Pause | Self::Stop | Self::Record | Self::Rewind | Self::Forward) {vec2(20.0,24.0)}
        else if current() == InterfaceTheme::Classic && matches!(self,Self::Loop | Self::Metronome | Self::Return) {vec2(24.0,16.0)}
        else if current() == InterfaceTheme::Classic && self==Self::Magnet {vec2(18.0,16.0)}
        else if current() == InterfaceTheme::Classic && matches!(self,
            Self::InputPitch | Self::OutputPitch | Self::Pitch | Self::Waveform | Self::Retake | Self::Phonemes |
            Self::ModeNote | Self::ModePitch | Self::MultiEdit | Self::ControlPoint | Self::Freehand | Self::LineTool |
            Self::LinearInterpolation | Self::CosineInterpolation | Self::SplineInterpolation) {
            vec2(24.0,16.0)
        } else { match self {
            Self::NoteSelect | Self::NoteDraw | Self::PitchAnchor | Self::PitchDraw => vec2(40.0, 24.0),
            Self::Play | Self::Pause | Self::Stop | Self::Record | Self::Rewind | Self::Forward | Self::Loop => vec2(24.0, 28.0),
            Self::AllRetakes | Self::PitchRetake | Self::TimbreRetake | Self::TimingRetake => vec2(98.0, 88.0),
            Self::Menu => vec2(24.0, 24.0),
            Self::Plus | Self::TempoTracks => vec2(24.0, 24.0),
            Self::Swap => vec2(14.0, 24.0),
            Self::Metronome | Self::Return => vec2(41.0, 28.0),
            _ => vec2(32.0, 24.0),
        }};
        let target = Rect::from_center_size(rect.center(), size);
        self.image().tint(tint).fit_to_exact_size(target.size()).paint_at(ui, target);
    }
}

pub struct IconButton {
    icon: Icon,
    label: &'static str,
    size: Vec2,
    icon_size: f32,
    native: bool,
    rail: bool,
    tool: bool,
    transport: bool,
    round_left: bool,
    round_right: bool,
    selected: bool,
    framed: bool,
    shared_background: bool,
    background: Option<Color32>,
}
impl IconButton {
    pub fn new(icon: Icon, label: &'static str) -> Self {
        Self {
            icon,
            label,
            size: vec2(28.0, 24.0),
            icon_size: 18.0,
            native: false,
            rail: false,
            tool: false,
            transport: false,
            round_left: true,
            round_right: true,
            selected: false,
            framed: false,
            shared_background: false,
            background: None,
        }
    }
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = size;
        self
    }
    pub fn native(mut self) -> Self { self.native = true; self }
    pub fn rail(mut self) -> Self { self.rail = true; self }
    pub fn tool(mut self) -> Self { self.native = true; self.tool = true; self }
    pub fn group_edges(mut self, first: bool, last: bool) -> Self { self.round_left = first; self.round_right = last; self }
    pub fn transport(mut self) -> Self { self.native = true; self.transport = true; self }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn framed(mut self) -> Self {
        self.framed = true;
        self
    }
    pub fn grouped(mut self) -> Self {
        self.framed = true;
        self.shared_background = true;
        self
    }
    pub fn background(mut self, color: Color32) -> Self {
        self.background = Some(color);
        self
    }
}
impl Widget for IconButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = ui.is_enabled();
        let (rect, response) = ui.allocate_exact_size(
            self.size,
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, self.label));
        let hovered = response.hovered() && enabled;
        if self.tool {
            if current() == InterfaceTheme::Classic {
                let corners=CornerRadius {nw:if self.round_left {3}else{0},sw:if self.round_left {3}else{0},ne:if self.round_right {3}else{0},se:if self.round_right {3}else{0}};
                if self.icon != Icon::Magnet { ui.painter().rect_filled(rect, corners, if self.selected { Color32::from_rgb(152,153,152) } else { Color32::from_gray(71) }); }
            } else if self.selected { ui.painter().rect_filled(rect, 3, Color32::from_rgb(87,84,89)); }
        }
        if !self.tool && !self.rail && (!self.native || self.framed) && ((self.framed && !self.shared_background) || self.selected || hovered) {
            let fill = if self.selected || response.is_pointer_button_down_on() {
                self.background.unwrap_or(button_active_color())
            } else if hovered {
                button_hover_color()
            } else {
                self.background.unwrap_or(button_color())
            };
            ui.painter().rect_filled(rect, 3, fill);
        }
        let tint = if self.selected {
            text_color()
        } else if self.framed {
            icon_tool_color()
        } else {
            muted_color()
        };
        let tint = if current()==InterfaceTheme::Classic && self.icon==Icon::Menu {
            if hovered && !response.is_pointer_button_down_on() {Color32::from_gray(160)} else {Color32::from_white_alpha(217)}
        } else if self.tool {
            if current() == InterfaceTheme::Classic {
                if self.selected { Color32::from_gray(71) } else { Color32::from_gray(166) }
            } else if self.selected { Color32::from_rgb(239,232,242) } else { Color32::from_rgb(163,157,166) }
        } else if self.transport {
            if current()==InterfaceTheme::Classic {
                if hovered || response.is_pointer_button_down_on() {Color32::from_rgb(125,178,53)}
                else if self.selected || self.icon==Icon::Pause {Color32::from_rgb(209,79,91)}
                else {Color32::from_gray(184)}
            } else if self.selected {Color32::from_rgb(239,232,242)} else {Color32::from_rgb(138,133,140)}
        } else if self.rail {
            let active = Color32::from_rgb(121, 216, 37);
            let inactive = Color32::from_gray(151);
            if response.is_pointer_button_down_on() {
                if self.selected { inactive } else { active }
            } else if hovered {
                let color = if self.selected { active } else { inactive };
                let channel = |v: u8| if self.selected { (v as f32 * 0.9090909) as u8 } else { (255.0 - (255.0 - v as f32) * 0.9090909) as u8 };
                Color32::from_rgba_unmultiplied(channel(color.r()), channel(color.g()), channel(color.b()), color.a())
            } else if self.selected { active } else { inactive }
        } else { tint };
        if current()==InterfaceTheme::Classic && self.icon==Icon::Magnet {
            let image=if self.selected ^ response.is_pointer_button_down_on() {
                Image::new(include_image!("../../assets/icons/classic/snap-selected.svg"))
            } else {self.icon.image()};
            image.paint_at(ui,Rect::from_center_size(rect.center(),vec2(18.0,16.0)));
        } else if current()==InterfaceTheme::Classic && self.icon==Icon::Swap {
            let image=if hovered {Image::new(include_image!("../../assets/icons/classic/swap-hover.svg"))} else {self.icon.image()};
            image.paint_at(ui,Rect::from_center_size(rect.center(),vec2(14.0,24.0)));
        } else if self.rail {
            let size = match (current(), self.icon) {
                (InterfaceTheme::Classic, Icon::Microphone) => vec2(12.0, 20.0),
                (InterfaceTheme::Classic, Icon::Notes) => vec2(18.0, 19.0),
                (InterfaceTheme::Classic, Icon::Dictionary) => vec2(16.0, 18.0),
                (InterfaceTheme::Classic, Icon::Render | Icon::Settings) => vec2(18.0, 18.0),
                (InterfaceTheme::Classic, Icon::Cloud) => vec2(18.0, 16.0),
                (_, Icon::Microphone) => vec2(10.0, 12.0),
                (_, Icon::Dictionary) => vec2(14.0, 16.0),
                (_, Icon::Render) => vec2(16.0, 17.0),
                (_, Icon::Cloud) => vec2(20.0, 14.0),
                _ => vec2(16.0, 16.0),
            };
            self.icon.paint(ui, Rect::from_center_size(rect.center(), size), tint);
        } else if self.native {
            self.icon.paint_control(ui, rect, tint);
        } else {
            self.icon.paint(ui, Rect::from_center_size(rect.center(), Vec2::splat(self.icon_size)), tint);
        }
        response
            .on_hover_text(self.label)
            .on_disabled_hover_text(self.label)
    }
}

pub fn close_button(ui: &mut Ui) -> Response {
    ui.add(IconButton::new(Icon::Close, crate::i18n::t("删除")).size(vec2(18.0,18.0)).icon_size(10.0))
}
