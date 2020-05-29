use core::{task, time};
use core::future::Future;
use core::pin::Pin;

///Timer that never expires.
pub struct NeverTimer;

impl super::Oneshot for NeverTimer {
    fn new(_: time::Duration) -> Self {
        Self
    }

    fn is_ticking(&self) -> bool {
        true
    }

    fn is_expired(&self) -> bool {
        false
    }

    fn cancel(&mut self) {
    }

    fn restart(&mut self, _: time::Duration) {
    }

    fn restart_waker(&mut self, _: time::Duration, _: &task::Waker) {
    }
}

impl Future for NeverTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut task::Context) -> task::Poll<Self::Output> {
        task::Poll::Pending
    }
}
