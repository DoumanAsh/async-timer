//! Linux timer Implementation based on `timer_fd`

use core::{task, mem, ptr, time};
use core::pin::Pin;
use std::io;

use crate::{TimerState, Timer};

use libc::{c_int};

struct RawTimer(c_int);

impl RawTimer {
    fn new() -> Self {
        let fd = unsafe { libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_NONBLOCK) };

        assert_ne!(fd, -1);
        Self(fd)
    }

    fn set(&mut self, timer: libc::itimerspec) {
        let ret = unsafe { libc::timerfd_settime(self.0, 0, &timer, ptr::null_mut()) };
        assert_ne!(ret, -1);
    }

    fn read(&mut self) -> usize {
        let mut read_num = 0u64;
        let _ = unsafe { libc::read(self.0, &mut read_num as *mut u64 as *mut _, 8) };
        read_num as usize
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

///Web Based timer
pub struct TimerFd {
    inner: romio::raw::PollEvented<RawTimer>,
    state: TimerState,
}

impl TimerFd {
    fn init_timer(&mut self, timeout: time::Duration, is_interval: bool) {
        let it_value = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_nsec: timeout.subsec_nanos() as libc::suseconds_t,
        };

        let it_interval = match is_interval {
            false => unsafe { mem::zeroed() },
            true => it_value.clone()
        };

        let new_value = libc::itimerspec {
            it_interval,
            it_value,
        };

        self.inner.get_mut().set(new_value);
    }
}

impl Timer for TimerFd {
    fn new() -> Self {
        Self {
            inner: romio::raw::PollEvented::new(RawTimer::new()),
            state: TimerState::new(),
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.inner.get_mut().set(unsafe { mem::zeroed() });
    }

    fn start_delay(&mut self, timeout: time::Duration) {
        self.init_timer(timeout, false);
    }

    fn start_interval(&mut self, interval: time::Duration) {
        self.init_timer(interval, true);
    }

    fn state(&self) -> &TimerState {
        &self.state
    }

    fn poll(&mut self, ctx: &mut task::Context) -> task::Poll<()> {
        match Pin::new(&mut self.inner).poll_read_ready(ctx) {
            task::Poll::Pending => return task::Poll::Pending,
            task::Poll::Ready(_) => {
                let _ = Pin::new(&mut self.inner).clear_read_ready(ctx);
                let _ = self.inner.get_mut().read();
                task::Poll::Ready(())
            }
        }
    }
}

unsafe impl Sync for TimerFd {}
unsafe impl Send for TimerFd {}

