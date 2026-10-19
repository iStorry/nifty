//! Waking the main thread from others, through the main dispatch queue the run loop drains.

use std::fmt;
use std::sync::Arc;

use dispatch2::{DispatchQueue, MainThreadBound};
use objc2::MainThreadMarker;

use nifty_runtime::{WakeHandler, Waker};

pub struct MacWaker(Arc<MainThreadBound<WakeHandler>>);

impl MacWaker {
    pub(crate) fn new(mtm: MainThreadMarker, on_wake: WakeHandler) -> Self {
        Self(Arc::new(MainThreadBound::new(on_wake, mtm)))
    }
}

impl Waker for MacWaker {
    fn wake(&self) {
        let on_wake = Arc::clone(&self.0);
        DispatchQueue::main().exec_async(move || {
            let mtm = MainThreadMarker::new().expect("the main queue runs on the main thread");
            (on_wake.get(mtm))();
        });
    }
}

impl fmt::Debug for MacWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacWaker").finish_non_exhaustive()
    }
}
