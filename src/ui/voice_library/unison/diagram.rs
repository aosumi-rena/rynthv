use crate::{synthesis::{unison::{UnisonSettings,SeatingPlan},voice::Speaker},ui::theme::*};
use eframe::egui::*;

pub(super) fn show(ui:&mut Ui,settings:&UnisonSettings,plan:&SeatingPlan,speakers:&[Speaker],accent:Color32)->bool {
    let (rect,_)=ui.allocate_exact_size(vec2(ui.available_width(),97.0),Sense::hover());
    let painter=ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let background=Color32::from_rgb(51,49,52);
    painter.rect(rect,4.0,background,Stroke::new(0.5,BORDER),StrokeKind::Inside);

    for row in (0..plan.rows.len()).rev() {
        let y=rect.center().y-(row as f32-(plan.rows.len()-1) as f32*0.5)*18.0;
        let columns=&plan.rows[row];
        for (column,&index) in columns.iter().enumerate() {
            let x=rect.center().x+(column as f32-(columns.len()-1) as f32*0.5)*settings.spacing*16.0/0.7;
            let alpha=if index<plan.first||index>=plan.first+plan.count {0.25}
                else if speakers.is_empty() {1.0}
                else {plan.speakers[index].map_or(1.0,|id|((id+3)%4) as f32/6.0+0.5)};
            let color=accent.linear_multiply(alpha);
            let head=Rect::from_min_size(pos2(x-5.0,y-13.0),vec2(10.0,10.0));
            let body=Rect::from_min_size(head.min+vec2(0.0,10.0),vec2(10.0,15.0));
            let mut mesh=Mesh::default();
            for (point,color) in [(body.left_top(),color),(body.right_top(),color),
                (body.left_bottom(),Color32::TRANSPARENT),(body.right_bottom(),Color32::TRANSPARENT)] {
                mesh.colored_vertex(point,color);
            }
            mesh.add_triangle(0,1,2);mesh.add_triangle(1,3,2);painter.add(Shape::mesh(mesh));
            let arc:Vec<_>=(0..=16).map(|i| {let a=std::f32::consts::PI*i as f32/16.0;
                head.center()+vec2(a.cos(),a.sin())*5.0}).collect();
            painter.add(Shape::line(arc,Stroke::new(2.0,background)));
            painter.circle_filled(head.center(),5.0,color);
        }
    }
    if speakers.is_empty(){return false}
    let icon=Rect::from_min_size(rect.right_top()+vec2(-25.0,5.0),vec2(20.0,20.0));
    let response=ui.interact(icon,ui.id().with("shuffle-seating"),Sense::click());
    Image::new(include_image!("../../../../assets/icons/choir-shuffle.svg"))
        .tint(if response.hovered(){TEXT}else{MUTED}).paint_at(ui,icon.shrink(2.0));
    response.on_hover_ui(|ui| {
        ui.strong(crate::i18n::t("轮换成员"));
        ui.label(crate::i18n::t("点击以对合唱团成员及站位进行轮换，可创造多样的声音质感。"));
    }).clicked()
}
