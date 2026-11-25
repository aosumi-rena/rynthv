use crate::model::{Parameter, new_id};
use serde::{Deserialize, Serialize};

use super::scale::ScaleDisplay;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Column {
    Editor,
    Voice,
    Note,
    Dictionary,
    Render,
    Script,
    Effects,
}
impl Column {
    pub const PANELS: [Self; 6] = [
        Self::Voice,
        Self::Note,
        Self::Dictionary,
        Self::Effects,
        Self::Render,
        Self::Script,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Editor => crate::i18n::t("编辑器"),
            Self::Voice => crate::i18n::t("Voice"),
            Self::Note => crate::i18n::t("音符"),
            Self::Dictionary => crate::i18n::t("词典"),
            Self::Render => crate::i18n::t("Render"),
            Self::Script => crate::i18n::t("脚本"),
            Self::Effects => crate::i18n::t("Effects"),
        }
    }
    pub fn icon(self) -> super::icons::Icon {
        use super::icons::Icon;
        match self {
            Self::Editor => Icon::NoteSelect,
            Self::Voice => Icon::Microphone,
            Self::Note => Icon::Notes,
            Self::Dictionary => Icon::Dictionary,
            Self::Render => Icon::Render,
            Self::Script => Icon::Script,
            Self::Effects => Icon::Effects,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Lane {
    pub id: u64,
    pub foreground: Parameter,
    pub background: Parameter,
    pub visible: bool,
    pub height: f32,
    pub tool: CurveTool,
    pub zoom: f32,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveTool {
    #[default]
    Point,
    Pencil,
    Line,
}
impl Lane {
    pub fn new() -> Self {
        Self {
            id: new_id(),
            foreground: Parameter::Pitch,
            background: Parameter::Loudness,
            visible: true,
            height: 100.0,
            tool: CurveTool::Point,
            zoom: 100.0,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub start: f64,
    pub beat_width: f32,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ViewState {
    pub columns: Vec<Column>,
    #[serde(skip)]
    pub auto_hidden: std::collections::HashSet<Column>,
    #[serde(skip)]
    pub focused_panel: Option<Column>,
    #[serde(default="panel_order")]
    pub panel_order: Vec<Column>,
    pub panel_width: f32,
    pub arrangement_first: bool,
    pub arrangement_height: f32,
    pub arrangement_collapsed: bool,
    pub piano: Viewport,
    pub arrangement: Viewport,
    pub top_pitch: f32,
    pub row_height: f32,
    pub grid: i32,
    pub triplet: bool,
    pub snap: bool,
    pub scale: ScaleDisplay,
    pub lanes: Vec<Lane>,
    pub phonemes: bool,
    #[serde(default)]
    pub instrument_lanes: super::instruments::lanes::InstrumentLanes,
    #[serde(default="dock_position")]
    pub instrument_dock: [f32; 2],
}
impl Default for ViewState {
    fn default() -> Self {
        Self {
            columns: vec![Column::Editor],
            auto_hidden: Default::default(),
            focused_panel: None,
            panel_order: panel_order(),
            panel_width: 234.0,
            arrangement_first: true,
            arrangement_height: 264.0,
            arrangement_collapsed: false,
            piano: Viewport {
                start: 0.0,
                beat_width: 160.0,
            },
            arrangement: Viewport {
                start: 0.0,
                beat_width: 20.0,
            },
            top_pitch: 74.0,
            row_height: 24.0,
            grid: 16,
            triplet: false,
            snap: true,
            scale: ScaleDisplay::default(),
            lanes: vec![Lane {
                visible: false,
                ..Lane::new()
            }],
            phonemes: false,
            instrument_lanes: Default::default(),
            instrument_dock: dock_position(),
        }
    }
}
impl ViewState {
    pub fn toggle(&mut self, column: Column) {
        if self.columns.contains(&column) && self.auto_hidden.contains(&column) {
            self.focused_panel = Some(column);
            return;
        }
        if self.columns.contains(&column) {
            self.columns.retain(|c| *c != column);
            if self.focused_panel == Some(column) { self.focused_panel = None; }
        } else {
            self.focused_panel = Some(column);
            if !self.panel_order.contains(&column){self.panel_order.push(column);}
            let rank = self.panel_order.iter().position(|c|*c==column).unwrap();
            let index = self.columns.iter().position(|c|
                self.panel_order.iter().position(|p|p==c).unwrap()>rank).unwrap_or(self.columns.len());
            self.columns.insert(index, column);
        }
    }
    pub fn step(&self) -> f64 {
        4.0 / self.grid as f64 * if self.triplet { 2.0 / 3.0 } else { 1.0 }
    }
    pub fn snap_beat(&self, beat: f64) -> f64 {
        if self.snap {
            (beat / self.step()).round() * self.step()
        } else {
            beat
        }
    }
    pub fn valid(&self) -> bool {
        self.panel_order.contains(&Column::Editor)
            && self.panel_order.iter().collect::<std::collections::HashSet<_>>().len()==self.panel_order.len()
            && self.columns.iter().all(|c|self.panel_order.contains(c))
            && self.columns
            .iter()
            .filter(|c| **c == Column::Editor)
            .count()
            == 1
            && self
                .columns
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.columns.len()
            && self.panel_width.is_finite()
            && (120.0..=600.0).contains(&self.panel_width)
            && self.arrangement_height.is_finite()
            && (28.0..=1000.0).contains(&self.arrangement_height)
            && [&self.piano, &self.arrangement].iter().all(|v| {
                v.start.is_finite()
                    && v.start >= 0.0
                    && v.beat_width.is_finite()
                    && v.beat_width>0.0
            })
            && (2.0..=100.0).contains(&self.arrangement.beat_width)
            && (10.0..=500.0).contains(&self.piano.beat_width)
            && self.top_pitch.is_finite()
            && (12.0..=127.0).contains(&self.top_pitch)
            && self.row_height.is_finite()
            && (8.0..=48.0).contains(&self.row_height)
            && [4, 8, 16, 32, 64, 128].contains(&self.grid)
            && self.lanes.len() <= 16
            && self.instrument_lanes.valid()
            && self.instrument_dock.iter().all(|v|v.is_finite() && (0.0..=1.0).contains(v))
            && self.lanes.iter().all(|l| {
                l.height.is_finite()
                    && (64.0..=600.0).contains(&l.height)
                    && l.zoom.is_finite()
                    && (50.0..=200.0).contains(&l.zoom)
            })
    }
}
fn panel_order()->Vec<Column> {
    vec![Column::Editor,Column::Voice,Column::Note,Column::Dictionary,Column::Effects,Column::Render,Column::Script]
}
fn dock_position()->[f32;2] {[0.5,0.94]}
#[derive(Clone, Copy)]
pub enum DragPayload {
    Column(Column),
    Arrangement,
    Piano,
    Lane(u64),
}
