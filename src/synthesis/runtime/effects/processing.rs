use super::*;
use crate::synthesis::audio::ChannelLayout;

pub(super) fn tail(configuration:&Configuration)->usize {
    (24000+configuration.reverb.map_or(0,|p|(p.decay*48000.0)as usize)).min(960000)
}
impl Effects {
    pub(super) fn poll(&mut self,ctx:&eframe::egui::Context) {
        if let Some(job)=&self.job {
            let mut done=false;
            let track=self.tracks.get_mut(&job.track).expect("retained effect job");
            loop {
                let event=match job.events.try_recv(){Ok(v)=>v,Err(mpsc::TryRecvError::Empty)=>break,
                    Err(mpsc::TryRecvError::Disconnected)=>Event::Failed("Track effect worker stopped".into())};
                match event {
                    Event::Audio(audio)=>{
                        let ready=(audio.end_seconds()*48000.0).round()as usize;
                        let layer=mixer::morph_layer(job.track,track.gains,audio);
                        track.parts.push(layer.clone());track.layer.publish([layer],ready);
                    },
                    Event::Done(processor,cursor)=>{track.processor=Some(processor);track.cursor=cursor;done=true;break;},
                    Event::Failed(error)=>{track.error=Some(error);track.parts.clear();track.layer.reset(0..0);done=true;break;},
                }
            }
            if done {self.job=None;}
        }
        if self.job.is_some(){return;}
        let candidate=self.tracks.iter().filter(|(_,t)|t.error.is_none()&&t.cursor<t.range.end)
            .filter_map(|(&id,t)|{
                let mut until=t.range.end;
                for (source,spec) in &t.sources {
                    let input=self.inputs.get(source)?;
                    if let Some(missing)=input.ready.first_missing(spec.range.clone()){until=until.min(missing);}
                }
                if until<t.range.end {until=t.range.start+(until-t.range.start)/4096*4096;}
                until=until.min(t.cursor.saturating_add(65536));
                (until>t.cursor).then_some((id,t.cursor,until))
            }).min_by_key(|(_,cursor,_)|*cursor);
        let Some((id,start,end))=candidate else{return};
        let track=self.tracks.get_mut(&id).unwrap();
        let mut processor=track.processor.take().expect("idle effect state");
        let sources=track.sources.keys().map(|id|self.inputs[id].ready.clone()).collect::<Vec<_>>();
        let (tx,events)=mpsc::channel();let cancel=Arc::new(AtomicBool::new(false));let stop=cancel.clone();let wake=ctx.clone();
        std::thread::spawn(move||{
            let result=(||->Result<(),String>{
                let mut cursor=start;
                while cursor<end {
                    if stop.load(Ordering::Acquire){return Err("Track effects cancelled".into());}
                    let count=(end-cursor).min(4096);let range=cursor..cursor+count;
                    let mut primary=vec![[0.0;4];count];let mut aspiration=vec![[0.0;4];count];
                    for source in &sources {source.mix_stems(&range,&mut primary,&mut aspiration);}
                    processor.process_encoded(&mut primary,&mut aspiration);
                    let mut stereo=Vec::with_capacity(count*2);
                    for (p,a) in primary.into_iter().zip(aspiration) {
                        stereo.extend(processor.decode(p,a));
                    }
                    let audio=RenderedAudio::new(48000,cursor as f64/48000.0,ChannelLayout::Stereo,stereo).map_err(|e|e.to_string())?;
                    tx.send(Event::Audio(Arc::new(audio))).map_err(|_|"Track effects cancelled")?;
                    cursor+=count;wake.request_repaint();
                }
                Ok(())
            })();
            let event=match result {Ok(())=>Event::Done(processor,end),Err(error)=>Event::Failed(error)};
            let _=tx.send(event);wake.request_repaint();
        });
        self.job=Some(Job {track:id,cancel,events});
    }
}
