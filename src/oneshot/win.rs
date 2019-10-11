//! Windows API based timer

use super::state::TimerState;
use crate::alloc::boxed::Box;

use core::{mem, task, time, ptr};
use core::pin::Pin;
use core::future::Future;

mod ffi {
    pub use winapi::shared::minwindef::{FILETIME};
    pub use winapi::um::threadpoolapiset::{
        CloseThreadpoolTimer,
        CreateThreadpoolTimer,
        SetThreadpoolTimerEx,
        WaitForThreadpoolTimerCallbacks,
    };

    pub use winapi::ctypes::{c_ulong, c_void};
    pub use winapi::um::winnt::{PTP_TIMER_CALLBACK, PTP_CALLBACK_INSTANCE, PTP_TIMER};
}

unsafe extern "system" fn timer_callback(_: ffi::PTP_CALLBACK_INSTANCE, data: *mut ffi::c_void, _: ffi::PTP_TIMER) {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_ptr_alignment))]
    let state = data as *mut TimerState;

    (*state).wake();
}

fn time_create(state: *mut TimerState) -> ffi::PTP_TIMER {
    let timer = unsafe {
        ffi::CreateThreadpoolTimer(Some(timer_callback), state as *mut ffi::c_void, ptr::null_mut())
    };
    debug_assert!(!timer.is_null());

    timer
}

fn set_timer_value(fd: ffi::PTP_TIMER, timeout: time::Duration) {
    let mut ticks = i64::from(timeout.subsec_nanos() / 100);
    ticks += (timeout.as_secs() * 10_000_000) as i64;
    let ticks = -ticks;

    unsafe {
        let mut time: ffi::FILETIME = mem::transmute(ticks);
        ffi::SetThreadpoolTimerEx(fd, &mut time, 0, 0);
    }
}

enum State {
    Init(time::Duration),
    Running(ffi::PTP_TIMER, Box<TimerState>),
}

///Windows Native timer
pub struct WinTimer {
    state: State,
}

impl super::Oneshot for WinTimer {
    fn new(timeout: time::Duration) -> Self {
        debug_assert!(!(timeout.as_secs() == 0 && timeout.subsec_nanos() == 0), "Zero timeout makes no sense");

        Self {
            state: State::Init(timeout),
        }
    }

    fn is_ticking(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => !state.is_done(),
        }
    }

    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => state.is_done(),
        }
    }

    fn cancel(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(fd, _) => unsafe {
                ffi::SetThreadpoolTimerEx(fd, ptr::null_mut(), 0, 0);
                ffi::WaitForThreadpoolTimerCallbacks(fd, 1);
            }
        }
    }

    fn restart(&mut self, new_value: time::Duration, waker: &task::Waker) {
        debug_assert!(!(new_value.as_secs() == 0 && new_value.subsec_nanos() == 0), "Zero timeout makes no sense");

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(fd, ref mut state) => {
                state.register(waker);
                set_timer_value(*fd, new_value);
            }
        }
    }
}

impl Future for WinTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        self.state = match &self.state {
            State::Init(ref timeout) => {
                let state = Box::into_raw(Box::new(TimerState::new()));
                let fd = time_create(state);

                let state = unsafe { Box::from_raw(state) };
                state.register(ctx.waker());

                set_timer_value(fd, *timeout);

                State::Running(fd, state)
            },
            State::Running(_, ref state) => match state.is_done() {
                false => return task::Poll::Pending,
                true => return task::Poll::Ready(()),
            }
        };

        task::Poll::Pending
    }
}

impl Drop for WinTimer {
    fn drop(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(fd, _) => unsafe {
                ffi::SetThreadpoolTimerEx(fd, ptr::null_mut(), 0, 0);
                ffi::WaitForThreadpoolTimerCallbacks(fd, 1);
                ffi::CloseThreadpoolTimer(fd);
            }
        }
    }
}

unsafe impl Send for WinTimer {}
unsafe impl Sync for WinTimer {}
