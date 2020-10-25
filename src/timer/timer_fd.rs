//! Timerfd based implementation
use crate::std::io;
use core::future::Future;
use core::pin::Pin;
use core::{mem, ptr, task, time};

use libc::c_int;

#[cfg(target_os = "android")]
mod sys {
    #[repr(C)]
    pub struct itimerspec {
        pub it_interval: libc::timespec,
        pub it_value: libc::timespec,
    }

    extern "C" {
        pub fn timerfd_create(clockid: libc::clockid_t, flags: libc::c_int) -> libc::c_int;
        pub fn timerfd_settime(timerid: libc::c_int, flags: libc::c_int, new_value: *const itimerspec, old_value: *mut itimerspec) -> libc::c_int;
    }

    pub const TFD_NONBLOCK: libc::c_int = libc::O_NONBLOCK;
}

#[cfg(not(target_os = "android"))]
use libc as sys;

struct RawTimer(c_int);

impl RawTimer {
    fn new() -> Self {
        let fd = unsafe { sys::timerfd_create(libc::CLOCK_MONOTONIC, sys::TFD_NONBLOCK) };

        os_assert!(fd != -1);
        Self(fd)
    }

    fn set(&self, timer: sys::itimerspec) {
        let ret = unsafe { sys::timerfd_settime(self.0, 0, &timer, ptr::null_mut()) };
        os_assert!(ret != -1);
    }

    fn read(&self) -> usize {
        let mut read_num = 0u64;
        match unsafe { libc::read(self.0, &mut read_num as *mut u64 as *mut _, 8) } {
            -1 => {
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::WouldBlock => 0,
                    _ => panic!("Unexpected read error: {}", error),
                }
            }
            _ => read_num as usize,
        }
    }
}

impl mio::Evented for RawTimer {
    fn register(&self, poll: &mio::Poll, token: mio::Token, interest: mio::Ready, opts: mio::PollOpt) -> io::Result<()> {
        mio::unix::EventedFd(&self.0).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &mio::Poll, token: mio::Token, interest: mio::Ready, opts: mio::PollOpt) -> io::Result<()> {
        mio::unix::EventedFd(&self.0).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        mio::unix::EventedFd(&self.0).deregister(poll)
    }
}

impl Drop for RawTimer {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

fn set_timer_value(fd: &RawTimer, timeout: time::Duration) {
    #[cfg(not(target_pointer_width = "64"))]
    use core::convert::TryFrom;

    let it_value = libc::timespec {
        tv_sec: timeout.as_secs() as libc::time_t,
        #[cfg(target_pointer_width = "64")]
        tv_nsec: libc::suseconds_t::from(timeout.subsec_nanos()),
        #[cfg(not(target_pointer_width = "64"))]
        tv_nsec: libc::suseconds_t::try_from(timeout.subsec_nanos()).unwrap_or(libc::suseconds_t::max_value()),
    };

    let new_value = sys::itimerspec {
        it_interval: unsafe { mem::zeroed() },
        it_value,
    };

    fd.set(new_value);
}

enum State {
    Init(time::Duration),
    Running(tokio::io::PollEvented<RawTimer>, bool),
}

///Linux `timerfd` wrapper
pub struct TimerFd {
    state: State,
}

impl TimerFd {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl super::Timer for TimerFd {
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
            State::Running(_, state) => !*state,
        }
    }

    #[inline]
    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, state) => *state
        }
    }

    fn restart(&mut self, new_value: time::Duration) {
        assert_time!(new_value);
        debug_assert!(new_value.as_millis() <= u32::max_value().into());

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(ref mut fd, ref mut state) => {
                *state = false;
                set_timer_value(fd.get_ref(), new_value);
            }
        }
    }

    #[inline(always)]
    fn restart_ctx(&mut self, new_value: time::Duration, _: &task::Waker) {
        self.restart(new_value)
    }

    fn cancel(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(ref mut fd, _) => {
                fd.get_mut().set(unsafe {
                    mem::MaybeUninit::zeroed().assume_init()
                });
            }
        }
    }
}

impl Future for TimerFd {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        loop {
            self.state = match &mut self.state {
                State::Init(ref timeout) => {
                    let fd = tokio::io::PollEvented::new(RawTimer::new()).expect("To create PollEvented");
                    set_timer_value(fd.get_ref(), *timeout);
                    State::Running(fd, false)
                }
                State::Running(ref mut fd, false) => {
                    let fd = Pin::new(fd);
                    match fd.poll_read_ready(ctx, mio::Ready::readable()) {
                        task::Poll::Pending => return task::Poll::Pending,
                        task::Poll::Ready(ready) => match ready.map(|ready| ready.is_readable()).expect("timerfd cannot be ready") {
                            true => {
                                let _ = fd.clear_read_ready(ctx, mio::Ready::readable());
                                match fd.get_mut().get_mut().read() {
                                    0 => return task::Poll::Pending,
                                    _ => return task::Poll::Ready(()),
                                }
                            }
                            false => return task::Poll::Pending,
                        },
                    }
                }
                State::Running(_, true) => return task::Poll::Ready(()),
            }
        }
    }
}
