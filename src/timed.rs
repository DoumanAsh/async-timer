//! Timed future

use core::future::Future;
use core::{fmt, task, time, mem};
use core::pin::Pin;

use crate::oneshot::Oneshot;
use crate::oneshot::Timer as PlatformTimer;

#[must_use = "Timed does nothing unless polled"]
///Limiter on time to wait for underlying `Future`
///
///# Usage
///
///```rust, no_run
///async fn job() {
///}
///
///async fn do_job() {
///    let work = unsafe {
///        async_timer::Timed::platform_new_unchecked(job(), core::time::Duration::from_secs(1))
///    };
///
///    match work.await {
///        Ok(_) => println!("I'm done!"),
///        //You can retry by polling `expired`
///        Err(expired) => println!("Job expired: {}", expired),
///    }
///}
///```
pub enum Timed<F, T=PlatformTimer> {
    #[doc(hidden)]
    Ongoing(T, F, time::Duration),
    #[doc(hidden)]
    Stopped,
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
        Timed::Ongoing(T::new(timeout), inner, timeout)
    }
}

impl<F: Future, T: Oneshot> Timed<F, T> {
    ///Creates new instance with specified timeout
    ///
    ///Unsafe version of `new` that doesn't require `Unpin`.
    ///
    ///Requires to specify `Oneshot` type (e.g. `Timed::<oneshoot::Timer>::new()`)
    pub unsafe fn new_unchecked(inner: F, timeout: time::Duration) -> Self {
        Timed::Ongoing(T::new(timeout), inner, timeout)
    }
}

impl<F: Future, T: Oneshot> Future for Timed<F, T> {
    type Output = Result<F::Output, Expired<F, T>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let mut state = Timed::Stopped;
        let mut this = unsafe { self.get_unchecked_mut() };
        mem::swap(&mut state, &mut this);

        match state {
            Timed::Ongoing(mut timer, mut future, timeout) => {
                match Future::poll(unsafe { Pin::new_unchecked(&mut future) }, ctx) {
                    task::Poll::Pending => (),
                    task::Poll::Ready(result) => return task::Poll::Ready(Ok(result)),
                }

                match Future::poll(Pin::new(&mut timer), ctx) {
                    task::Poll::Pending => (),
                    task::Poll::Ready(_) => return task::Poll::Ready(Err(Expired {
                        inner: Timed::Ongoing(timer, future, timeout),
                    })),
                }

                *this = Timed::Ongoing(timer, future, timeout);
                task::Poll::Pending
            },
            Timed::Stopped => task::Poll::Pending,
        }
    }
}

impl<F: Future + Unpin, T: Oneshot> Unpin for Timed<F, T> {}

///Error when [Timed](struct.Timed.html) expires
///
///Implements `Future` that can be used to restart `Timed`
///Note, that `Oneshot` starts execution immediately after resolving this Future
pub struct Expired<F, T> {
    inner: Timed<F, T>,
}

impl<F: Future, T: Oneshot> Expired<F, T> {
    ///Returns underlying `Future`
    pub fn into_inner(self) -> F {
        match self.inner {
            Timed::Ongoing(_, fut, _) => fut,
            _ => unreach!(),
        }
    }
}

impl<F: Future, T: Oneshot> Future for Expired<F, T> {
    type Output = Timed<F, T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let mut state = Timed::Stopped;
        let this = unsafe { self.get_unchecked_mut() };
        mem::swap(&mut this.inner, &mut state);

        match state {
            Timed::Ongoing(mut timer, future, timeout) => {
                timer.restart(timeout, ctx.waker());

                task::Poll::Ready(Timed::Ongoing(timer, future, timeout))
            },
            _ => task::Poll::Pending,
        }
    }
}

impl<F: Future + Unpin, T: Oneshot> Unpin for Expired<F, T> {}

#[cfg(not(feature = "no_std"))]
impl<F, T: Oneshot> crate::std::error::Error for Expired<F, T> {}
impl<F, T: Oneshot> fmt::Debug for Expired<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl<F, T: Oneshot> fmt::Display for Expired<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            Timed::Stopped => write!(f, "Future is being re-tried."),
            Timed::Ongoing(_, _, timeout) => match timeout.as_secs() {
                0 => write!(f, "Future expired in {} ms", timeout.as_millis()),
                secs => write!(f, "Future expired in {} seconds and {} ms", secs, timeout.subsec_millis()),
            },
        }
    }
}
