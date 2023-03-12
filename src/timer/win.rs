//! Windows API based timer

use core::{task, time, ptr, mem};
use core::pin::Pin;
use core::future::Future;

use crate::state::TimerState;
use crate::alloc::boxed::Box;

#[allow(non_snake_case, non_camel_case_types)]
mod ffi {
    pub use core::ffi::c_void;
    use libc::{c_ulong, c_int};

    #[repr(C)]
    pub struct FILETIME {
        pub dwLowDateTime: c_ulong,
        pub dwHighDateTime: c_ulong,
    }
    pub type PTP_TIMER = *mut c_void;

    type PTP_TIMER_CALLBACK = Option<unsafe extern "system" fn(Instance: *mut c_void, Context: *mut c_void, Timer: *mut c_void)>;

    extern "system" {
        pub fn CloseThreadpoolTimer(pti: *mut c_void);
        pub fn CreateThreadpoolTimer(pfnti: PTP_TIMER_CALLBACK, pv: *mut c_void, pcbe: *mut c_void) -> *mut c_void;
        pub fn SetThreadpoolTimerEx(pti: *mut c_void, pftDueTime: *mut FILETIME, msPeriod: c_ulong, msWindowLength: c_ulong) -> c_int;
        pub fn WaitForThreadpoolTimerCallbacks(pti: PTP_TIMER, fCancelPendingCallbacks: c_int);
    }
}

unsafe extern "system" fn timer_callback(_: *mut ffi::c_void, data: *mut ffi::c_void, _: *mut ffi::c_void) {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_ptr_alignment))]
    let state = data as *mut TimerState;

    (*state).wake();
}

fn time_create(state: *mut TimerState) -> ffi::PTP_TIMER {
    let timer = unsafe {
        ffi::CreateThreadpoolTimer(Some(timer_callback), state as *mut ffi::c_void, ptr::null_mut())
    };
    os_assert!(!timer.is_null());

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

unsafe impl Send for State {}
unsafe impl Sync for State {}

///Windows Native timer
pub struct WinTimer {
    state: State,
}

impl WinTimer {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl super::Timer for WinTimer {
    #[inline(always)]
    fn new(timeout: time::Duration) -> Self {
        assert_time!(timeout);
        debug_assert!(timeout.as_millis() <= u32::max_value().into());
        Self::new(timeout)
    }

    #[inline]
    fn is_ticking(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => !state.is_done(),
        }
    }

    #[inline]
    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => state.is_done(),
        }
    }

    fn restart(&mut self, new_value: time::Duration) {
        assert_time!(new_value);
        debug_assert!(new_value.as_millis() <= u32::max_value().into());

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(ref fd, ref state) => {
                state.reset();
                set_timer_value(*fd, new_value);
            }
        }
    }

    fn restart_ctx(&mut self, new_value: time::Duration, waker: &task::Waker) {
        assert_time!(new_value);
        debug_assert!(new_value.as_millis() <= u32::max_value().into());

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(ref fd, ref state) => {
                state.register(waker);
                state.reset();
                set_timer_value(*fd, new_value);
            }
        }
    }

    fn cancel(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(fd, ref state) => unsafe {
                state.cancel();
                ffi::SetThreadpoolTimerEx(fd, ptr::null_mut(), 0, 0);
                ffi::WaitForThreadpoolTimerCallbacks(fd, 1);
            }
        }
    }
}

impl super::SyncTimer for WinTimer {
    fn init<R, F: Fn(&TimerState) -> R>(&mut self, init: F) -> R {
        if let State::Init(timeout) = self.state {
            let state = Box::into_raw(Box::new(TimerState::new()));
            let fd = time_create(state);

            let state = unsafe { Box::from_raw(state) };

            init(&state);

            set_timer_value(fd, timeout);

            self.state = State::Running(fd, state)
        }

        match &self.state {
            State::Running(_, ref state) => init(&state),
            State::Init(_) => unreach!(),
        }
    }
}

impl Future for WinTimer {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        crate::timer::poll_sync(self.get_mut(), ctx)
    }
}

impl Drop for WinTimer {
    fn drop(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(fd, ref state) => unsafe {
                state.cancel();
                ffi::SetThreadpoolTimerEx(fd, ptr::null_mut(), 0, 0);
                ffi::WaitForThreadpoolTimerCallbacks(fd, 1);
                ffi::CloseThreadpoolTimer(fd);
            }
        }
    }
}
