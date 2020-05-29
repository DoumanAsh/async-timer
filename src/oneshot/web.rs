//! Web based timer

use core::{task, time};
use core::pin::Pin;
use core::future::Future;

use crate::state::TimerState;
use crate::alloc::boxed::Box;

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn setTimeout(closure: &wasm_bindgen::closure::Closure<dyn FnMut()>, time: u32) -> i32;
    fn clearTimeout(id: i32);
}

struct TimerHandle(i32);

impl TimerHandle {
    #[inline]
    fn clear(&mut self) {
        clearTimeout(self.0)
    }
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        self.clear();
    }
}

fn time_create(timeout: time::Duration, state: *const TimerState) -> TimerHandle {
    let timeout = timeout.as_millis() as u32;

    let cb = wasm_bindgen::closure::Closure::once(move || unsafe {
        (*state).wake();
    });
    let handle = setTimeout(&cb, timeout);

    TimerHandle(handle)
}

enum State {
    Init(time::Duration),
    Running(TimerHandle, *const TimerState),
}

///Web timer wrapper
pub struct WebTimer {
    state: State,
}

impl super::Oneshot for WebTimer {
    fn new(timeout: time::Duration) -> Self {
        debug_assert!(!(timeout.as_secs() == 0 && timeout.subsec_nanos() == 0), "Zero timeout makes no sense");

        Self {
            state: State::Init(timeout),
        }
    }

    fn is_ticking(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => unsafe { !(**state).is_done() },
        }
    }

    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => unsafe { (**state).is_done() },
        }
    }

    fn cancel(&mut self) {
        match &mut self.state {
            State::Init(_) => (),
            State::Running(ref mut fd, _) => {
                fd.clear();
            }
        }
    }

    fn restart(&mut self, new_value: time::Duration) {
        debug_assert!(!(new_value.as_secs() == 0 && new_value.subsec_nanos() == 0), "Zero timeout makes no sense");

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value
            },
            State::Running(ref mut fd, ref state) => {
                unsafe { (**state).reset() };
                *fd = time_create(new_value, *state);
            },
        }
    }

    fn restart_waker(&mut self, new_value: time::Duration, waker: &task::Waker) {
        debug_assert!(!(new_value.as_secs() == 0 && new_value.subsec_nanos() == 0), "Zero timeout makes no sense");

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value
            },
            State::Running(ref mut fd, ref state) => {
                unsafe { (**state).register(waker) };
                *fd = time_create(new_value, *state);
            },
        }
    }
}

impl Future for WebTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        self.state = match &self.state {
            State::Init(ref timeout) => {
                let state = TimerState::new();
                state.register(ctx.waker());

                let state = Box::into_raw(Box::new(state));
                let fd = time_create(*timeout, state);

                State::Running(fd, state)
            },
            State::Running(_, ref state) => match unsafe { (**state).is_done() } {
                false => return task::Poll::Pending,
                true => return task::Poll::Ready(()),
            }
        };

        task::Poll::Pending
    }
}

impl Drop for WebTimer {
    fn drop(&mut self) {
        match self.state {
            State::Running(ref mut fd, state) => {
                fd.clear();
                unsafe { Box::from_raw(state as *mut TimerState) };
            },
            _ => (),
        }
    }
}

unsafe impl Send for WebTimer {}
unsafe impl Sync for WebTimer {}
