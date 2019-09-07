//! Linux `timerfd` implementation

#[cfg(feature = "no_std")]
core::compile_error!("no_std is not supported for timerfd implementation");

use core::{task, mem, ptr, time};
use core::pin::Pin;
use core::future::Future;
use crate::std::io;

use libc::{c_int};

struct RawTimer(c_int);

impl RawTimer {
    fn new() -> Self {
        let fd = unsafe { libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_NONBLOCK) };

        assert_ne!(fd, -1);
        Self(fd)
    }

    fn set(&self, timer: libc::itimerspec) {
        let ret = unsafe { libc::timerfd_settime(self.0, 0, &timer, ptr::null_mut()) };
        assert_ne!(ret, -1);
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
            },
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

enum State {
    Init(time::Duration),
    Running(bool),
}

fn set_timer_value(fd: &RawTimer, timeout: &time::Duration) {
    let it_value = libc::timespec {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_nsec: libc::suseconds_t::from(timeout.subsec_nanos()),
    };

    let new_value = libc::itimerspec {
        it_interval: unsafe { mem::zeroed() },
        it_value,
    };

    fd.set(new_value);
}

///Linux `timerfd` wrapper
pub struct TimerFd {
    fd: tokio_net::util::PollEvented<RawTimer>,
    state: State,
}

impl super::Oneshot for TimerFd {
    fn new(timeout: time::Duration) -> Self {
        debug_assert!(!(timeout.as_secs() == 0 && timeout.subsec_nanos() == 0), "Zero timeout makes no sense");

        Self {
            fd: tokio_net::util::PollEvented::new(RawTimer::new()),
            state: State::Init(timeout),
        }
    }

    fn is_ticking(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(is_finished) => !*is_finished,
        }
    }

    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(is_finished) => *is_finished,
        }
    }

    fn cancel(&mut self) {
        self.fd.get_mut().set(unsafe { mem::zeroed() });
    }

    fn restart(&mut self, new_value: &time::Duration, _: &task::Waker) {
        debug_assert!(!(new_value.as_secs() == 0 && new_value.subsec_nanos() == 0), "Zero timeout makes no sense");

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = *new_value;
            },
            State::Running(ref mut is_finished) => {
                *is_finished = false;
                set_timer_value(&self.fd.get_ref(), new_value);
            },
        }
    }
}

impl Future for TimerFd {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        loop {
            self.state = match &self.state {
                State::Init(ref timeout) => {
                    set_timer_value(self.fd.get_ref(), timeout);
                    State::Running(false)
                },
                State::Running(false) => match Pin::new(&mut self.fd).poll_read_ready(ctx, mio::Ready::readable()) {
                    task::Poll::Pending => return task::Poll::Pending,
                    task::Poll::Ready(ready) => match ready.map(|ready| ready.is_readable()).expect("timerfd cannot be ready") {
                        true => {
                            let _ = Pin::new(&mut self.fd).clear_read_ready(ctx, mio::Ready::readable());
                            match self.fd.get_mut().read() {
                                0 => return task::Poll::Pending,
                                _ => return task::Poll::Ready(()),
                            }
                        },
                        false => return task::Poll::Pending,
                    }
                },
                State::Running(true) => return task::Poll::Ready(()),
            }
        }
    }
}
