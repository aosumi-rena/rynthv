use crate::{i18n::t, model::RocoflexTrack, synthesis::{settings::VoiceSelection,
    voice::{VoiceArtwork, VoiceCatalog}}};
use super::{icons::{IconButton, Icon}, theme::{BACKGROUND, BORDER, PANEL}};
use eframe::egui::*;
use std::path::PathBuf;

#[derive(Default)]
pub struct Draft {
    name: String,
    vendor: String,
    description: String,
    avatar: Option<PathBuf>,
    background: Option<PathBuf>,
    donor: Option<VoiceSelection>,
    export_target: usize,
    pub morph: Option<RocoflexTrack>,
}

#[derive(Default)]
pub struct VoiceFinetuningState {
    pub open: bool,
    drafts: Vec<Draft>,
    selected: usize,
}

const EXPORT_TARGETS: [&str; 7] = ["RFX", "R3 AI 2", "R3 AI", "R2 AI", "R2 AI Flat", "R2 STD", "R1 STD"];

pub fn show(ctx: &Context, state: &mut VoiceFinetuningState, catalog: &VoiceCatalog,
    mut canvas: impl FnMut(&mut Ui, usize, &mut Draft))
{
    if state.drafts.is_empty() { state.drafts.push(Draft::default()); }
    ctx.set_embed_viewports(false);
    ctx.show_viewport_immediate(ViewportId::from_hash_of("voice-lab"), ViewportBuilder::default()
        .with_title(t("VOICE_LAB")).with_inner_size(vec2(1250.0, 760.0))
        .with_min_inner_size(vec2(790.0, 550.0)), |ui, _| {
        if ui.input(|i| i.viewport().close_requested()) { state.open = false; return; }
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0, BACKGROUND);
        let body = full.shrink(12.0);
        let head = Rect::from_min_size(body.min, vec2(body.width(), 39.0));
        let mut header = super::child(ui, head, "voice-lab-draft-bar");
        header.horizontal_centered(|ui| {
            ui.label(RichText::new(t("VOICE_LAB_DRAFTS")).strong());
            ComboBox::from_id_salt("active-draft").width(210.0)
                .selected_text(draft_title(&state.drafts[state.selected], state.selected)).show_ui(ui, |ui| {
                for (index, draft) in state.drafts.iter().enumerate() {
                    ui.selectable_value(&mut state.selected, index, draft_title(draft, index));
                }
            });
            if ui.button(t("VOICE_LAB_NEW_DRAFT")).clicked() {
                state.selected = state.drafts.len();
                state.drafts.push(Draft::default());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let target = &mut state.drafts[state.selected].export_target;
                ui.add_enabled(false, Button::new(t("VOICE_LAB_EXPORT")))
                    .on_disabled_hover_text(t("VOICE_LAB_EXPORT_UNAVAILABLE"));
                ComboBox::from_id_salt("voice-export-format").width(100.0).selected_text(EXPORT_TARGETS[*target])
                    .show_ui(ui, |ui| for (index, title) in EXPORT_TARGETS.iter().enumerate() {
                        ui.selectable_value(target, index, *title);
                    });
            });
        });
        let split = body.left() + (body.width() * 0.27).clamp(245.0, 300.0);
        let top = head.bottom() + 8.0;
        let left = Rect::from_min_max(pos2(body.left(), top), pos2(split, body.bottom()));
        let right = Rect::from_min_max(pos2(split + 12.0, top), body.max);
        ui.painter().rect(left, 4, PANEL, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let draft = &mut state.drafts[state.selected];
        let mut sidebar = super::child(ui, left.shrink(12.0), "voice-lab-draft-details");
        let details = &mut sidebar;
        details.label(RichText::new(t("VOICE_LAB_DETAILS")).strong());
        details.add_space(6.0);
        Grid::new("voice-details").num_columns(2).spacing(vec2(12.0, 8.0)).show(details, |ui| {
            text_field(ui, "VOICE_LAB_NAME", &mut draft.name);
            text_field(ui, "VOICE_LAB_VENDOR", &mut draft.vendor);
            ui.label(t("VOICE_LAB_DESCRIPTION"));
            ui.add(TextEdit::multiline(&mut draft.description).desired_rows(2).desired_width(130.0));
            ui.end_row();
            artwork(ui, "VOICE_LAB_AVATAR", &mut draft.avatar);
            artwork(ui, "VOICE_LAB_BACKGROUND", &mut draft.background);
        });
        details.add_space(8.0);
        details.separator();
        donors(details, catalog, &mut draft.donor);
        canvas(&mut super::child(ui, right, "voice-lab-canvas"), state.selected, draft);
    });
}

fn draft_title(draft: &Draft, index: usize) -> String {
    if draft.name.trim().is_empty() { format!("{} {}", t("VOICE_LAB_UNNAMED"), index + 1) }
    else { draft.name.trim().to_owned() }
}

fn text_field(ui: &mut Ui, label: &str, value: &mut String) {
    let title = ui.label(t(label));
    ui.add(TextEdit::singleline(value).desired_width(130.0)).labelled_by(title.id);
    ui.end_row();
}

fn artwork(ui: &mut Ui, label: &str, path: &mut Option<PathBuf>) {
    ui.label(t(label));
    ui.horizontal(|ui| {
        if ui.button(t("VOICE_LAB_CHOOSE")).clicked()
            && let Some(file) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg"]).pick_file() { *path = Some(file); }
        if let Some(file) = path {
            let picture = VoiceArtwork::File { path: file.clone() };
            ui.add(super::voice_library::artwork::image(&picture).fit_to_exact_size(Vec2::splat(26.0)))
                .on_hover_text(file.file_name().unwrap_or(file.as_os_str()).to_string_lossy());
            if ui.add(IconButton::new(Icon::Close, t("VOICE_LAB_CLEAR_DONOR"))).clicked() { *path = None; }
        }
    });
    ui.end_row();
}

fn donors(ui: &mut Ui, catalog: &VoiceCatalog, donor: &mut Option<VoiceSelection>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("VOICE_LAB_DONOR")).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if donor.is_some() && ui.small_button(t("VOICE_LAB_CLEAR_DONOR")).clicked() { *donor = None; }
        });
    });
    super::voice_library::donor_picker(ui, catalog, donor);
}
