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

    fn is_expired(&self) -> bool {
        false
    }

    fn cancel(&mut self) {
        unimplemented!();
    }

    fn restart(&mut self, new_value: &time::Duration, waker: &task::Waker) {
        unimplemented!();
    }
}

impl Future for DummyTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        unimplemented!();
    }
}

