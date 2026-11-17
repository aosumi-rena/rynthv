use super::manager::State;
use crate::{
    app::App,
    synthesis::instruments::Entry,
    ui::{
        child,
        resource_manager::{self, ACCENT, INK},
        theme::*,
    },
};
use eframe::egui::*;
pub(super) fn show(app: &mut App, ui: &mut Ui, entry: &Entry, state: &mut State) {
    ui.push_id((&entry.selection.database_id, "instrument-product"), |ui| {
        resource_manager::product(ui, |ui| {
            let compact = ui.available_width() < 620.;
            let (rect, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), if compact { 152. } else { 88. }),
                Sense::hover(),
            );
            let art = Rect::from_min_size(
                rect.min + vec2(0., 4.),
                Vec2::splat(if compact { 60. } else { 80. }),
            );
            super::selector::artwork(ui, art, Some(entry));
            let origin = pos2(art.right() + 18., rect.top() + 3.);
            let end = rect.right() - if compact { 30. } else { 174. };
            let name = Rect::from_min_max(origin, pos2(end, origin.y + 26.));
            ui.painter()
                .with_clip_rect(name.intersect(ui.clip_rect()))
                .text(
                    name.min,
                    Align2::LEFT_TOP,
                    crate::i18n::t(&entry.name),
                    FontId::new(if compact { 17. } else { 20. }, section_title_font().family),
                    INK,
                );
            let mut meta = child(
                ui,
                Rect::from_min_max(origin + vec2(0., 49.), pos2(end, origin.y + 73.)),
                ui.id().with("instrument-revision"),
            );
            ui.painter()
                .with_clip_rect(
                    Rect::from_min_max(origin + vec2(0., 28.), pos2(end, origin.y + 46.))
                        .intersect(ui.clip_rect()),
                )
                .text(
                    origin + vec2(0., 28.),
                    Align2::LEFT_TOP,
                    &entry.vendor,
                    FontId::proportional(12.),
                    MUTED,
                );
            meta.spacing_mut().interact_size.y = 22.;
            meta.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(10);
            meta.visuals_mut().widgets.inactive.bg_fill = BUTTON;
            meta.visuals_mut().widgets.inactive.weak_bg_fill = BUTTON;
            let mut revision = entry.selection.revision.clone();
            ComboBox::from_id_salt("version")
                .width(90.)
                .selected_text(format!("v{}", revision))
                .show_ui(&mut meta, |ui| {
                    for e in &app.synthesis.instruments.catalog.entries {
                        if e.selection.database_id == entry.selection.database_id {
                            ui.selectable_value(
                                &mut revision,
                                e.selection.revision.clone(),
                                format!("v{}", e.selection.revision),
                            );
                        }
                    }
                });
            let changed = revision != entry.selection.revision;
            if changed {
                state
                    .versions
                    .insert(entry.selection.database_id.clone(), revision);
            }
            let tags = Rect::from_min_max(
                if compact {
                    rect.min + vec2(0., 78.)
                } else {
                    origin + vec2(110., 49.)
                },
                pos2(
                    end,
                    if compact {
                        rect.top() + 100.
                    } else {
                        rect.bottom()
                    },
                ),
            );
            let label = entry
                .room
                .as_deref()
                .map(|room| super::native_label(&format!("Mic_{room}")).to_owned())
                .unwrap_or_else(|| crate::i18n::t("Instrument").to_owned());
            let galley = ui
                .painter()
                .layout_no_wrap(label, FontId::proportional(11.), TEXT);
            let badge = Rect::from_min_size(
                tags.min,
                vec2((galley.size().x + 18.).min(tags.width()), 22.),
            );
            ui.painter()
                .rect_filled(badge, 11, Color32::from_rgb(65, 65, 65));
            ui.painter().with_clip_rect(badge).galley(
                badge.left_center() + vec2(9., -galley.size().y / 2.),
                galley,
                TEXT,
            );
            let current = super::current(app).is_some_and(|e| e.selection == entry.selection);
            let available = entry.auxiliary.is_some();
            let label = crate::i18n::t(if !available {
                "加载失败"
            } else if current {
                "当前音轨"
            } else {
                "可加载"
            });
            let status = if compact {
                Rect::from_min_max(
                    pos2(rect.left(), rect.bottom() - 34.),
                    pos2(rect.right() - 162., rect.bottom()),
                )
            } else {
                Rect::from_min_max(
                    pos2(rect.right() - 160., rect.top()),
                    pos2(rect.right() - 32., rect.top() + 28.),
                )
            };
            ui.painter().text(
                if compact {
                    status.left_center()
                } else {
                    status.right_center()
                },
                if compact {
                    Align2::LEFT_CENTER
                } else {
                    Align2::RIGHT_CENTER
                },
                label,
                FontId::proportional(11.),
                if !available { Color32::from_rgb(218, 142, 126) } else if current { ACCENT } else { MUTED },
            );
            let menu_rect = Rect::from_min_size(rect.right_top() - vec2(24., 2.), Vec2::splat(26.));
            let mut menu = child(ui, menu_rect, ui.id().with("instrument-actions"));
            resource_manager::more(&mut menu, crate::i18n::t("Manage Instruments"), |ui| {
                resource_manager::details_action(ui, &mut state.expanded, &entry.selection.database_id);
            });
            let action_rect = Rect::from_min_max(rect.max - vec2(146., 36.), rect.max);
            let mut action = child(ui, action_rect, ui.id().with("load-instrument"));
            let enabled = !changed && (!available || (!current && app.editor.current_track().is_instrument()));
            let button = resource_manager::action_button(
                crate::i18n::t(if !available {
                    "处理问题"
                } else if current {
                    "当前音轨"
                } else {
                    "用于当前音轨"
                }),
                enabled,
                current || !available,
            );
            if action
                .add_enabled(
                    enabled,
                    button.min_size(vec2(146., 36.)),
                )
                .clicked()
            {
                if available {
                    app.editor.select_instrument(entry);
                } else {
                    state.expanded = Some(entry.selection.database_id.clone());
                }
            }
            if state.expanded.as_deref() == Some(&entry.selection.database_id) {
                ui.add_space(16.);
                ui.separator();
                ui.add_space(10.);
                if !available {
                    resource_manager::error_label(ui, crate::i18n::t("合成环境准备失败"));
                    ui.collapsing(crate::i18n::t("查看错误详情"), |ui| {
                        resource_manager::error_label(ui, "Instrument auxiliary model is not installed in the selected version directory");
                    });
                }
                ui.label(crate::i18n::t("Instrument Resources"))
                    .on_hover_text(entry.main.display().to_string());
                if let Some([lo, hi]) = entry.pitch_range {
                    ui.weak(format!("MIDI {lo}–{hi}"));
                }
                super::settings::version(app, ui, entry);
            }
        })
    });
}
