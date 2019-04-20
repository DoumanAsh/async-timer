//! Windows Native Timer

use crate::{TimerState, Timer};

use core::{time, ptr};

use winapi::shared::minwindef::{FILETIME};
use winapi::um::threadpoolapiset::{
    CloseThreadpoolTimer,
    CreateThreadpoolTimer,
    SetThreadpoolTimerEx,
    WaitForThreadpoolTimerCallbacks,
};

use winapi::ctypes::{c_ulong, c_void};
use winapi::um::winnt::{PTP_CALLBACK_INSTANCE, PTP_TIMER};

unsafe extern "system" fn timer_callback(_: PTP_CALLBACK_INSTANCE, data: *mut c_void, _: PTP_TIMER) {
    let state = data as *mut TimerState;

    (*state).wake();
}

///Windows Native timer
pub struct WinTimer {
    timer: PTP_TIMER,
    state: *const TimerState,
}

unsafe impl Send for WinTimer {}
unsafe impl Sync for WinTimer {}

impl WinTimer {
    fn init_self(&mut self) {
        if self.timer.is_null() {
            self.timer = unsafe {
                CreateThreadpoolTimer(Some(timer_callback), self.state as *mut c_void, ptr::null_mut())
            };
            debug_assert!(!self.timer.is_null());
        }
    }

    fn start(&mut self, time: i64, interval: c_ulong) {
        unsafe {
            let mut time: FILETIME = std::mem::transmute(time);
            SetThreadpoolTimerEx(self.timer, &mut time, interval, 0);
        }
    }
}

impl Timer for WinTimer {
    fn new(state: *const TimerState) -> Self {
        Self {
            timer: ptr::null_mut(),
            state,
        }
    }

    #[inline]
    fn reset(&mut self) {
        if !self.timer.is_null() {
            unsafe {
                //Winapi Note:
                //In some cases, callback functions might run after an application closes the threadpool timer.
                //To prevent this behavior, an application should call SetThreadpoolTimerEx
                //with the pftDueTime parameter set to NULL and the msPeriod and msWindowLength parameters set to 0.
                //See https://docs.microsoft.com/en-gb/windows/desktop/api/threadpoolapiset/nf-threadpoolapiset-closethreadpooltimer
                SetThreadpoolTimerEx(self.timer, ptr::null_mut(), 0, 0);
                WaitForThreadpoolTimerCallbacks(self.timer, 1);
                CloseThreadpoolTimer(self.timer);
            }

            self.timer = ptr::null_mut();
        }
    }

    fn start_delay(&mut self, timeout: time::Duration) {
        self.init_self();

        let mut ticks = i64::from(timeout.subsec_nanos() / 100);
        ticks += (timeout.as_secs() * 10_000_000) as i64;
        let ticks = -ticks;

        self.start(ticks, 0);
    }

    fn start_interval(&mut self, interval: time::Duration) {
        self.init_self();

        let mut ticks = i64::from(interval.subsec_nanos() / 100);
        ticks += (interval.as_secs() * 10_000_000) as i64;
        let millis = (ticks / 10_000) as u32;
        let ticks = -ticks;

        self.start(ticks, millis);
    }

    fn state(&self) -> &TimerState {
        match unsafe { self.state.as_ref() } {
            Some(state) => state,
            None => unreach!(),
        }
    }
}

impl Drop for WinTimer {
    fn drop(&mut self) {
        self.reset();
    }
}
