use core::{task, time};
use core::pin::Pin;
use core::future::Future;
use crate::std::io;

use libc::{c_int};

struct RawTimer(c_int);

impl RawTimer {
    fn new() -> Self {
        let fd = nix::sys::event::kqueue().unwrap_or(-1);

        //If you hit this, then most likely you run into OS imposed limit on file descriptor number
        os_assert!(fd != -1);
        Self(fd)
    }

    fn set(&self, time: time::Duration) {
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
    fn register(&self, poll: &mio::Poll, token: mio::Token, mut interest: mio::Ready, opts: mio::PollOpt) -> io::Result<()> {
        interest.remove(mio::Ready::writable());
        mio::unix::EventedFd(&self.0).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &mio::Poll, token: mio::Token, mut interest: mio::Ready, opts: mio::PollOpt) -> io::Result<()> {
        interest.remove(mio::Ready::writable());
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
    Running(tokio::io::PollEvented<RawTimer>, bool),
}

///Timer based on `kqueue`
pub struct KqueueTimer {
    state: State,
}

impl KqueueTimer {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl super::Timer for KqueueTimer {
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
            State::Running(ref fd, ref mut state) => {
                *state = false;
                fd.get_ref().set(new_value);
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
            State::Running(ref mut fd, _) => fd.get_mut().unset(),
        }
    }
}

impl Future for KqueueTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        loop {
            self.state = match &mut self.state {
                State::Init(ref timeout) => {
                    let fd = tokio::io::PollEvented::new(RawTimer::new()).expect("To create PollEvented");
                    fd.get_ref().set(*timeout);
                    State::Running(fd, false)
                }
                State::Running(ref mut fd, false) => {
                    let fd = Pin::new(fd);
                    match fd.poll_read_ready(ctx, mio::Ready::readable()) {
                        task::Poll::Pending => return task::Poll::Pending,
                        task::Poll::Ready(ready) => match ready.map(|ready| ready.is_readable()).expect("kqueue cannot be ready") {
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
