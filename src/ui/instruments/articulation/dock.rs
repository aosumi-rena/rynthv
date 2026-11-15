use super::*;

pub fn toolbar(app: &mut App, ui: &mut Ui, canvas: Rect) {
    if canvas.width() < 80.0 || canvas.height() < 48.0 { return; }
    let size = vec2(452.0_f32.min(canvas.width()-8.0), 36.0);
    let travel = (canvas.size()-size-vec2(8.0,8.0)).max(Vec2::ZERO);
    let position = app.view.instrument_dock;
    let origin = canvas.min+vec2(4.0+travel.x*position[0],4.0+travel.y*position[1]);
    let color = accent(ui);
    Area::new(Id::new("instrument-articulation-toolbar"))
        .order(Order::Middle).movable(false).fixed_pos(origin)
        .constrain_to(canvas).show(ui.ctx(), |ui| {
            Frame::new().fill(BUTTON).corner_radius(5).stroke(Stroke::new(1.0,BUTTON_HOVER))
                .inner_margin(3).show(ui, |ui| {
                    ui.set_width(size.x-6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x=2.0;
                        let grip_width=16.0;
                        ScrollArea::horizontal().id_salt("articulation-buttons")
                            .max_width((size.x-8.0-grip_width).max(20.0))
                            .auto_shrink([false,true])
                            .show(ui, |ui| ui.horizontal(|ui| {
                                let button=|icon,label|IconButton::new(icon,label).size(vec2(30.0,30.0)).icon_size(24.0).tint(color);
                                let can_edit=!app.editor.is_editing()&&!app.editor.selected.is_empty();
                                if ui.add_enabled(can_edit,button(Icon::Reset,crate::i18n::t("Clear"))).clicked() {
                                    super::super::controls::edit_notes(app,crate::i18n::t("Articulation"),|n| {
                                        n.attributes.remove("articulations");
                                        n.attributes.insert("articulationsFixed".into(),true.into());
                                    });
                                }
                                if ui.add_enabled(super::super::smart::can_toggle(app),button(Icon::SmartArticulation,
                                    crate::i18n::t("Smart Articulations")).selected(super::super::smart::chosen(app))).clicked() {
                                    super::super::smart::toggle(app);
                                }
                                for (name,icon,_) in ITEMS {
                                    let response=ui.add_enabled(enabled(app,name),button(icon,label(name)).selected(chosen(app,name)));
                                    if matches!(name,"Staccato"|"Trill") {
                                        ui.painter().vline(response.rect.left()-1.0,
                                            response.rect.top()..=response.rect.bottom(),Stroke::new(0.5,BORDER));
                                    }
                                    if response.clicked() {
                                        toggle(app,name);
                                    }
                                }
                                ui.visuals_mut().override_text_color=Some(color);
                                interval(app,ui);
                            }));
                        let (rect,response)=ui.allocate_exact_size(vec2(grip_width,30.0),Sense::drag());
                        ui.painter().vline(rect.left()-1.0,rect.top()+2.0..=rect.bottom()-2.0,
                            Stroke::new(0.5,BORDER));
                        for y in [-4.0,0.0,4.0] { for x in [-2.0,2.0] {
                            ui.painter().rect_filled(Rect::from_center_size(rect.center()+vec2(x,y),vec2(2.5,2.5)),0,MUTED);
                        }}
                        let drag_id=Id::new("instrument-dock-drag");
                        if response.drag_started() && let Some(start)=ui.input(|i|i.pointer.press_origin()) {
                            ui.ctx().data_mut(|d|d.insert_temp(drag_id,(position,start)));
                        }
                        if (response.dragged() || response.drag_stopped())
                            && let Some((initial,start))=ui.ctx().data(|d|d.get_temp::<([f32;2],Pos2)>(drag_id)) {
                            let pointer=ui.input(|i|i.events.iter().find_map(|event|match event {
                                Event::PointerButton {pos,button:PointerButton::Primary,pressed:false,..}=>Some(*pos),
                                _=>None,
                            }).or_else(||i.pointer.interact_pos())).unwrap_or(start);
                            let delta=pointer-start;
                            for (i,d) in [delta.x,delta.y].into_iter().enumerate() {
                                let span=if i==0 {travel.x}else{travel.y};
                                if span>0.0 {app.view.instrument_dock[i]=(initial[i]+d/span).clamp(0.0,1.0);}
                            }
                        }
                        if response.drag_stopped(){ui.ctx().data_mut(|d|d.remove::<([f32;2],Pos2)>(drag_id));}
                        response.on_hover_cursor(CursorIcon::Grab);
                    });
                });
        });
}
