//! Dummy Timer

use core::{task, time};
use core::future::Future;
use core::pin::Pin;

///Dummy Timer with implementation that panics
pub struct DummyTimer;

impl DummyTimer {
    ///Creates new instance
    pub const fn new(_: time::Duration) -> Self {
        Self
    }
}

impl super::Timer for DummyTimer {
    fn new(_: time::Duration) -> Self {
        unimplemented!();
    }

    fn is_ticking(&self) -> bool {
        false
    }

    fn is_expired(&self) -> bool {
        false
    }

    fn cancel(&mut self) {
        unimplemented!();
    }

    fn restart(&mut self, _: time::Duration) {
        unimplemented!();
    }

    fn restart_ctx(&mut self, _: time::Duration, _: &task::Waker) {
        unimplemented!();
    }
}

impl super::SyncTimer for DummyTimer {
    fn init<R, F: Fn(&crate::state::TimerState) -> R>(&mut self, _: F) -> R {
        unimplemented!();
    }
}

impl Future for DummyTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut task::Context) -> task::Poll<Self::Output> {
        unimplemented!();
    }
}
