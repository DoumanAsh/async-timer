//! Delay

use core::pin::Pin;
use core::future::Future;
use core::{task, time};

use crate::{PlatformTimer, Timer};
use crate::state::TimerState;

use crate::alloc::boxed::Box;

enum State<T> {
    Init,
    Active(Box<TimerState>, T)
}

#[must_use = "Delay does nothing unless polled"]
///Future that resolves sometime in future
///
///## Implementation notes
///
///When `Future` is created, timer is not yet started.
///The actual timer will start only after future will be initally polled.
///
///After timer expires, executor will be notified and next call to `Future::poll` shall
///return `Ready`, after which all further calls to `Future::poll` result in `Ready`
///without restarting timer.
///
///## Usage
///
///```rust,no_run
/// use async_timer::Delay;
/// use core::time;
///
/// Delay::platform_new(time::Duration::from_secs(2));
///```
pub struct Delay<T=PlatformTimer> {
    pub(crate) timeout: time::Duration,
    state: State<T>
}

impl Delay {
    ///Creates new instance using [PlatformTimer](../type.PlatformTimer.html)
    pub fn platform_new(timeout: time::Duration) -> Self {
        Self {
            timeout,
            state: State::Init,
        }
    }
}

impl<T: Timer> Delay<T> {
    ///Creates new instance with specified timeout
    ///
    ///Requires to specify `Timer` type (e.g. `Delay::<PlatformTimer>::new()`)
    pub fn new(timeout: time::Duration) -> Self {
        Self {
            timeout,
            state: State::Init,
        }
    }

    ///Restarts underlying `Timer`.
    ///
    ///Comparing to creating `Delay`
    ///the `Timer` will start counting immediately.
    pub fn restart(&mut self, ctx: &task::Context) {
        match self.state {
            State::Init => (),
            State::Active(ref mut _state, ref mut timer) => {
                match timer.state().is_done() {
                    true => (),
                    false => timer.reset(),
                }

                timer.register_waker(ctx.waker());
                timer.start_delay(self.timeout)
            }
        }
    }
}

impl<T: Timer> Future for Delay<T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        self.state = match &self.state {
            State::Init => {
                let state = Box::into_raw(Box::new(TimerState::new()));
                let mut timer = T::new(state);
                let state = unsafe { Box::from_raw(state) };
                assert!(state.is_done());

                timer.register_waker(ctx.waker());
                timer.start_delay(self.timeout);

                State::Active(state, timer)
            },
            State::Active(ref _state, ref timer) => match timer.state().is_done() {
                true => return task::Poll::Ready(()),
                false => return task::Poll::Pending,
            }
        };

        task::Poll::Pending
    }
}
