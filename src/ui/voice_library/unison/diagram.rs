use crate::{synthesis::{unison::{UnisonSettings,SeatingPlan},voice::Speaker},ui::icons::Icon};
use eframe::egui::*;

pub(super) fn show(ui:&mut Ui,settings:&UnisonSettings,plan:&SeatingPlan,speakers:&[Speaker],accent:Color32)->bool {
    let (rect,_)=ui.allocate_exact_size(vec2(ui.available_width(),97.0),Sense::hover());
    let painter=ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let background=Color32::from_rgb(51,49,52);
    painter.rect_filled(rect,4.0,Color32::from_white_alpha(8));
    let pixel=1.0/ui.pixels_per_point();
    painter.rect_stroke(rect.shrink(pixel*0.5),4.0,Stroke::new(pixel,Color32::from_white_alpha(18)),StrokeKind::Middle);

    for row in (0..plan.rows.len()).rev() {
        let y=rect.center().y-(row as f32-(plan.rows.len()-1) as f32*0.5)*18.0;
        let columns=&plan.rows[row];
        for (column,&index) in columns.iter().enumerate() {
            let x=rect.center().x+(column as f32-(columns.len()-1) as f32*0.5)*settings.spacing*16.0/0.7;
            let outside=plan.count>0 && (index<plan.first||index>=plan.first+plan.count);
            let body_alpha:u8=if outside {26}else{255};
            let head_alpha=if outside {26}else if speakers.is_empty(){255}else{
                let alpha=plan.speakers[index].map_or(1.0,|id|((id+3)%4) as f32/6.0+0.5);
                (alpha.clamp(0.0,1.0)*255.0).round_ties_even() as u8
            };
            let color=|alpha|Color32::from_rgba_unmultiplied(accent.r(),accent.g(),accent.b(),alpha);
            let head=Rect::from_min_size(pos2(x-5.0,y-13.0),vec2(10.0,10.0));
            let body=Rect::from_min_size(head.min+vec2(0.0,10.0),vec2(10.0,15.0));
            let top_color=color((f32::from(body_alpha)*0.75).round_ties_even() as u8);
            let mut mesh=Mesh::default();
            for (point,color) in [(body.left_top(),top_color),(body.right_top(),top_color),
                (body.left_bottom(),Color32::TRANSPARENT),(body.right_bottom(),Color32::TRANSPARENT)] {
                mesh.colored_vertex(point,color);
            }
            mesh.add_triangle(0,1,2);mesh.add_triangle(1,3,2);painter.add(Shape::mesh(mesh));
            let start=2.356194496154785_f32;
            let end=3.9269909858703613_f32;
            let point=|angle:f32|head.center()+vec2(angle.sin()*6.0,-angle.cos()*6.0);
            let mut arc=vec![point(start)];
            let mut angle=start+0.05000000074505806_f32;
            while angle<end {arc.push(point(angle));angle+=0.05000000074505806_f32;}
            arc.push(point(end));
            painter.add(Shape::line(arc,Stroke::new(2.0,background)));
            painter.circle_filled(head.center(),5.0,color(head_alpha));
        }
    }
    if speakers.is_empty(){return false}
    let icon=Rect::from_min_size(rect.right_top()+vec2(-25.0,5.0),vec2(20.0,20.0));
    let response=ui.interact(icon,ui.id().with("shuffle-seating"),Sense::click());
    Icon::ChoirShuffle.paint(ui, icon.shrink(2.0), if response.hovered(){Color32::from_rgb(188,182,191)}else{Color32::from_rgb(163,157,166)});
    response.on_hover_ui(|ui| {
        ui.strong(crate::i18n::t("轮换成员"));
        ui.label(crate::i18n::t("点击以对合唱团成员及站位进行轮换，可创造多样的声音质感。"));
    }).clicked()
}
