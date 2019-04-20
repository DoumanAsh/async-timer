//! Delay

use core::pin::Pin;
use core::future::Future;
use core::{task, time};

use crate::Timer;
use crate::state::TimerState;

enum State<T> {
    Init,
    Active(Box<TimerState>, T)
}

#[must_use = "futures do nothing unless polled"]
///Future that resolves sometime in future
pub struct Delay<T=crate::PlatformTimer> {
    timeout: time::Duration,
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
    pub fn new(timeout: time::Duration) -> Self {
        Self {
            timeout,
            state: State::Init,
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
