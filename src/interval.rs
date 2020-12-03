//!Interval module

use core::future::Future;
use core::{task, time};
use core::pin::Pin;

use crate::timer::Timer;
use crate::timer::Platform as PlatformTimer;

///Periodic Timer
///
///On each completion, underlying timer is restarted and therefore `Future` can be polled once
///more.
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
///        interval.wait().await;
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

impl<T: Timer> Interval<T> {
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
    pub fn restart(&mut self) {
        let interval = self.interval;
        self.timer.restart(interval);
    }

    #[inline(always)]
    ///Returns future for next expiration.
    pub fn wait<'a>(&'a mut self) -> impl Future<Output=()> + 'a {
        self
    }
}

impl<T: Timer> Future for &'_ mut Interval<T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        match Future::poll(Pin::new(&mut self.timer), ctx) {
            task::Poll::Ready(()) => {
                self.restart();
                task::Poll::Ready(())
            },
            task::Poll::Pending => task::Poll::Pending,
        }
    }
}

#[cfg(feature = "stream")]
impl<T: Timer> futures_core::stream::Stream for Interval<T> {
    type Item = ();

    #[inline]
    fn poll_next(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Option<Self::Item>> {
        let mut this = self.get_mut();
        Future::poll(Pin::new(&mut this), ctx).map(|res| Some(res))
    }
}
