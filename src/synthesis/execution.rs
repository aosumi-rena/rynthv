use super::voice::{Result, VoiceError};
use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct Cancellation {
    flag: Arc<AtomicBool>,
    parent: Option<Arc<Cancellation>>,
}

impl Cancellation {
    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self { flag, parent: None }
    }

    pub(crate) fn child(parent: Option<Self>) -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            parent: parent.map(Arc::new),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.is_cancelled())
    }

    pub(crate) fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Cancellation>> = const { RefCell::new(None) };
    static ZERO_GRU_WEIGHTS: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn zero_gru_weights() -> bool {
    ZERO_GRU_WEIGHTS.get()
}

pub fn scope_gru_weights<T>(zero: bool, run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ZERO_GRU_WEIGHTS.set(self.0);
        }
    }
    let _restore = Restore(ZERO_GRU_WEIGHTS.replace(zero));
    run()
}

pub(crate) fn render_scope<T>(
    cancellation: Cancellation,
    zero_gru_weights: bool,
    run: impl FnOnce() -> T,
) -> T {
    scope_gru_weights(zero_gru_weights, || scope(cancellation, run))
}

pub fn current() -> Option<Cancellation> {
    CURRENT.with(|current| current.borrow().clone())
}

pub fn checkpoint() -> Result<()> {
    let cancelled = CURRENT.with(|current| {
        current
            .borrow()
            .as_ref()
            .is_some_and(Cancellation::is_cancelled)
    });
    if cancelled {
        Err(VoiceError::new("合成任务已由更新的编辑替代。"))
    } else {
        Ok(())
    }
}

pub fn scope<T>(cancellation: Cancellation, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<Cancellation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|current| *current.borrow_mut() = self.0.take());
        }
    }
    let previous = CURRENT.with(|current| current.replace(Some(cancellation)));
    let _restore = Restore(previous);
    run()
}
