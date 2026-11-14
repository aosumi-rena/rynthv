use super::theme::*;
use eframe::egui::*;

#[derive(Clone,Copy)]
pub enum Choice { Cancel, Discard, Save }

pub fn unsaved(ctx:&Context,title:&str)->Option<Choice> {
    let mut choice=None;
    let response=Modal::new(Id::new("unsaved"))
        .frame(Frame::NONE.fill(Color32::from_rgb(61,59,64))
            .corner_radius(4).stroke(Stroke::new(0.5,MUTED)))
        .show(ctx,|ui| {
            let (r,_)=ui.allocate_exact_size(vec2(420.0,172.0),Sense::hover());
            let p=ui.painter();
            p.rect_filled(Rect::from_min_size(r.min,vec2(40.0,172.0)),
                CornerRadius{nw:4,ne:0,sw:4,se:0},Color32::from_rgb(48,70,72));
            p.text(r.min+vec2(20.0,86.0),Align2::CENTER_CENTER,"?",
                FontId::proportional(29.0),Color32::from_rgb(44,157,166));
            p.text(r.min+vec2(64.0,35.0),Align2::LEFT_CENTER,title,
                FontId::new(16.0,FontFamily::Name("SectionTitle".into())),TEXT);
            p.text(r.min+vec2(64.0,70.0),Align2::LEFT_CENTER,crate::i18n::t("是否放弃未保存的更改？"),
                FontId::proportional(12.0),MUTED);
            for (i,label,color,fill,value) in [
                (0,crate::i18n::t("取消"),TEXT,BUTTON,Choice::Cancel),
                (1,crate::i18n::t("放弃更改"),Color32::from_rgb(255,91,38),BUTTON,Choice::Discard),
                (2,crate::i18n::t("保存"),Color32::from_rgb(30,30,30),Color32::from_rgb(116,204,20),Choice::Save),
            ] {
                let rect=Rect::from_min_size(r.min+vec2(80.0+i as f32*108.0,116.0),vec2(100.0,32.0));
                let mut button=super::child(ui,rect,("unsaved-choice",i));
                if button.add_sized(rect.size(),Button::new(RichText::new(label).font(FontId::new(14.0,FontFamily::Name("SectionTitle".into()))).color(color)).fill(fill).corner_radius(4)).clicked(){choice=Some(value);}
            }
        });
    if response.should_close() {choice=Some(Choice::Cancel);}
    choice
}

pub fn error(ctx:&Context,message:&str)->bool {
    notice(ctx,crate::i18n::t("操作未完成"),message)
}

pub fn notice(ctx:&Context,title:&str,message:&str)->bool {
    let message=crate::i18n::message(message);
    let mut accepted=false;
    let response=Modal::new(Id::new("error"))
        .frame(Frame::NONE.fill(Color32::from_rgb(61,59,64))
            .corner_radius(4).stroke(Stroke::new(0.5,MUTED)))
        .show(ctx,|ui|{
            let text=ui.painter().layout(message.into(),FontId::proportional(12.0),MUTED,336.0);
            let height=(text.size().y+130.0).max(172.0);
            let (r,_)=ui.allocate_exact_size(vec2(420.0,height),Sense::hover());
            let p=ui.painter();
            p.rect_filled(Rect::from_min_size(r.min,vec2(40.0,height)),
                CornerRadius{nw:4,ne:0,sw:4,se:0},Color32::from_rgb(115,66,49));
            p.text(pos2(r.left()+20.0,r.center().y),Align2::CENTER_CENTER,"!",
                FontId::proportional(29.0),Color32::from_rgb(255,91,38));
            p.text(r.min+vec2(64.0,35.0),Align2::LEFT_CENTER,title,
                FontId::new(16.0,FontFamily::Name("SectionTitle".into())),TEXT);
            p.galley(r.min+vec2(64.0,62.0),text,MUTED);
            let rect=Rect::from_min_size(pos2(r.left()+296.0,r.bottom()-56.0),vec2(100.0,32.0));
            let mut button=super::child(ui,rect,"error-acknowledgement");
            accepted=button.add_sized(rect.size(),Button::new(RichText::new(crate::i18n::t("确定")).font(FontId::new(14.0,FontFamily::Name("SectionTitle".into()))).color(Color32::from_rgb(30,30,30)))
                .fill(Color32::from_rgb(116,204,20)).corner_radius(4)).clicked();
        });
    accepted || response.should_close()
}
