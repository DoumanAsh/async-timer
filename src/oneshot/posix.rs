//! Posix based timer

use core::future::Future;
use core::pin::Pin;
use core::{mem, ptr, time, task};

use super::state::TimerState;
use crate::alloc::boxed::Box;

type RawFd = usize;

mod ffi {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe fn get_value(info: *mut libc::siginfo_t) -> *const TimerState {
        //TODO: Support BSD, but this shit requires some hacking due to slightly different struct
        //      Most likely needs libc to sort out this shit
        let raw_bytes = (*info)._pad;
        let val: libc::sigval = ptr::read(raw_bytes[3..].as_ptr() as *const _);

        val.sival_ptr as *const TimerState
    }

    pub unsafe extern "C" fn timer_handler(_sig: libc::c_int, si: *mut libc::siginfo_t, _uc: *mut libc::c_void) {
        let state = get_value(si);

        (*state).wake();
    }

    #[repr(C)]
    pub struct itimerspec {
        pub it_interval: libc::timespec,
        pub it_value: libc::timespec,
    }

    extern "C" {
        pub fn timer_create(clockid: libc::clockid_t, sevp: *mut libc::sigevent, timerid: *mut RawFd) -> libc::c_int;
        pub fn timer_settime(timerid: RawFd, flags: libc::c_int, new_value: *const itimerspec, old_value: *mut itimerspec) -> libc::c_int;
        pub fn timer_delete(timerid: RawFd);
    }
}

const TIMER_SIG: libc::c_int = 40;

fn init() {
    let mut sa_mask: libc::sigset_t = unsafe { mem::uninitialized() };
    unsafe {
        libc::sigemptyset(&mut sa_mask);
    }

    let timer_sig = libc::sigaction {
        sa_flags: libc::SA_SIGINFO,
        sa_sigaction: ffi::timer_handler as usize,
        sa_mask,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        sa_restorer: None,
    };

    unsafe {
        assert_ne!(libc::sigaction(TIMER_SIG, &timer_sig, ptr::null_mut()), -1);
    }
}

lazy_static::lazy_static! {
    static ref RUNTIME: () = {
        init();
    };
}

fn time_create(state: *mut TimerState) -> RawFd {
    let mut event: libc::sigevent = unsafe { mem::zeroed() };

    event.sigev_value = libc::sigval {
        sival_ptr: state as *mut _,
    };
    event.sigev_signo = TIMER_SIG;
    event.sigev_notify = libc::SIGEV_SIGNAL;
    event.sigev_notify_thread_id = 0;

    let mut res = 0;

    unsafe {
        assert_eq!(ffi::timer_create(libc::CLOCK_MONOTONIC, &mut event, &mut res), 0);
    }

    res
}

fn set_timer_value(fd: RawFd, timeout: &time::Duration) {
    let it_value = libc::timespec {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_nsec: libc::suseconds_t::from(timeout.subsec_nanos()),
    };

    let new_value = ffi::itimerspec {
        it_interval: unsafe { mem::zeroed() },
        it_value,
    };

    unsafe {
        assert_eq!(ffi::timer_settime(fd, 0, &new_value, ptr::null_mut()), 0);
    }
}

enum State {
    Init(time::Duration),
    Running(RawFd, Box<TimerState>),
}

///Posix Timer
///
///Currently implemented only for `Linux` and `Android` as BSD systems
///proved to be a bit  problematic
pub struct PosixTimer {
    state: State,
}

impl super::Oneshot for PosixTimer {
    fn new(timeout: time::Duration) -> Self {
        debug_assert!(!(timeout.as_secs() == 0 && timeout.subsec_nanos() == 0), "Zero timeout makes no sense");
        lazy_static::initialize(&RUNTIME);

        Self {
            state: State::Init(timeout),
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
                ffi::timer_settime(fd, 0, &mut mem::zeroed(), ptr::null_mut());
            }
        }
    }

    fn restart(&mut self, new_value: &time::Duration, waker: &task::Waker) {
        debug_assert!(!(new_value.as_secs() == 0 && new_value.subsec_nanos() == 0), "Zero timeout makes no sense");

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = *new_value;
            },
            State::Running(fd, ref mut state) => {
                state.register(waker);
                set_timer_value(*fd, new_value);
            }
        }
    }
}

impl Future for PosixTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        self.state = match &self.state {
            State::Init(ref timeout) => {
                let state = Box::into_raw(Box::new(TimerState::new()));
                let fd = time_create(state);

                let state = unsafe { Box::from_raw(state) };
                state.register(ctx.waker());

                set_timer_value(fd, timeout);

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

impl Drop for PosixTimer {
    fn drop(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(fd, _) => unsafe {
                ffi::timer_delete(fd);
            }
        }
    }
}
