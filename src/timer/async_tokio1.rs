use tokio_1 as tokio;

use tokio::io::unix::AsyncFd;
use libc::{c_int};

use core::{task, time};
use core::pin::Pin;
use core::future::Future;

pub trait TimerFd: crate::std::os::unix::io::AsRawFd + Sync + Send + Unpin {
    fn new() -> Self;
    fn set(&mut self, time: time::Duration);
    fn unset(&mut self);
    fn read(&mut self) -> usize;
}

///Wrapper over fd based timer.
pub struct RawTimer(c_int);

impl crate::std::os::unix::io::AsRawFd for RawTimer {
    #[inline(always)]
    fn as_raw_fd(&self) -> c_int {
        self.0
    }
}

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

#[cfg(target_os = "linux")]
use libc as sys;

#[cfg(any(target_os = "linux", target_os = "android"))]
impl TimerFd for RawTimer {
    fn new() -> Self {
        let fd = unsafe { sys::timerfd_create(libc::CLOCK_MONOTONIC, sys::TFD_NONBLOCK) };

        os_assert!(fd != -1);
        Self(fd)
    }

    fn set(&mut self, timeout: time::Duration) {
        #[cfg(not(target_pointer_width = "64"))]
        use core::convert::TryFrom;

        let it_value = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            #[cfg(target_pointer_width = "64")]
            tv_nsec: libc::suseconds_t::from(timeout.subsec_nanos()),
            #[cfg(not(target_pointer_width = "64"))]
            tv_nsec: libc::suseconds_t::try_from(timeout.subsec_nanos()).unwrap_or(libc::suseconds_t::max_value()),
        };

        let timer = sys::itimerspec {
            it_interval: unsafe { core::mem::MaybeUninit::zeroed().assume_init() },
            it_value,
        };

        let ret = unsafe { sys::timerfd_settime(self.0, 0, &timer, core::ptr::null_mut()) };
        os_assert!(ret != -1);
    }

    #[inline]
    fn unset(&mut self) {
        self.set(time::Duration::from_secs(0));
    }

    fn read(&mut self) -> usize {
        let mut read_num = 0u64;
        match unsafe { libc::read(self.0, &mut read_num as *mut u64 as *mut _, 8) } {
            -1 => {
                let error = crate::std::io::Error::last_os_error();
                match error.kind() {
                    crate::std::io::ErrorKind::WouldBlock => 0,
                    _ => panic!("Unexpected read error: {}", error),
                }
            }
            _ => read_num as usize,
        }
    }
}

#[cfg(any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd"))]
impl TimerFd for RawTimer {
    fn new() -> Self {
        let fd = nix::sys::event::kqueue().unwrap_or(-1);

        //If you hit this, then most likely you run into OS imposed limit on file descriptor number
        os_assert!(fd != -1);
        Self(fd)
    }

    fn set(&mut self, time: time::Duration) {
        use nix::sys::event::*;

        let flags = EventFlag::EV_ADD | EventFlag::EV_ENABLE | EventFlag::EV_ONESHOT;
        let mut time = time.as_nanos();
        let mut unit = FilterFlag::NOTE_NSECONDS;

        if time > isize::max_value() as u128 {
            unit = FilterFlag::NOTE_USECONDS;
            time /= 1_000;
        }
        if time > isize::max_value() as u128 {
            unit = FilterFlag::empty(); // default is milliseconds
            time /= 1_000;
        }
        if time > isize::max_value() as u128 {
            unit = FilterFlag::NOTE_SECONDS;
            time /= 1_000;
        }

        let time = time as isize;
        kevent(self.0, &[KEvent::new(1, EventFilter::EVFILT_TIMER, flags, unit, time, 0)], &mut [], 0).expect("To arm timer");
    }

    fn unset(&mut self) {
        use nix::sys::event::*;

        let flags = EventFlag::EV_DELETE;
        kevent(self.0, &[KEvent::new(1, EventFilter::EVFILT_TIMER, flags, FilterFlag::empty(), 0, 0)], &mut [], 0).expect("To disarm timer");
    }

    fn read(&mut self) -> usize {
        use nix::sys::event::*;

        let mut ev = [KEvent::new(0, EventFilter::EVFILT_TIMER, EventFlag::empty(), FilterFlag::empty(), 0, 0)];

        kevent(self.0, &[], &mut ev[..], 0).expect("To execute kevent")
    }
}

enum State<T> {
    Init(time::Duration),
    Running(T, bool),
}

///Timer implemented on top of `AsyncFd`
pub struct AsyncTokioTimer<T: TimerFd> {
    state: State<AsyncFd<T>>
}

impl AsyncTokioTimer<RawTimer> {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl<T: TimerFd> super::Timer for AsyncTokioTimer<T> {
    #[inline(always)]
    fn new(timeout: time::Duration) -> Self {
        assert_time!(timeout);
        debug_assert!(timeout.as_millis() <= u32::max_value().into());
        Self {
            state: State::Init(timeout),
        }
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
                fd.get_mut().set(new_value);
            }
        }
    }

    #[inline(always)]
    fn restart_ctx(&mut self, new_value: time::Duration, _: &task::Waker) {
        self.restart(new_value)
    }

    fn cancel(&mut self) {
        unreachable!();
    }
}

impl<T: TimerFd> Future for AsyncTokioTimer<T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        if let State::Init(ref timeout) = &self.state {
            let mut fd = AsyncFd::with_interest(T::new(), tokio::io::Interest::READABLE).expect("To create AsyncFd");
            fd.get_mut().set(*timeout);
            self.state = State::Running(fd, false)
        };

        if let State::Running(ref mut fd, ref mut state) = &mut self.state {
            if *state {
                return task::Poll::Ready(());
            }

            let fd = Pin::new(fd);
            match fd.poll_read_ready(ctx) {
                task::Poll::Pending => return task::Poll::Pending,
                task::Poll::Ready(ready) => {
                    let mut ready = ready.expect("Unable to read async timer's fd");
                    //technically we should read first, but we cannot borrow as mut then
                    ready.clear_ready();

                    match fd.get_mut().get_mut().read() {
                        0 => {
                            *state = false;
                            return task::Poll::Pending
                        },
                        _ => {
                            *state = true;
                            return task::Poll::Ready(())
                        }
                    }
                }
            }
        } else {
            unreach!();
        }
    }
}

///Timer based on tokio's `AsyncFd`
pub type AsyncTimer = AsyncTokioTimer<RawTimer>;
