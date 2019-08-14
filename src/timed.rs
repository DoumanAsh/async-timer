//! Timed future

use core::future::Future;
use core::{fmt, task, time};
use core::pin::Pin;

use crate::oneshot::Oneshot;
use crate::oneshot::Timer as PlatformTimer;

#[must_use = "Timed does nothing unless polled"]
///Limiter on time to wait for underlying `Future`
///
///# Usage
///
///```rust, no_run
///#![feature(async_await)]
///
///async fn job() {
///}
///
///async fn do_job() {
///    let work = unsafe {
///        async_timer::Timed::platform_new_unchecked(job(), core::time::Duration::from_secs(1))
///    };
///
///    match work.wait().await {
///        Ok(_) => println!("I'm done!"),
///        //You can `Expired::retry` to resume it
///        Err(expired) => println!("Job expired: {}", expired),
///    }
///}
///```
pub struct Timed<F, T=PlatformTimer> {
    timeout: time::Duration,
    inner: F,
    timer: T,
}

impl<F: Future + Unpin> Timed<F> {
    #[inline]
    ///Creates new instance using [Timer](../oneshot/type.Timer.html) alias.
    pub fn platform_new(inner: F, timeout: time::Duration) -> Self {
        Timed::<F, PlatformTimer>::new(inner, timeout)
    }
}

impl<F: Future> Timed<F> {
    #[inline]
    ///Creates new instance using [Timer](../oneshot/type.Timer.html) alias.
    ///
    ///Unsafe version of `platform_new` that doesn't require `Unpin`.
    pub unsafe fn platform_new_unchecked(inner: F, timeout: time::Duration) -> Self {
        Timed::<F, PlatformTimer>::new_unchecked(inner, timeout)
    }
}

impl<F: Future + Unpin, T: Oneshot> Timed<F, T> {
    ///Creates new instance with specified timeout
    ///
    ///Requires to specify `Oneshot` type (e.g. `Timed::<oneshoot::Timer>::new()`)
    pub fn new(inner: F, timeout: time::Duration) -> Self {
        Self {
            timeout,
            inner,
            timer: T::new(timeout),
        }
    }
}

impl<F: Future, T: Oneshot> Timed<F, T> {
    ///Creates new instance with specified timeout
    ///
    ///Unsafe version of `new` that doesn't require `Unpin`.
    ///
    ///Requires to specify `Oneshot` type (e.g. `Timed::<oneshoot::Timer>::new()`)
    pub unsafe fn new_unchecked(inner: F, timeout: time::Duration) -> Self {
        Self {
            timeout,
            inner,
            timer: T::new(timeout),
        }
    }

    ///Waits for future to finish executing within specified timeout.
    pub async fn wait(mut self) -> Result<F::Output, Expired<F, T>> {
        match unsafe { Pin::new_unchecked(&mut self) }.await {
            TimedStatus::Finished(result) => Ok(result),
            TimedStatus::Expired => Err(Expired {
                timed: self,
            }),
        }
    }
}

///Plain result of Timed Future execution.
pub enum TimedStatus<T> {
    ///Future is successfully finished in time.
    Finished(T),
    ///Timed expired before future finished execution.
    Expired,
}

impl<F: Future, T: Oneshot> Future for Timed<F, T> {
    type Output = TimedStatus<F::Output>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        match Future::poll(unsafe { Pin::new_unchecked(&mut this.inner) }, ctx) {
            task::Poll::Pending => (),
            task::Poll::Ready(result) => return task::Poll::Ready(TimedStatus::Finished(result)),
        }

        match Future::poll(Pin::new(&mut this.timer), ctx) {
            task::Poll::Pending => task::Poll::Pending,
            task::Poll::Ready(_) => return task::Poll::Ready(TimedStatus::Expired),
        }
    }
}

impl<F: Future + Unpin, T: Oneshot> Unpin for Timed<F, T> {}

///Error when [Timed](struct.Timed.html) expires
///
///Implements `Future` that can be used to restart `Timed`
///Note, that `Oneshot` starts execution immediately after resolving this Future
pub struct Expired<F, T> {
    timed: Timed<F, T>
}

impl<F: Future, T: Oneshot> Expired<F, T> {
    ///Returns underlying `Future`
    pub fn into_inner(self) -> F {
        self.timed.inner
    }

    ///Retries to execute future again with the same timeout
    pub async fn retry(mut self) -> Timed<F, T> {
        unsafe { Pin::new_unchecked(&mut self) }.await;

        self.timed
    }

    ///Retries to execute future again with the new timeout
    pub async fn retry_within(mut self, timeout: time::Duration) -> Timed<F, T> {
        self.timed.timeout = timeout;
        unsafe { Pin::new_unchecked(&mut self) }.await;

        self.timed
    }
}

impl<F: Future, T: Oneshot> Future for Expired<F, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let timeout = self.timed.timeout;
        let this = unsafe { self.get_unchecked_mut() };
        this.timed.timer.restart(&timeout, ctx.waker());

        task::Poll::Ready(())
    }
}

impl<F: Future + Unpin, T: Oneshot> Unpin for Expired<F, T> {}

#[cfg(not(feature = "no_std"))]
impl<F, T: Oneshot> std::error::Error for Expired<F, T> {}
impl<F, T: Oneshot> fmt::Debug for Expired<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl<F, T: Oneshot> fmt::Display for Expired<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.timed.timeout.as_secs() {
            0 => write!(f, "Future expired in {} ms", self.timed.timeout.as_millis()),
            secs => write!(f, "Future expired in {} seconds and {} ms", secs, self.timed.timeout.subsec_millis()),
        }
    }
}
