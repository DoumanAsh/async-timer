//! Posix based timer

#[cfg(feature = "no_std")]
core::compile_error!("no_std is not supported for posix implementation");

use core::future::Future;
use core::pin::Pin;
use core::{mem, ptr, time, task};

use crate::state::TimerState;
use crate::alloc::boxed::Box;

mod ffi {
    use super::*;

    #[allow(non_camel_case_types)]
    pub type timer_t = usize;

    #[inline(always)]
    unsafe fn get_value(info: *mut libc::siginfo_t) -> *const TimerState {
        let value = (*info).si_value();

        value.sival_ptr as *const TimerState
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
        pub fn timer_create(clockid: libc::clockid_t, sevp: *mut libc::sigevent, timerid: *mut timer_t) -> libc::c_int;
        pub fn timer_settime(timerid: timer_t, flags: libc::c_int, new_value: *const itimerspec, old_value: *mut itimerspec) -> libc::c_int;
        pub fn timer_delete(timerid: timer_t);
    }
}

const TIMER_SIG: libc::c_int = 40;

fn init() {
    let mut sa_mask = mem::MaybeUninit::<libc::sigset_t>::uninit();
    unsafe {
        libc::sigemptyset(sa_mask.as_mut_ptr());
    }

    let timer_sig = libc::sigaction {
        sa_flags: libc::SA_SIGINFO,
        sa_sigaction: ffi::timer_handler as usize,
        sa_mask: unsafe { sa_mask.assume_init() },
        #[cfg(any(target_os = "linux", target_os = "android"))]
        sa_restorer: None,
    };

    unsafe {
        os_assert!(libc::sigaction(TIMER_SIG, &timer_sig, ptr::null_mut()) != -1);
    }
}

fn time_create(state: *mut TimerState) -> ffi::timer_t {
    let mut event: libc::sigevent = unsafe { mem::zeroed() };

    event.sigev_value = libc::sigval {
        sival_ptr: state as *mut _,
    };
    event.sigev_signo = TIMER_SIG;
    //NOTE: Timer handler is invoked by signal handler
    //      Therefore all limitations are applied to your waker.
    //      To be safe we could use thread, but in this case
    //      we cannot really hope for it to be optimal...
    event.sigev_notify = libc::SIGEV_SIGNAL;

    let mut res = mem::MaybeUninit::<ffi::timer_t>::uninit();

    unsafe {
        os_assert!(ffi::timer_create(libc::CLOCK_MONOTONIC, &mut event, res.as_mut_ptr()) == 0);
        res.assume_init()
    }
}

fn set_timer_value(fd: ffi::timer_t, timeout: time::Duration) {
    let it_value = libc::timespec {
        tv_sec: timeout.as_secs() as libc::time_t,
        #[cfg(not(any(target_os = "openbsd", target_os = "netbsd")))]
        tv_nsec: timeout.subsec_nanos() as libc::suseconds_t,
        #[cfg(any(target_os = "openbsd", target_os = "netbsd"))]
        tv_nsec: timeout.subsec_nanos() as libc::c_long,
    };

    let new_value = ffi::itimerspec {
        it_interval: unsafe { mem::zeroed() },
        it_value,
    };

    unsafe {
        os_assert!(ffi::timer_settime(fd, 0, &new_value, ptr::null_mut()) == 0);
    }
}

enum State {
    Init(time::Duration),
    Running(ffi::timer_t, Box<TimerState>),
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
        use crate::std::sync::Once;
        static RUNTIME: Once = Once::new();

        debug_assert!(!(timeout.as_secs() == 0 && timeout.subsec_nanos() == 0), "Zero timeout makes no sense");

        RUNTIME.call_once(init);

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
                ffi::timer_settime(fd, 0, &mut mem::zeroed(), ptr::null_mut());
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

impl Future for PosixTimer {
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
