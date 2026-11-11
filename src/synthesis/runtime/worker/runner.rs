use super::{
    Command, Event,
    state::{Session, WorkState},
};
use crate::synthesis::{
    engine::{self, RenderStage, progressive},
    execution,
    plan::OwnerId,
    render_cache::{Completion, RenderJob},
    rendered::RenderedClip,
    runtime::{VoiceState, control::Control, events::Events},
    score::{self, LyricFrontend},
    voice::{self, PreparedVoice, VoiceError, VoiceInventory},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

pub(super) fn run(
    path: PathBuf,
    dictionaries: PathBuf,
    phone_conversion: Option<PathBuf>,
    duration_key_override: Option<u64>,
    commands: Receiver<Command>,
    events: Events,
    alive: Arc<AtomicBool>,
    control: Arc<Control>,
) {
    if !alive.load(Ordering::Acquire) {
        return;
    }
    let loading_checkpoint = || {
        if alive.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(VoiceError::new("Voice loading cancelled"))
        }
    };
    let started = Instant::now();
    let inventory=match VoiceInventory::load(&path) {
        Ok(inventory)=>inventory,
        Err(error)=>{
            let _=events.send(Event::Voice {state:VoiceState::Failed(error.to_string()),elapsed:started.elapsed()});
            return;
        },
    };
    if inventory.standard.is_some(){
        super::standard::run(inventory,commands,events,alive,control,started);
        return;
    }
    let loaded = Ok::<_,VoiceError>(inventory).and_then(|inventory| {
        loading_checkpoint()?;
        let prepared = inventory.prepare()?;
        loading_checkpoint()?;
        let (name, version) = inventory
            .configuration
            .as_ref()
            .map(|config| (config.name.clone(), config.version.clone()))
            .ok_or_else(|| VoiceError::new("声库缺少合成配置。"))?;
        Ok((inventory, prepared, name, version))
    });
    if !alive.load(Ordering::Acquire) {
        return;
    }
    let (mut inventory, voice, name, version) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = events.send(Event::Voice {
                state: VoiceState::Failed(error.to_string()),
                elapsed: started.elapsed(),
            });
            return;
        }
    };
    let database_id = inventory.database_id.clone();
    let frontend = inventory
        .configuration
        .as_ref()
        .ok_or_else(|| VoiceError::new("声库缺少语言配置"))
        .and_then(|config| {
            let frontend=LyricFrontend::for_voice_with_conversion(&dictionaries,config,phone_conversion.as_deref())?;
            Ok(frontend)
        });
    if let Ok(frontend)=&frontend && let Some(identity)=frontend.conversion_identity() {
        inventory.source_identity.push_str(":phone-conversion:");
        inventory.source_identity.push_str(identity);
    }
    if events
        .send(Event::Voice {
            state: VoiceState::Loaded(Box::new(inventory)),
            elapsed: started.elapsed(),
        })
        .is_err()
        || !alive.load(Ordering::Acquire)
    {
        return;
    }
    let mut context_required = false;
    let context = frontend.and_then(|lexicon| {
        loading_checkpoint()?;
        let identity = if voice.family.requires_identity() {
            Some(match duration_key_override {
                Some(value) => value,
                None =>
                voice::session_identity::saved_identity(&name, voice.family.model_version())
                    .inspect_err(|_| context_required = true)?,
            })
        } else {
            None
        };
        loading_checkpoint()?;
        let network = progressive::Network::prepare(&voice)?;
        Ok((lexicon, identity, network))
    });
    if !alive.load(Ordering::Acquire) {
        return;
    }
    let (lexicon, identity, network) = match context {
        Ok(context) => context,
        Err(error) => {
            let error = format!("合成环境准备失败：{error}");
            let event = if context_required {
                Event::ContextRequired { error }
            } else {
                Event::Error {
                    revision: None,
                    error,
                }
            };
            let _ = events.send(event);
            return;
        }
    };
    let _ = events.send(Event::Ready);
    let mut state = WorkState::new();
    while alive.load(Ordering::Acquire) {
        state.drain(&commands);
        state.cache.reuse_queued();
        state.publish(&events);
        let Some((owner, target)) = state.next_work() else {
            match commands.recv_timeout(Duration::from_millis(100)) {
                Ok(command) => state.apply(command),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            continue;
        };
        if !state.sessions.contains_key(&owner) {
            if let Some(job) = state.cache.take_owner(owner) {
                let _active = control.register(&job, state.reset_generation);
                let result = execution::scope(job.cancellation(), || {
                    execution::checkpoint()?;
                    score::validate_voice(&job.request.input, &name, &version, &database_id)?;
                    progressive::prepare(
                        &voice,
                        identity,
                        &job.request.input,
                        &lexicon,
                        job.base.as_deref(),
                        |stage| checkpoint(&mut state, &commands, &events, owner, stage),
                    )
                });
                state.drain(&commands);
                if !job.is_cancelled() {
                    match result {
                        Ok(prepared) => {
                            match Session::new(job.clone(), prepared, state.seek_generation) {
                                Ok(session) => {
                                    state.sessions.insert(owner, session);
                                    if let Err(error) = state.publish_prepared(owner, &events) {
                                        state.sessions.remove(&owner);
                                        finish_error(&mut state, &events, &job, error);
                                    }
                                }
                                Err(error) => {
                                    finish_error(&mut state, &events, &job, error.to_string())
                                }
                            }
                        }
                        Err(error) => finish_error(&mut state, &events, &job, error.to_string()),
                    }
                }
            }
        } else {
            advance(
                &mut state, &network, &voice, &commands, &events, &control, owner, target,
            );
        }
        state.publish(&events);
    }
}

