use super::App;
use crate::{model::PitchControl,synthesis::voice::VoiceFamily};
use rynthv::clipboard::{self,Content};

impl App {
    pub(crate) fn paste_text(&mut self,text:&str,at:i64)->Result<usize,String> {
        self.paste_content(clipboard::read(text)?,at)
    }

    pub(crate) fn paste_content(&mut self,data:Content,at:i64)->Result<usize,String> {
        let runtime=&self.synthesis;
        self.editor.paste_content_checked(data,at,&|editor,data,at| {
            let point=data.pitch_controls.iter().any(|control|matches!(control,PitchControl::Point(_)));
            let has=|key:&str|data.parameters.get(key).is_some_and(|curve|!curve.points.is_empty());
            if !point&&!has("toneShift")&&!has("voicing") {return Ok(());}
            let Some(reference)=editor.paste_destination(data,at) else {return Ok(());};
            let selected=runtime.resolve_voice(editor.current_track(),reference)?;
            let family=selected.as_ref().and_then(|key|runtime.library.entry(key)).and_then(|entry|entry.family);
            if point&&family.is_some_and(|family|family!=VoiceFamily::Sv2) {
                return Err(crate::i18n::t("当前声库暂不支持音高控制点，请使用手绘音高模式").into());
            }
            if family.is_some_and(VoiceFamily::is_standard)&&has("toneShift") {
                return Err(crate::i18n::t("Tone shift is unavailable for this voice").into());
            }
            if family==Some(VoiceFamily::R1Std)&&has("voicing") {
                return Err(crate::i18n::t("Voicing is unavailable for this voice").into());
            }
            Ok(())
        })
    }
}
