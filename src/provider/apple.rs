//! Timer for Apple OSes

use core::{time};

use crate::{TimerState, Timer};

use libc::{c_long, c_ulong, c_void, int64_t, uint64_t, uintptr_t};

#[allow(non_camel_case_types)]
mod ffi {
    use super::*;

    pub type dispatch_object_t = *const c_void;
    pub type dispatch_queue_t = *const c_void;
    pub type dispatch_source_t = *const c_void;
    pub type dispatch_source_type_t = *const c_void;
    pub type dispatch_time_t = uint64_t;

    pub const DISPATCH_TIME_NOW: dispatch_time_t = 0;
    pub const QOS_CLASS_DEFAULT: c_long = 0x15;

    extern "C" {
        pub static SOURCE_TYPE_TIMER: c_long;

        pub fn dispatch_get_global_queue(identifier: c_long, flags: c_ulong) -> dispatch_queue_t;
        pub fn dispatch_source_create(type_: dispatch_source_type_t, handle: uintptr_t, mask: c_ulong, queue: dispatch_queue_t) -> dispatch_source_t;
        pub fn dispatch_source_set_timer(source: dispatch_source_t, start: dispatch_time_t, interval: uint64_t, leeway: uint64_t);
        pub fn dispatch_source_set_event_handler_f(source: dispatch_source_t, handler: unsafe extern "C" fn(*mut c_void));
        pub fn dispatch_set_context(object: dispatch_object_t, context: *mut c_void);
        pub fn dispatch_resume(object: dispatch_object_t);
        pub fn dispatch_suspend(object: dispatch_object_t);
        pub fn dispatch_release(object: dispatch_object_t);
        pub fn dispatch_source_cancel(object: dispatch_source_t);
        pub fn dispatch_time(when: dispatch_time_t, delta: int64_t) -> dispatch_time_t;
    }
}

unsafe extern "C" fn timer_handler(context: *mut c_void) {
    let state = context as *mut TimerState;

    (*state).wake();
}

unsafe extern "C" fn interval_handler(context: *mut c_void) {
    let state = context as *mut TimerState;

    (*state).wake_by_ref();
}

///Timer based on Apple APIs
pub struct AppleTimer {
    handle: ffi::dispatch_source_t,
    state: *const TimerState,
    is_active: bool,
}

impl Timer for AppleTimer {
    fn new(state: *const TimerState) -> Self {
        let handle = unsafe {
            let queue = ffi::dispatch_get_global_queue(ffi::QOS_CLASS_DEFAULT, 0);
            ffi::dispatch_source_create(&ffi::SOURCE_TYPE_TIMER as *const _ as ffi::dispatch_source_type_t, 0, 0, queue)
        };

        assert!(!handle.is_null());

        Self {
            handle,
            state,
            is_active: false,
        }
    }

    #[inline]
    fn reset(&mut self) {
        if self.is_active {
            unsafe {
                ffi::dispatch_suspend(self.handle);
            }

            self.is_active = false;
        }
    }

    fn start_delay(&mut self, timeout: time::Duration) {
        self.reset();

        unsafe {
            ffi::dispatch_source_set_event_handler_f(self.handle, timer_handler);
            ffi::dispatch_set_context(self.handle, self.state as *mut _);

            let start = ffi::dispatch_time(ffi::DISPATCH_TIME_NOW, timeout.as_nanos() as int64_t);
            ffi::dispatch_source_set_timer(self.handle, start, 0, 0);
            ffi::dispatch_resume(self.handle);

            self.is_active = true;
        }
    }

    fn start_interval(&mut self, interval: time::Duration) {
        self.reset();

        unsafe {
            ffi::dispatch_source_set_event_handler_f(self.handle, interval_handler);
            ffi::dispatch_set_context(self.handle, self.state as *mut _);

            let start = ffi::dispatch_time(ffi::DISPATCH_TIME_NOW, interval.as_nanos() as int64_t);
            ffi::dispatch_source_set_timer(self.handle, start, interval.as_nanos() as uint64_t, 0);
            ffi::dispatch_resume(self.handle);

            self.is_active = true;
        }
    }

    fn state(&self) -> &TimerState {
        match unsafe { self.state.as_ref() } {
            Some(state) => state,
            None => unreach!(),
        }
    }
}

impl Drop for AppleTimer {
    fn drop(&mut self) {
        self.reset();
        unsafe {
            ffi::dispatch_source_cancel(self.handle);
            ffi::dispatch_release(self.handle);
        }
    }
}
unsafe impl Send for AppleTimer {}
unsafe impl Sync for AppleTimer {}
