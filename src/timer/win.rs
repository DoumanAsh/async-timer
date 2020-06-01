//! Windows API based timer

use core::{task, time};
use core::pin::Pin;
use core::future::Future;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::state::{TimerState};
use crate::alloc::boxed::Box;

//We can probably achieve even more with `NtSetTimerResolution` but it is still at most 0.5ms so
//who cares
pub(crate) const RESOLUTION: AtomicU32 = AtomicU32::new(10); //ms

type Callback = Option<unsafe extern "system" fn(_: u32, _:u32, data: usize, _: usize, _: usize)>;

#[link(name = "winmm", lind = "static")]
extern "system" {
    fn timeKillEvent(id: u32) -> u32;
    fn timeSetEvent(delay: u32, resolution: u32, cb: Callback, data: usize, typ: u32) -> u32;
}

unsafe extern "system" fn timer_callback(_: u32, _: u32, data: usize, _: usize, _: usize) {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_ptr_alignment))]
    let state = data as *mut () as *mut TimerState;

    (*state).wake();
}

fn create_timer(state: *mut TimerState, timeout: time::Duration) -> u32 {
    let res = unsafe {
        timeSetEvent(timeout.as_millis() as u32, RESOLUTION.load(Ordering::Relaxed), Some(timer_callback), state as usize, 0)
    };

    os_assert!(res != 0);

    res
}

enum State {
    Init(time::Duration),
    Running(u32, *mut TimerState),
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

///Windows Native timer
pub struct WinTimer {
    state: State,
}

impl WinTimer {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl super::Timer for WinTimer {
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
            State::Running(_, ref state) => unsafe { !(**state).is_done() },
        }
    }

    #[inline]
    fn is_expired(&self) -> bool {
        match &self.state {
            State::Init(_) => false,
            State::Running(_, ref state) => unsafe { (**state).is_done() },
        }
    }

    fn restart(&mut self, new_value: time::Duration) {
        assert_time!(new_value);
        debug_assert!(new_value.as_millis() <= u32::max_value().into());

        match &mut self.state {
            State::Init(ref mut timeout) => {
                *timeout = new_value;
            },
            State::Running(ref mut fd, ref state) => unsafe {
                (**state).reset();
                timeKillEvent(*fd);
                *fd = create_timer(*state, new_value);
            }
        }
    }

    fn cancel(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(ref mut fd, _) => unsafe {
                timeKillEvent(*fd);
                *fd = 0;
            }
        }
    }
}

impl super::SyncTimer for WinTimer {
    fn init<R, F: Fn(&TimerState) -> R>(&mut self, init: F) -> R {
        if let State::Init(timeout) = self.state {
            let state = Box::into_raw(Box::new(TimerState::new()));
            unsafe {
                init(&*state);
            }
            let fd = create_timer(state, timeout);

            self.state = State::Running(fd, state)
        }

        match &self.state {
            State::Running(_, ref state) => init(unsafe { &**state }),
            State::Init(_) => unreach!(),
        }
    }
}

impl Future for WinTimer {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        crate::timer::poll_sync(self.get_mut(), ctx)
    }
}

impl Drop for WinTimer {
    fn drop(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(fd, state) => unsafe {
                timeKillEvent(fd);
                Box::from_raw(state as *mut TimerState);
            }
        }
    }
}
