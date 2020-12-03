//! Posix based timer

use core::{mem, ptr, time, task};
use core::pin::Pin;
use core::future::Future;

use crate::state::TimerState;
use crate::alloc::boxed::Box;

mod ffi {
    use super::*;

    #[allow(non_camel_case_types)]
    pub type timer_t = usize;

    #[cfg(feature = "c_wrapper")]
    pub unsafe extern "C" fn timer_handler(value: libc::sigval) {
        let state = value.sival_ptr as *const TimerState;
        (*state).wake();
    }

    #[cfg(not(feature = "c_wrapper"))]
    pub unsafe extern "C" fn timer_handler(_sig: libc::c_int, si: *mut libc::siginfo_t, _uc: *mut libc::c_void) {
        let state = (*si).si_value().sival_ptr as *const TimerState;
        (*state).wake();
    }

    #[repr(C)]
    pub struct itimerspec {
        pub it_interval: libc::timespec,
        pub it_value: libc::timespec,
    }

    extern "C" {
        #[allow(unused)]
        pub fn timer_create(clockid: libc::clockid_t, sevp: *mut libc::sigevent, timerid: *mut timer_t) -> libc::c_int;
        pub fn timer_settime(timerid: timer_t, flags: libc::c_int, new_value: *const itimerspec, old_value: *mut itimerspec) -> libc::c_int;
        pub fn timer_delete(timerid: timer_t);
    }
}

#[cfg(not(feature = "c_wrapper"))]
const TIMER_SIG: libc::c_int = 40;

#[cfg(not(feature = "c_wrapper"))]
fn init_sig() {
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

#[cfg(feature = "c_wrapper")]
fn time_create(state: *mut TimerState) -> ffi::timer_t {
    #[link(name = "posix_wrapper", lind = "static")]
    extern "C" {
        fn posix_timer(_: Option<unsafe extern "C" fn(value: libc::sigval)>, _: *mut libc::c_void) -> ffi::timer_t;
    }

    let res = unsafe {
        posix_timer(Some(ffi::timer_handler), state as *mut libc::c_void)
    };

    os_assert!(res != 0);
    res
}

#[cfg(not(feature = "c_wrapper"))]
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
        os_assert!(ffi::timer_create(libc::CLOCK_REALTIME, &mut event, res.as_mut_ptr()) == 0);
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
///When using `c_wrapper` feature implementation uses C shim to create timers which call callbacks
///from a separate thread.
///
///Without it, callback is called from signal handler which limits usable operations within the
///callback.
pub struct PosixTimer {
    state: State,
}

impl PosixTimer {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl super::Timer for PosixTimer {
    #[inline(always)]
    fn new(timeout: time::Duration) -> Self {
        assert_time!(timeout);
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

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(fd, ref mut state) => {
                state.reset();
                set_timer_value(*fd, new_value);
            }
        }
    }

    fn restart_ctx(&mut self, new_value: time::Duration, waker: &task::Waker) {
        assert_time!(new_value);

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(fd, ref mut state) => {
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
                ffi::timer_settime(fd, 0, &mut mem::zeroed(), ptr::null_mut());
            }
        }
    }
}

impl super::SyncTimer for PosixTimer {
    fn init<R, F: Fn(&TimerState) -> R>(&mut self, init: F) -> R {
        #[cfg(not(feature = "c_wrapper"))]
        {
            extern crate std;
            static RUNTIME: std::sync::Once = std::sync::Once::new();
            RUNTIME.call_once(init_sig);
        }

        if let State::Init(timeout) = self.state {
            let state = Box::into_raw(Box::new(TimerState::new()));
            let fd = time_create(state);

            let state = unsafe { Box::from_raw(state) };
            init(&state);

            set_timer_value(fd, timeout);

            self.state = State::Running(fd, state)
        }

        match &self.state {
            State::Running(_, ref state) => init(state),
            State::Init(_) => unreach!(),
        }
    }
}

impl Future for PosixTimer {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        crate::timer::poll_sync(self.get_mut(), ctx)
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
