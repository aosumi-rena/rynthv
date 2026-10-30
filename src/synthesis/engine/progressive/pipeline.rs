use crate::synthesis::{
    execution::{self, Cancellation},
    inference::Sequence,
    stages::vocoder::{VocoderOutput, VocoderStage},
    voice::{PreparedTree, Result, VoiceError},
};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_OUTSTANDING: usize = 2;
const WAIT: Duration = Duration::from_millis(10);

struct Input {
    sequence: usize,
    physical: Sequence,
    controls: Option<Sequence>,
    natural_final: bool,
}

pub(super) struct Output {
    sequence: usize,
    pub pcm: VocoderOutput,
    pub natural_final: bool,
    pub elapsed_ms: f64,
}

pub(super) struct Pipeline {
    input: Option<SyncSender<Input>>,
    output: Option<Receiver<Result<Output>>>,
    worker: Option<JoinHandle<()>>,
    cancellation: Cancellation,
    submitted: usize,
    received: usize,
    final_submitted: bool,
}

impl Pipeline {
    pub fn new(tree: Arc<PreparedTree>,effects:Option<crate::synthesis::stages::vocoder::effects::EffectTimeline>,compatible_dispatch:bool,native_dds:bool,seeds:Option<Arc<[u64]>>) -> Result<Self> {
        let cancellation = Cancellation::child(execution::current());
        let worker_cancellation = cancellation.clone();
        let (input, inputs) = mpsc::sync_channel(MAX_OUTSTANDING);

        let (outputs, output) = mpsc::channel();
        #[cfg(feature = "synth-profile")]
        let profile = crate::synthesis::profiling::parallel_work("pipeline.vocoder");
        let worker = thread::Builder::new()
            .name("rynthv-vocoder".into())
            .spawn(move || {
                execution::scope(worker_cancellation, || {
                    let result = run(
                        &tree,
                        &inputs,
                        &outputs,
                        effects,
                        compatible_dispatch,
                        native_dds,
                        seeds,
                        #[cfg(feature = "synth-profile")]
                        profile,
                    );
                    if let Err(error) = result {
                        let _ = outputs.send(Err(error));
                    }
                });
            })?;
        Ok(Self {
            input: Some(input),
            output: Some(output),
            worker: Some(worker),
            cancellation,
            submitted: 0,
            received: 0,
            final_submitted: false,
        })
    }

    pub fn can_submit(&self) -> bool {
        !self.final_submitted && self.submitted - self.received < MAX_OUTSTANDING
    }

    pub fn submit(
        &mut self,
        physical: Sequence,
        controls: Option<Sequence>,
        natural_final: bool,
    ) -> Result<()> {
        if !self.can_submit()
            || physical.frames > 128
            || physical.channels != 72
            || controls.as_ref().is_some_and(|controls| {
                controls.channels != 2 || physical.frames != controls.frames
            })
        {
            return Err(VoiceError::new("Invalid progressive vocoder submission"));
        }
        execution::checkpoint()?;
        let input = Input {
            sequence: self.submitted,
            physical,
            controls,
            natural_final,
        };
        match self.input.as_ref().expect("open pipeline").try_send(input) {
            Ok(()) => {
                self.submitted += 1;
                self.final_submitted = natural_final;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(VoiceError::new(
                "Progressive vocoder exceeded its queue budget",
            )),
            Err(TrySendError::Disconnected(_)) => {

                while self.poll()?.is_some() {}
                Err(VoiceError::new(
                    "Progressive vocoder worker stopped accepting input",
                ))
            }
        }
    }

    pub fn poll(&mut self) -> Result<Option<Output>> {
        let result = match self.output.as_ref().expect("open pipeline").try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => return Err(worker_stopped()),
        };
        self.accept(result).map(Some)
    }

    pub fn wait(&mut self) -> Result<Option<Output>> {
        let result = match self
            .output
            .as_ref()
            .expect("open pipeline")
            .recv_timeout(WAIT)
        {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(RecvTimeoutError::Disconnected) => return Err(worker_stopped()),
        };
        self.accept(result).map(Some)
    }

    fn accept(&mut self, result: Result<Output>) -> Result<Output> {
        let output = result?;
        if output.sequence != self.received || self.received >= self.submitted {
            return Err(VoiceError::new(
                "Progressive vocoder output is out of order",
            ));
        }
        self.received += 1;
        Ok(output)
    }

    pub fn stop(&mut self) {
        self.cancellation.cancel();
        self.input.take();
        self.output.take();
        if let Some(worker) = self.worker.take() {

            let _ = worker.join();
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

fn worker_stopped() -> VoiceError {
    VoiceError::new("Progressive vocoder worker exited before completing its output")
}

fn run(
    tree: &PreparedTree,
    inputs: &Receiver<Input>,
    outputs: &mpsc::Sender<Result<Output>>,
    effects:Option<crate::synthesis::stages::vocoder::effects::EffectTimeline>,
    compatible_dispatch:bool,
    native_dds:bool,
    seeds:Option<Arc<[u64]>>,
    #[cfg(feature = "synth-profile")] profile: crate::synthesis::profiling::ParallelWork,
) -> Result<()> {
    execution::checkpoint()?;
    let started = Instant::now();
    let mut stage=VocoderStage::prepare(tree)?;
    stage.effects=effects;
    if compatible_dispatch {stage.set_compatible_dispatch();}
    if native_dds {stage.set_native_dds();}
    if let Some(seeds)=seeds {stage.set_editor_seed_source(seeds);}
    let mut vocoder = stage.into_stream()?;
    let mut setup_ms = started.elapsed().as_secs_f64() * 1000.0;
    loop {
        execution::checkpoint()?;
        let input = match inputs.recv_timeout(WAIT) {
            Ok(input) => input,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        execution::checkpoint()?;
        let started = Instant::now();
        #[cfg(feature = "synth-profile")]
        let pcm = profile.measure(|| {
            vocoder.process(
                &input.physical,
                input.controls.as_ref(),
                input.natural_final,
            )
        })?;
        #[cfg(not(feature = "synth-profile"))]
        let pcm = vocoder.process(
            &input.physical,
            input.controls.as_ref(),
            input.natural_final,
        )?;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0 + std::mem::take(&mut setup_ms);
        execution::checkpoint()?;
        let output = Output {
            sequence: input.sequence,
            pcm,
            natural_final: input.natural_final,
            elapsed_ms,
        };
        if outputs.send(Ok(output)).is_err() || input.natural_final {
            return Ok(());
        }
    }
}
