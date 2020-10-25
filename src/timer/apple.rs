//! Dispatch Source based Timer

use core::{ptr, task, time};
use core::pin::Pin;
use core::future::Future;

use crate::state::TimerState;
use crate::alloc::boxed::Box;

use libc::{c_long, c_ulong, c_void, uintptr_t};

#[allow(non_camel_case_types)]
mod ffi {
    use super::*;

    pub type dispatch_object_t = *const c_void;
    pub type dispatch_queue_t = *const c_void;
    pub type dispatch_source_t = *const c_void;
    pub type dispatch_source_type_t = *const c_void;
    pub type dispatch_time_t = u64;

    pub const DISPATCH_TIME_FOREVER: dispatch_time_t = !0;
    //pub const DISPATCH_WALLTIME_NOW: dispatch_time_t = !1;
    pub const QOS_CLASS_DEFAULT: c_long = 0x15;

    extern "C" {
        pub static _dispatch_source_type_timer: c_long;

        pub fn dispatch_get_global_queue(identifier: c_long, flags: c_ulong) -> dispatch_queue_t;
        pub fn dispatch_source_create(type_: dispatch_source_type_t, handle: uintptr_t, mask: c_ulong, queue: dispatch_queue_t) -> dispatch_source_t;
        pub fn dispatch_source_set_timer(source: dispatch_source_t, start: dispatch_time_t, interval: u64, leeway: u64);
        pub fn dispatch_source_set_event_handler_f(source: dispatch_source_t, handler: unsafe extern "C" fn(*mut c_void));
        pub fn dispatch_set_context(object: dispatch_object_t, context: *mut c_void);
        pub fn dispatch_resume(object: dispatch_object_t);
        pub fn dispatch_suspend(object: dispatch_object_t);
        pub fn dispatch_release(object: dispatch_object_t);
        pub fn dispatch_source_cancel(object: dispatch_object_t);
        pub fn dispatch_walltime(when: *const c_void, delta: i64) -> dispatch_time_t;
    }
}

//TODO: Investigate why sometimes it is called multiple times
unsafe extern "C" fn timer_handler(context: *mut c_void) {
    let state = context as *mut TimerState;

    (*state).wake();
}

struct TimerHandle {
    inner: ffi::dispatch_source_t,
    //Suspension count. Incremented suspend, and decremented on each resume
    s_count: u8,
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        unsafe {
            ffi::dispatch_source_cancel(self.inner);

            //It is error to release while source is suspended
            //So we decrement it
            self.resume();

            ffi::dispatch_release(self.inner);
        }
    }
}

impl TimerHandle {
    fn new(state: *mut TimerState) -> Self {
        let inner = unsafe {
            let queue = ffi::dispatch_get_global_queue(ffi::QOS_CLASS_DEFAULT, 0);
            ffi::dispatch_source_create(&ffi::_dispatch_source_type_timer as *const _ as ffi::dispatch_source_type_t, 0, 0, queue)
        };

        os_assert!(!inner.is_null());

        unsafe {
            ffi::dispatch_source_set_event_handler_f(inner, timer_handler);
            ffi::dispatch_set_context(inner, state as *mut _);
        }

        Self {
            inner,
            //Starts as suspended
            s_count: 1,
        }
    }

    fn suspend(&mut self) {
        if self.s_count == 0 {
            unsafe {
                ffi::dispatch_suspend(self.inner);
            }

            self.s_count += 1;
        }
    }

    fn resume(&mut self) {
        while self.s_count > 0 {
            unsafe {
                ffi::dispatch_resume(self.inner)
            }

            self.s_count -= 1;
        }
    }

    fn set_delay(&mut self, timeout: time::Duration) {
        self.suspend();

        unsafe {
            let start = ffi::dispatch_walltime(ptr::null(), timeout.as_nanos() as i64);
            ffi::dispatch_source_set_timer(self.inner, start, ffi::DISPATCH_TIME_FOREVER, 0);
        }

        self.resume();
    }
}

unsafe impl Send for TimerHandle {}
unsafe impl Sync for TimerHandle {}

enum State {
    Init(time::Duration),
    Running(TimerHandle, Box<TimerState>),
}

///Posix Timer
///
///Currently implemented only for `Linux` and `Android` as BSD systems
///proved to be a bit  problematic
pub struct AppleTimer {
    state: State,
}

impl AppleTimer {
    #[inline]
    ///Creates new instance
    pub const fn new(time: time::Duration) -> Self {
        Self {
            state: State::Init(time),
        }
    }
}

impl super::Timer for AppleTimer {
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
                fd.set_delay(new_value);
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
                fd.set_delay(new_value);
            }
        }
    }

    fn cancel(&mut self) {
        match self.state {
            State::Init(_) => (),
            State::Running(ref mut fd, ref state) => {
                state.cancel();
                fd.suspend();
            }
        }
    }
}

impl super::SyncTimer for AppleTimer {
    fn init<R, F: Fn(&TimerState) -> R>(&mut self, init: F) -> R {
        if let State::Init(timeout) = self.state {
            let state = Box::into_raw(Box::new(TimerState::new()));
            let mut fd = TimerHandle::new(state);

            let state = unsafe { Box::from_raw(state) };
            init(&state);

            fd.set_delay(timeout);

            self.state = State::Running(fd, state)
        }

        match &self.state {
            State::Running(_, ref state) => init(state),
            State::Init(_) => unreach!(),
        }
    }
}

impl Future for AppleTimer {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        crate::timer::poll_sync(self.get_mut(), ctx)
    }
}
