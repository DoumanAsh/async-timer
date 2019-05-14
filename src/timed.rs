//! Timed future

use core::future::Future;
use core::{fmt, task, time};
use core::pin::Pin;

use crate::oneshot::Oneshot;
use crate::oneshot::Timer as PlatformTimer;

#[must_use = "Timed does nothing unless polled"]
///Limiter on time to wait for underlying `Future`
pub struct Timed<F, T=PlatformTimer> {
    timeout: time::Duration,
    state: Option<(F, T)>,
}

impl<F: Unpin> Timed<F> {
    #[inline]
    ///Creates new instance using [Timer](../oneshot//type.Timer.html) alias.
    pub fn platform_new(inner: F, timeout: time::Duration) -> Self {
        Timed::<F, PlatformTimer>::new(inner, timeout)
    }
}

impl<F> Timed<F> {
    #[inline]
    ///Creates new instance using [Timer](../oneshot//type.Timer.html) alias.
    ///
    ///Unsafe version of `platform_new` that doesn't require `Unpin`.
    pub unsafe fn platform_new_unchecked(inner: F, timeout: time::Duration) -> Self {
        Timed::<F, PlatformTimer>::new_unchecked(inner, timeout)
    }
}

impl<F: Unpin, T: Oneshot> Timed<F, T> {
    ///Creates new instance with specified timeout
    ///
    ///Requires to specify `Oneshot` type (e.g. `Timed::<oneshoot::Timer>::new()`)
    pub fn new(inner: F, timeout: time::Duration) -> Self {
        Self {
            timeout: timeout.clone(),
            state: Some((inner, T::new(timeout))),
        }
    }
}

impl<F, T: Oneshot> Timed<F, T> {
    ///Creates new instance with specified timeout
    ///
    ///Unsafe version of `new` that doesn't require `Unpin`.
    ///
    ///Requires to specify `Oneshot` type (e.g. `Timed::<oneshoot::Timer>::new()`)
    pub unsafe fn new_unchecked(inner: F, timeout: time::Duration) -> Self {
        Self {
            timeout: timeout.clone(),
            state: Some((inner, T::new(timeout))),
        }
    }

    fn state(&mut self) -> &mut (F, T) {
        match self.state {
            Some(ref mut state) => state,
            None => unreach!()
        }
    }
}

impl<F: Future, T: Oneshot> Future for Timed<F, T> {
    type Output = Result<F::Output, Expired<F, T>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        match Future::poll(unsafe { Pin::new_unchecked(&mut this.state().0) }, ctx) {
            task::Poll::Pending => (),
            task::Poll::Ready(result) => return task::Poll::Ready(Ok(result)),
        }

        match Future::poll(Pin::new(&mut this.state().1), ctx) {
            task::Poll::Pending => task::Poll::Pending,
            task::Poll::Ready(_) => return task::Poll::Ready(Err(Expired {
                timeout: this.timeout.clone(),
                state: this.state.take()
            }))
        }
    }
}

impl<F: Future + Unpin, T: Oneshot> Unpin for Timed<F, T> {}

///Error when [Timed](struct.Timed.html) expires
///
///Implements `Future` that can be used to restart `Timed`
///Note, that `Oneshot` starts execution immediately after resolving this Future
pub struct Expired<F, T> {
    timeout: time::Duration,
    state: Option<(F, T)>,
}

impl<F, T> Expired<F, T> {
    ///Returns underlying `Future`
    pub fn into_inner(self) -> F {
        match self.state {
            Some((inner, _)) => inner,
            None => unreach!()
        }
    }
}

impl<F: Future, T: Oneshot> Future for Expired<F, T> {
    type Output = Timed<F, T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        match this.state.take() {
            Some((inner, mut delay)) => {
                delay.restart(&this.timeout, ctx.waker());
                task::Poll::Ready(Timed::<F, T> {
                    timeout: this.timeout.clone(),
                    state: Some((inner, delay)),
                })
            },
            None => unreach!(),
        }
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
        match self.timeout.as_secs() {
            0 => write!(f, "Future expired in {} ms", self.timeout.as_millis()),
            secs => write!(f, "Future expired in {} seconds and {} ms", secs, self.timeout.subsec_millis()),
        }
    }
}
