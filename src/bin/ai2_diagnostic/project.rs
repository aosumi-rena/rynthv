//! The production editor owner path, without creating an application window.
use rynthv::synthesis::{score::{ExplicitScore,PreparedPhrase,LyricFrontend},
    voice::{VoiceInventory,PreparedVoice},plan::Planner};
use std::path::Path;

pub(super) fn prepare(path:&Path,resources:&Path,inventory:&VoiceInventory,voice:&PreparedVoice,section:Option<usize>)
    -> Result<(ExplicitScore,PreparedPhrase),Box<dyn std::error::Error>> {
    let project=rynthv::storage::load(path)?;
    let mut planner=Planner::default();
    let plan=planner.update(&project,0,|_|Some(inventory.identity().to_owned()));
    if plan.requests.len()!=1 && section.is_none() {return Err("Project contains multiple sounding sections; choose --section INDEX in planner order".into());}
    let input=&plan.requests.values().nth(section.unwrap_or(0)).ok_or("Project section index is outside the render plan")?.input;
    let config=inventory.configuration.as_ref().ok_or("Voice metadata is unavailable")?;
    let lexicon=LyricFrontend::for_voice(resources,config)?;
    let tree=voice.model("duration")?;
    let score=ExplicitScore::from_owner(input,&lexicon,tree,voice.family)?;
    let phrase=rynthv::synthesis::score::prepare_owner(input,&lexicon,tree,voice.family)?;
    Ok((score,phrase))
}

pub(super) fn save_audio(path:&Path,voice:&PreparedVoice,phrase:&PreparedPhrase,
    output:&rynthv::synthesis::engine::AcousticRender)
    -> Result<f64,Box<dyn std::error::Error>> {
    let mut vocoder=rynthv::synthesis::stages::vocoder::VocoderStage::prepare(voice.model("vocoder")?)?;
    let origin=phrase.start_seconds+output.timbre.first_frame as f64*rynthv::synthesis::score::FRAME_SECONDS;
    if voice.family==rynthv::synthesis::voice::VoiceFamily::Sv2 {
        vocoder.set_editor_controls(&output.prepared.phrase,origin);
        let seeds=rynthv::synthesis::stages::vocoder::noise_seeds::for_phrase(&output.prepared.phrase,output.timbre.first_frame)?;
        vocoder.set_editor_seed_source(seeds.into());
    }
    if voice.family==rynthv::synthesis::voice::VoiceFamily::Sv2Compatible {vocoder.set_compatible_dispatch();}
    let pcm=vocoder.render_components(&output.timbre.acoustics,output.timbre.controls.as_ref())?;
    let pcm=rynthv::synthesis::engine::PcmComponents::new(origin,pcm)?;
    let audio=pcm.publish(phrase.visible_begin_seconds,phrase.visible_end_seconds)?;
    rynthv::synthesis::audio::save_wav(path,&audio)?;
    Ok(audio.start_seconds())
}
