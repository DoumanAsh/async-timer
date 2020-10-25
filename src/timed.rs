//! Timed future

use core::future::Future;
use core::{fmt, task, time};
use core::pin::Pin;

use crate::timer::Timer;
use crate::timer::Platform as PlatformTimer;

struct State<'a, F, T> {
    timer: T,
    timeout: time::Duration,
    fut: Pin<&'a mut F>
}

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
///    let mut job = job();
///    let job = unsafe {
///        core::pin::Pin::new_unchecked(&mut job)
///    };
///    let work = unsafe {
///        async_timer::Timed::platform_new(job, core::time::Duration::from_secs(1))
///    };
///
///    match work.await {
///        Ok(_) => println!("I'm done!"),
///        //You can retry by polling `expired`
///        Err(expired) => println!("Job expired: {}", expired),
///    }
///}
///```
pub struct Timed<'a, F, T=PlatformTimer> {
    state: Option<State<'a, F, T>>,
}

impl<'a, F: Future> Timed<'a, F> {
    #[inline]
    ///Creates new instance using [Timer](../oneshot/type.Timer.html) alias.
    pub fn platform_new(fut: Pin<&'a mut F>, timeout: time::Duration) -> Self {
        Self::new(fut, timeout)
    }
}

impl<'a, F: Future, T: Timer> Timed<'a, F, T> {
    ///Creates new instance with specified timeout
    ///
    ///Unsafe version of `new` that doesn't require `Unpin`.
    ///
    ///Requires to specify `Timer` type (e.g. `Timed::<timer::Platform>::new()`)
    pub fn new(fut: Pin<&'a mut F>, timeout: time::Duration) -> Self {
        Self {
            state: Some(State {
                timer: T::new(timeout),
                timeout,
                fut,
            })
        }
    }
}

impl<'a, F: Future, T: Timer> Future for Timed<'a, F, T> {
    type Output = Result<F::Output, Expired<'a, F, T>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = self.get_mut();

        if let Some(state) = this.state.as_mut() {
            match Future::poll(state.fut.as_mut(), ctx) {
                task::Poll::Pending => (),
                task::Poll::Ready(result) => return task::Poll::Ready(Ok(result)),
            }

            match Future::poll(Pin::new(&mut state.timer), ctx) {
                task::Poll::Pending => (),
                task::Poll::Ready(_) => return task::Poll::Ready(Err(Expired(this.state.take()))),
            }
        }

        task::Poll::Pending
    }
}

#[must_use = "Expire should be handled as error or to restart Timed"]
///Error when [Timed](struct.Timed.html) expires
///
///Implements `Future` that can be used to restart `Timed`
///Note, that `Timer` starts execution immediately after resolving this Future.
pub struct Expired<'a, F, T>(Option<State<'a, F, T>>);

impl<'a, F: Future, T: Timer> Future for Expired<'a, F, T> {
    type Output = Timed<'a, F, T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = self.get_mut();

        match this.0.take() {
            Some(mut state) => {
                state.timer.restart_ctx(state.timeout, ctx.waker());

                task::Poll::Ready(Timed {
                    state: Some(state)
                })
            },
            None => task::Poll::Pending,
        }
    }
}

#[cfg(feature = "std")]
impl<'a, F, T: Timer> crate::std::error::Error for Expired<'a, F, T> {}

impl<'a, F, T: Timer> fmt::Debug for Expired<'a, F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl<'a, F, T: Timer> fmt::Display for Expired<'a, F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_ref() {
            None => write!(f, "Future is being re-tried."),
            Some(state) => match state.timeout.as_secs() {
                0 => write!(f, "Future expired in {} ms", state.timeout.as_millis()),
                secs => write!(f, "Future expired in {} seconds and {} ms", secs, state.timeout.subsec_millis()),
            },
        }
    }
}
