mod background;
mod background_notes;
mod canvas;
mod gestures;
mod keyboard;
mod lyrics;
mod phonemes;
mod pitch_anchor;
mod pitch_edit;
mod portrait;
mod rendered;
mod retakes;
mod scale_guide;
mod scene;
mod toolbar;
use crate::model::*;
pub use canvas::show;
use eframe::egui::Pos2;
pub use keyboard::{KeyboardAudition, begin_keyboard_frame, cancel_audition, end_keyboard_frame};
pub use pitch_anchor::{
    AnchorSelection, anchor_delete, anchor_keyboard, anchor_select_all, end_anchor_frame,
};
pub use pitch_anchor::{
    anchor_clear, anchor_copy, anchor_cut, anchor_has_selection,
    anchor_paste_text, anchor_shift_octave,
};
pub use pitch_edit::end_pitch_frame;
use std::collections::BTreeSet;
pub use toolbar::toolbar;
pub fn toolbar_height() -> f32 { crate::ui::theme::metrics().piano_header_height }
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    NoteSelect,
    NoteDraw,
    PitchAnchor,
    PitchDraw,
}
impl Tool {
    pub fn is_pitch(self) -> bool {
        matches!(self, Self::PitchAnchor | Self::PitchDraw)
    }
}
pub struct LyricEdit {
    pub id: EntityId,
    pub text: String,
    pub original: String,
    pub focus: bool,
    pub phonemes: bool,
}
#[derive(Clone, Copy)]
pub enum NoteDrag {
    Move,
    Left,
    Right,
}
pub enum Gesture {
    Rap(Box<crate::ui::lanes::RapGesture>),
    Phoneme(Box<crate::ui::phonemes::PhonemeGesture>),
    Curve(Box<crate::ui::lanes::CurveGesture>),
    Anchor(Box<pitch_anchor::AnchorGesture>),
    Pitch(Box<pitch_edit::PitchGesture>),
    Notes {
        origin: Pos2,
        notes: Vec<Note>,
        kind: NoteDrag,
        anchor: i64,
        movement: Option<Box<crate::editor::note_movement::NoteMovement>>,
    },
    Create {
        origin: Pos2,
        id: EntityId,
        onset: i64,
    },
    Select {
        origin: Pos2,
        initial: BTreeSet<EntityId>,
    },
    Parameter {
        lane: u64,
        time: i64,
    },
}
