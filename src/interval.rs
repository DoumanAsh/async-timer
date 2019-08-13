//!Interval module

use core::future::Future;
use core::{task, time};
use core::pin::Pin;

use crate::oneshot::Oneshot;
use crate::oneshot::Timer as PlatformTimer;

///Periodic Timer
#[must_use = "Interval does nothing unless polled"]
pub struct Interval<T=PlatformTimer> {
    inner: Option<(T, time::Duration)>,
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
            inner: Some((T::new(interval), interval)),
        }
    }

    fn inner(&mut self) -> &mut (T, time::Duration) {
        match self.inner {
            Some(ref mut inner) => inner,
            None => unreach!()
        }
    }
}

impl<T: Oneshot> Future for Interval<T> {
    type Output = Self;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = self.get_mut();

        match Future::poll(Pin::new(&mut this.inner().0), ctx) {
            task::Poll::Ready(()) => {
                let (mut timer, interval) = match this.inner.take() {
                    Some(res) => res,
                    None => unreach!(),
                };
                timer.restart(&interval, ctx.waker());
                task::Poll::Ready(Self {
                    inner: Some((timer, interval)),
                })
            },
            task::Poll::Pending => task::Poll::Pending,
        }
    }
}
