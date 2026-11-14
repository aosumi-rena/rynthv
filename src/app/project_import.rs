use super::{App,MidiImportMode};
use crate::{editor::Editor,ui};
use eframe::egui::Context;
use rynthv::project_import::Imported;
use std::{path::PathBuf,sync::mpsc::{self,Receiver,TryRecvError}};

pub(super) struct Import {
    source:PathBuf,
    mode:MidiImportMode,
    result:Receiver<Result<(Imported,Option<PathBuf>),String>>,
    revision:u64,
}
impl App {
    pub(super) fn begin_project_import(&mut self,path:PathBuf,mode:MidiImportMode,ctx:&Context) {
        if self.project_import.is_some(){self.error=Some(crate::i18n::t("已有工程正在导入。").into());return;}
        let (tx,rx)=mpsc::channel();let source=path.clone();let wake=ctx.clone();
        match std::thread::Builder::new().name("project-import".into()).spawn(move||{
            let _=tx.send(rynthv::project_import::convert(&source).map(|p|(p,None)));
            wake.request_repaint();
        }) {
            Ok(_)=>{self.project_import=Some(Import{source:path,mode,result:rx,revision:self.editor.revision});self.status=crate::i18n::t("正在导入工程…").into();}
            Err(e)=>self.error=Some(crate::lformat!("无法启动工程导入：{e}")),
        }
    }
    pub(super) fn poll_project_import(&mut self,ctx:&Context) {
        let Some(import)=self.project_import.take() else{return;};
        let result=match import.result.try_recv(){
            Ok(result)=>result,
            Err(TryRecvError::Empty)=>{self.project_import=Some(import);return;}
            Err(TryRecvError::Disconnected)=>Err(crate::i18n::t("工程导入线程意外停止。").into()),
        };
        let (mut converted,path)=match result {Ok(r)=>r,Err(e)=>{self.status.clear();self.error=Some(e);return;}};
        if matches!(import.mode,MidiImportMode::NewProject) && self.editor.revision!=import.revision {
            self.error=Some(crate::i18n::t("导入期间当前工程已更改，已保留当前工程。请保存后重新打开要导入的文件。").into());return;
        }
        if !converted.assets.is_empty() {
            let Some(destination)=rfd::FileDialog::new().set_title(crate::i18n::t("选择导入工程与音频的保存文件夹")).pick_folder() else {
                self.status=crate::i18n::t("已取消导入").into();return;
            };
            let (tx,rx)=mpsc::channel();let wake=ctx.clone();
            match std::thread::Builder::new().name("project-audio-extract".into()).spawn(move||{
                let result=(||{
                    let folder=converted.extract(&destination)?;
                    let stem=converted.source.file_stem().unwrap_or_default();
                    let path=folder.join(format!("{}.rvp",stem.to_string_lossy()));
                    crate::storage::save(&path,&converted.project)?;
                    Ok((converted,Some(path)))
                })();
                let _=tx.send(result);wake.request_repaint();
            }) {
                Ok(_)=>self.project_import=Some(Import{result:rx,..import}),
                Err(e)=>self.error=Some(crate::lformat!("无法启动音频解压：{e}")),
            }
            return;
        }
        match import.mode {
            MidiImportMode::NewProject=>{
                self.synthesis.reset_project();
                self.editor=Editor::new(converted.project);
                self.restore_transport_choices();
                self.path=path;
                if self.path.is_none(){self.editor.mark_imported();}
                self.gesture=None;self.group_gesture=None;self.audio_drag=None;self.audio_import=None;
                self.piano_anchor=ui::piano::AnchorSelection::default();
                self.view_pan=ui::view_pan::ViewPan::default();
                self.rename=None;self.time_edit=None;self.bpm_edit=None;self.lyric=None;self.fill_lyrics=None;
            }
            MidiImportMode::Tracks=>self.editor.import_project_tracks(converted.project),
        }

        self.piano_collapsed=false;self.view.arrangement_collapsed=false;
        if let Some(first)=self.editor.notes().first() {
            let reference=self.editor.active_ref();
            let offset=reference.map_or(0,|r|r.blick_offset);
            let pitch=first.pitch+reference.map_or(0,|r|r.pitch_offset);
            let start=crate::model::beats(first.onset+offset).floor().max(0.0);
            self.view.piano.start=start;self.view.arrangement.start=start;
            self.view.top_pitch=(pitch+9).clamp(12,127) as f32;
        }else if let Some(clip)=self.editor.current_track().audio_clip() {
            self.view.arrangement.start=crate::model::beats(self.editor.project.time.position_at(clip.onset_seconds)).max(0.0);
        }
        self.status=crate::lformat!("已导入 {}",import.source.file_name().unwrap_or_default().to_string_lossy());
    }
}
