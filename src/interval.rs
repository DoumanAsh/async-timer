//!Interval module

use core::future::Future;
use core::{task, time, mem};
use core::pin::Pin;

use crate::oneshot::Oneshot;
use crate::oneshot::Timer as PlatformTimer;

///Periodic Timer
///
///On each completition user receives new instance that can be polled once again
///Note that returned Interval is already armed, so it don't need initial poll to start over.
///
///## Usage
///
///```rust, no_run
///#![feature(async_await)]
///
///async fn job() {
///}
///
///async fn do_a_while() {
///    let mut times: u8 = 0;
///    let mut interval = async_timer::Interval::platform_new(core::time::Duration::from_secs(1));
///
///    while times < 5 {
///        job().await;
///        interval = interval.await;
///        times += 1;
///    }
///}
///```
#[must_use = "Interval does nothing unless polled"]
pub enum Interval<T=PlatformTimer> {
    #[doc(hidden)]
    Ongoing(T, time::Duration),
    #[doc(hidden)]
    Stopped,
}

impl Interval {
    #[inline(always)]
    ///Creates new instance using platform timer
    pub fn platform_new(interval: time::Duration) -> Self {
        Interval::<PlatformTimer>::new(interval)
    }
}

impl<T: Oneshot> Interval<T> {
    ///Creates new instance with specified timer type.
    pub fn new(interval: time::Duration) -> Self {
        Interval::Ongoing(T::new(interval), interval)
    }
}

impl<T: Oneshot> Future for Interval<T> {
    type Output = Self;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let mut state = Interval::Stopped;
        mem::swap(self.as_mut().get_mut(), &mut state);
        match state {
            Interval::Ongoing(mut timer, interval) => match Future::poll(Pin::new(&mut timer), ctx) {
                task::Poll::Ready(()) => {
                    timer.restart(&interval, ctx.waker());
                    task::Poll::Ready(Interval::Ongoing(timer, interval))
                },
                task::Poll::Pending => {
                    *self = Interval::Ongoing(timer, interval);
                    task::Poll::Pending
                },
            },
            Interval::Stopped => task::Poll::Pending
        }
    }
}
