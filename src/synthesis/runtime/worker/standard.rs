use std::{sync::{Arc,atomic::{AtomicBool,Ordering},mpsc::{Receiver,RecvTimeoutError}},time::{Instant,Duration}};
use super::{Command,Event,state::WorkState};
use crate::synthesis::{voice::VoiceInventory,runtime::{VoiceState,events::Events,control::Control},render_cache::Completion,rendered::PhonemeSnapshot,execution,engine::standard::StandardRenderer};

pub(super) fn run(inventory:VoiceInventory,commands:Receiver<Command>,events:Events,alive:Arc<AtomicBool>,control:Arc<Control>,started:Instant) {
    let descriptor=inventory.standard.as_ref().expect("STD inventory").clone();
    if !alive.load(Ordering::Acquire) { return; }
    let mut renderer=match StandardRenderer::from_voice(descriptor) {Ok(renderer)=>renderer,Err(error)=>{let _=events.send(Event::Voice {state:VoiceState::Failed(error.to_string()),elapsed:started.elapsed()});return;}};
    if !alive.load(Ordering::Acquire) { return; }
    if events.send(Event::Voice {state:VoiceState::Loaded(Box::new(inventory)),elapsed:started.elapsed()}).is_err(){return;}
    let _=events.send(Event::Ready);
    let mut state=WorkState::new();
    while alive.load(Ordering::Acquire) {
        state.drain(&commands);state.cache.reuse_queued();state.publish(&events);
        let Some((owner,_))=state.next_work()else{
            match commands.recv_timeout(Duration::from_millis(100)){Ok(command)=>state.apply(command),Err(RecvTimeoutError::Timeout)=>{},Err(RecvTimeoutError::Disconnected)=>break}
            continue;
        };
        let Some(job)=state.cache.take_owner(owner)else{continue;};
        let _active=control.register(&job,state.reset_generation);
        let _=events.send(Event::Progress {revision:state.revision,owner,stage:crate::i18n::t("正在合成歌声").into()});
        let rendered=execution::scope(job.cancellation(),||{
            execution::checkpoint()?;
            let mut emit=|event| {
                state.drain(&commands);
                execution::checkpoint()?;
                use crate::synthesis::engine::standard::StreamEvent as E;
                let mirrors=super::state::equivalent_owners(&state.plan,owner);
                match event {
                    E::Stage(stage)=>{let _=events.send(Event::Progress {revision:state.revision,owner,stage:stage.label().into()});},
                    E::Prepared {phones,model}=>{
                        if let Some(live)=&mut state.live {for &mirror in &mirrors {
                            live.prepared(mirror,model.clone()).map_err(crate::synthesis::voice::VoiceError::new)?;
                        }}
                        for &mirror in &mirrors {let _=events.send(Event::Phonemes {revision:state.revision,owner:mirror,
                            snapshot:PhonemeSnapshot {input:Arc::clone(&job.request.input),phones:phones.clone()}});}
                    },
                    E::Audio {audio,visuals}=>{
                        if let Some(live)=&mut state.live {live.publish_for(&mirrors,Arc::clone(&audio)).map_err(crate::synthesis::voice::VoiceError::new)?;}
                        let input=&job.request.input;
                        let range=input.tempo.position_at(audio.start_seconds()).0..input.tempo.position_at(audio.end_seconds()).0;
                        let patches=mirrors.iter().map(|&mirror|(mirror,crate::synthesis::rendered::VisualPatch {
                            input:Arc::clone(input),range:range.clone(),data:Some(Arc::clone(&visuals)),highlight:true,
                        })).collect();
                        let _=events.send(Event::VisualPatches {revision:state.revision,patches});
                    },
                }
                Ok(())
            };
            renderer.render_stream(&job.request.input,job.cancellation_flag(),&mut emit)
        });
        state.drain(&commands);
        if !job.is_cancelled() {
            let phones=rendered.as_ref().ok().map(|clip|clip.visuals.phonemes.clone());
            match state.cache.finish(&job,rendered.map_err(|error|error.to_string())) {
                Completion::Published=>{if let Some(phones)=phones {
                    let _=events.send(Event::Phonemes {revision:state.revision,owner,snapshot:PhonemeSnapshot {input:Arc::clone(&job.request.input),phones}});
                }},
                Completion::Failed(error)=>{state.rebuild_source=true;let _=events.send(Event::Error {revision:Some(state.revision),error:crate::lformat!("音符组合成失败：{error}")});},
                Completion::Discarded=>{},
            }
        }
        if job.is_cancelled() {state.rebuild_source=true;}
        state.publish(&events);
    }
}
