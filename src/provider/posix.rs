//! POSIX based timer

use core::{mem, ptr, time};

use crate::{TimerState, Timer};

use libc::{c_int, c_void, clockid_t, siginfo_t, suseconds_t, time_t, timespec, CLOCK_MONOTONIC};

#[repr(C)]
struct itimerspec {
    it_interval: timespec,
    it_value: timespec,
}

extern "C" {
    fn timer_create(clockid: clockid_t, sevp: *mut libc::sigevent, timerid: *mut usize) -> c_int;
    fn timer_settime(timerid: usize, flags: c_int, new_value: *const itimerspec, old_value: *mut itimerspec) -> c_int;
    fn timer_delete(timerid: usize);
}

const TIMER_SIG: c_int = 40;
const INTERVAL_SIG: c_int = TIMER_SIG + 1;

unsafe fn get_value(info: *mut siginfo_t) -> *const TimerState {
    //TODO: Support BSD, but this shit requires some hacking due to slightly different struct
    //      Most likely needs libc to sort out this shit
    let raw_bytes = (*info)._pad;
    let val: libc::sigval = ptr::read(raw_bytes[3..].as_ptr() as *const _);

    val.sival_ptr as *const TimerState
}

unsafe extern "C" fn timer_handler(_sig: c_int, si: *mut siginfo_t, _uc: *mut c_void) {
    let state = get_value(si);

    (*state).wake();
}

unsafe extern "C" fn interval_handler(_sig: c_int, si: *mut siginfo_t, _uc: *mut c_void) {
    let state = get_value(si);

    (*state).wake_by_ref();
}

fn init() {
    let mut sa_mask: libc::sigset_t = unsafe { mem::uninitialized() };
    unsafe {
        libc::sigemptyset(&mut sa_mask);
    }

    let timer_sig = libc::sigaction {
        sa_flags: libc::SA_SIGINFO,
        sa_sigaction: timer_handler as usize,
        sa_mask,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        sa_restorer: None,
    };

    unsafe {
        assert_ne!(libc::sigaction(TIMER_SIG, &timer_sig, ptr::null_mut()), -1);
    }

    let interval_sig = libc::sigaction {
        sa_flags: libc::SA_SIGINFO,
        sa_sigaction: interval_handler as usize,
        sa_mask,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        sa_restorer: None,
    };

    unsafe {
        assert_ne!(libc::sigaction(INTERVAL_SIG, &interval_sig, ptr::null_mut()), -1);
    }
}

lazy_static::lazy_static! {
    static ref RUNTIME: () = {
        init();
    };
}

///Posix Timer
///
///Currently implemented only for `Linux` and `Android` as BSD systems
///proved to be a bit  problematic
pub struct PosixTimer {
    handle: usize,
    state: *const TimerState,
}

impl PosixTimer {
    fn init_raw_timer(&mut self, sigev_signo: c_int, state: *const TimerState) {
        self.reset();

        let mut event: libc::sigevent = unsafe { mem::zeroed() };

        event.sigev_value = libc::sigval {
            sival_ptr: state as *mut _,
        };
        event.sigev_signo = sigev_signo;
        event.sigev_notify = libc::SIGEV_SIGNAL;
        event.sigev_notify_thread_id = 0;

        unsafe {
            assert_eq!(timer_create(CLOCK_MONOTONIC, &mut event, &mut self.handle), 0);
        }
    }

    fn init_timer(&mut self, timeout: time::Duration, is_interval: bool) {
        let it_value = timespec {
            tv_sec: timeout.as_secs() as time_t,
            tv_nsec: timeout.subsec_nanos() as suseconds_t,
        };

        let (signal, it_interval) = match is_interval {
            false => (TIMER_SIG, unsafe { mem::zeroed() }),
            true => (INTERVAL_SIG, it_value.clone())
        };

        let mut new_value = itimerspec {
            it_interval,
            it_value,
        };

        self.init_raw_timer(signal, self.state);

        unsafe {
            assert_eq!(timer_settime(self.handle, 0, &mut new_value, ptr::null_mut()), 0);
        }
    }
}

impl Timer for PosixTimer {
    fn new() -> Self {
        lazy_static::initialize(&RUNTIME);

        Self {
            handle: 0,
            state: Box::into_raw(Box::new(TimerState::new())),
        }
    }

    #[inline]
    fn reset(&mut self) {
        if self.handle != 0 {
            unsafe {
                //Cancel old timer, if any
                timer_settime(self.handle, 0, &mut mem::zeroed(), ptr::null_mut());
                timer_delete(self.handle);
            }
            self.handle = 0;
        }
    }

    #[inline]
    fn start_delay(&mut self, timeout: time::Duration) {
        self.init_timer(timeout, false);
    }

    #[inline]
    fn start_interval(&mut self, interval: time::Duration) {
        self.init_timer(interval, true);
    }

    fn state(&self) -> &TimerState {
        match unsafe { self.state.as_ref() } {
            Some(state) => state,
            None => unreach!(),
        }
    }
}

impl Drop for PosixTimer {
    fn drop(&mut self) {
        self.reset();
        unsafe { Box::from_raw(self.state as *mut TimerState) };
    }
}

unsafe impl Send for PosixTimer {}
unsafe impl Sync for PosixTimer {}
