//! Timer based on `kqueue`

#[cfg(feature = "no_std")]
core::compile_error!("no_std is not supported for kqueue implementation");

use core::{task, time};
use core::pin::Pin;
use core::future::Future;
use std::io;

use libc::{c_int};

struct RawTimer(c_int);

impl RawTimer {
    fn new() -> Self {
        let fd = nix::sys::event::kqueue().unwrap_or(-1);

        assert_ne!(fd, -1);
        Self(fd)
    }

    fn set(&self, time: &time::Duration) {
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

    fn unset(&self) {
        use nix::sys::event::*;

        let flags = EventFlag::EV_DELETE;
        kevent(self.0, &[KEvent::new(1, EventFilter::EVFILT_TIMER, flags, FilterFlag::empty(), 0, 0)], &mut [], 0).expect("To disarm timer");
    }

    fn read(&self) -> usize {
        use nix::sys::event::*;

        let mut ev = [KEvent::new(0, EventFilter::EVFILT_TIMER, EventFlag::empty(), FilterFlag::empty(), 0, 0)];

        kevent(self.0, &[], &mut ev[..], 0).expect("To execute kevent")
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
        let _ = nix::unistd::close(self.0);
    }
}

enum State {
    Init(time::Duration),
    Running(bool),
}

///Timer based on `kqueue`
pub struct KqueueTimer {
    fd: romio::raw::PollEvented<RawTimer>,
    state: State,
}

impl super::Oneshot for KqueueTimer {
    fn new(timeout: time::Duration) -> Self {
        debug_assert!(!(timeout.as_secs() == 0 && timeout.subsec_nanos() == 0), "Zero timeout makes no sense");

        Self {
            fd: romio::raw::PollEvented::new(RawTimer::new()),
            state: State::Init(timeout),
        }
    }

    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(is_finished) => *is_finished,
        }
    }

    fn cancel(&mut self) {
        self.fd.get_mut().unset();
    }

    fn restart(&mut self, new_value: &time::Duration, _: &task::Waker) {
        debug_assert!(!(new_value.as_secs() == 0 && new_value.subsec_nanos() == 0), "Zero timeout makes no sense");

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = *new_value;
            },
            State::Running(ref mut is_finished) => {
                *is_finished = false;
                self.fd.get_ref().set(new_value);
            },
        }
    }
}

impl Future for KqueueTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        loop {
            self.state = match &self.state {
                State::Init(ref timeout) => {
                    self.fd.get_ref().set(timeout);
                    State::Running(false)
                },
                State::Running(false) => match Pin::new(&mut self.fd).poll_read_ready(ctx) {
                    task::Poll::Pending => return task::Poll::Pending,
                    task::Poll::Ready(ready) => match ready.map(|ready| ready.is_readable()).expect("kqueue cannot be ready") {
                        true => {
                            let _ = Pin::new(&mut self.fd).clear_read_ready(ctx);
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
