//! Dummy Timer

use core::{task, time};
use core::future::Future;
use core::pin::Pin;

///Dummy Timer
pub struct DummyTimer;

impl super::Oneshot for DummyTimer {
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

    fn restart_waker(&mut self, _: time::Duration, _: &task::Waker) {
        unimplemented!();
    }
}

impl Future for DummyTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut task::Context) -> task::Poll<Self::Output> {
        unimplemented!();
    }
}

