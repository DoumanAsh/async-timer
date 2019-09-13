//!Interval module

use core::future::Future;
use core::{task, time};
use core::pin::Pin;

use crate::oneshot::Oneshot;
use crate::oneshot::Timer as PlatformTimer;

///Periodic Timer
///
///On each completition user receives new instance that can be polled once again
///
///## Note
///
///Returned Interval is already armed, so it don't need initial poll to start over.
///
///## Usage
///
///```rust, no_run
///async fn job() {
///}
///
///async fn do_a_while() {
///    let mut times: u8 = 0;
///    let mut interval = async_timer::Interval::platform_new(core::time::Duration::from_secs(1));
///
///    while times < 5 {
///        job().await;
///        interval.as_mut().await;
///        times += 1;
///    }
///}
///```
#[must_use = "Interval does nothing unless polled"]
pub struct Interval<T=PlatformTimer> {
    timer: T,
    ///Timer interval, change to this value will be reflected on next restart of timer.
    pub interval: time::Duration,
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
        Self {
            timer: T::new(interval),
            interval,
        }
    }

    #[inline(always)]
    ///Stops interval
    pub fn cancel(&mut self) {
        self.timer.cancel()
    }

    ///Restarts interval
    pub fn restart(&mut self, ctx: &task::Context) {
        let interval = self.interval;
        self.timer.restart(interval, ctx.waker());
    }


    #[inline(always)]
    ///Gets mutable reference
    pub fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: Oneshot> Future for &'_ mut Interval<T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        match Future::poll(Pin::new(&mut self.timer), ctx) {
            task::Poll::Ready(()) => {
                self.restart(ctx);
                task::Poll::Ready(())
            },
            task::Poll::Pending => task::Poll::Pending,
        }
    }
}