#[allow(clippy::too_many_arguments)]
fn advance<'network, 'tree>(
    state: &mut WorkState<'network, 'tree>,
    network: &'network progressive::Network<'tree>,
    voice: &'tree PreparedVoice,
    commands: &Receiver<Command>,
    events: &Events,
    control: &Control,
    owner: OwnerId,
    target: f64,
) {
    let Some(mut session) = state.sessions.remove(&owner) else {
        return;
    };
    let _active = control.register(&session.job, state.reset_generation);
    let result = execution::scope(session.job.cancellation(), || {
        execution::checkpoint()?;
        if session.seek_generation != state.seek_generation {
            session.seek_generation = state.seek_generation;
            if session.prepared.needs_audio(target) {
                session.cursor = None;
            }
        }
        if session.prepared.needs_audio(target)
            && session
                .cursor
                .as_ref()
                .is_some_and(|cursor| !cursor.covers_demand(target))
        {

            session.cursor = None;
        }
        if session
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.is_finished())
        {
            session.cursor = None;
        }
        if session.cursor.is_none() {
            session.cursor = session.prepared.start(network, voice, target)?;
        }
        match session.cursor.as_mut() {
            Some(cursor) => session.prepared.advance(cursor, 128, |stage| {
                checkpoint(state, commands, events, owner, stage)
            }),
            None if session.prepared.is_complete() => Ok(Vec::new()),
            None => Err(VoiceError::new("未完成的音符组没有可执行的音频区间")),
        }
    });

    state.drain(commands);
    if session.job.is_cancelled() {
        return;
    }
    match result {
        Ok(chunks) => {
            if let Err(error) = state.publish_chunks(&mut session, chunks, events) {
                finish_error(state, events, &session.job, error);
            } else if session.prepared.is_complete()
                && session
                    .cursor
                    .as_ref()
                    .is_none_or(|cursor| cursor.is_finished())
            {
                finish(state, events, &session.job, session.prepared.finish());
            } else {
                state.sessions.insert(owner, session);
            }
        }
        Err(error) => finish_error(state, events, &session.job, error.to_string()),
    }
}

fn checkpoint(
    state: &mut WorkState<'_, '_>,
    commands: &Receiver<Command>,
    events: &Events,
    owner: OwnerId,
    stage: RenderStage,
) -> voice::Result<()> {
    state.drain(commands);
    execution::checkpoint()?;
    let _ = events.send(Event::Progress {
        revision: state.revision,
        owner,
        stage: stage.label().into(),
    });
    Ok(())
}

fn finish(
    state: &mut WorkState<'_, '_>,
    events: &Events,
    job: &RenderJob,
    result: voice::Result<engine::RenderResult>,
) {
    let rendered = result
        .map(|result| {
            Arc::new(RenderedClip {
                audio: Arc::new(result.audio),
                visuals: Arc::new(result.visuals),
                state: Some(Arc::new(result.state)),
            })
        })
        .map_err(|error| error.to_string());
    if let Completion::Failed(error) = state.cache.finish(job, rendered) {
        let _ = events.send(Event::Error {
            revision: Some(state.revision),
            error: format!("音符组合成失败：{error}"),
        });
    }
}

fn finish_error(state: &mut WorkState<'_, '_>, events: &Events, job: &RenderJob, error: String) {
    if let Completion::Failed(error) = state.cache.finish(job, Err(error)) {
        let _ = events.send(Event::Error {
            revision: Some(state.revision),
            error: format!("音符组合成失败：{error}"),
        });
    }
}
