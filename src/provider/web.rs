//! Web browser based `Timer`

use core::{time};

use crate::{TimerState, Timer};

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn setTimeout(closure: &wasm_bindgen::closure::Closure<FnMut()>, time: u32) -> i32;
    fn clearTimeout(id: i32);

    fn setInterval(closure: &wasm_bindgen::closure::Closure<FnMut()>, time: u32) -> i32;
    fn clearInterval(id: i32);
}

enum TimerHandle {
    None,
    Timeout(i32),
    Interval(i32),
}

impl TimerHandle {
    fn clear(&mut self) {
        match self {
            TimerHandle::Timeout(ref mut val) => clearTimeout(*val),
            TimerHandle::Interval(ref mut val) => clearInterval(*val),
            _ => (),
        }

        *self = TimerHandle::None;
    }
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        self.clear();
    }
}

///Web Based timer
pub struct WebTimer {
    handle: TimerHandle,
    state: *const TimerState,
}

impl Timer for WebTimer {
    fn new(state: *const TimerState) -> Self {
        Self {
            handle: TimerHandle::None,
            state,
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.handle.clear();
    }

    fn start_delay(&mut self, timeout: time::Duration) {
        let timeout = timeout.as_millis() as u32;

        let state = self.state;
        let cb = wasm_bindgen::closure::Closure::once(move || unsafe {
            (*state).wake();
        });
        let handle = setTimeout(&cb, timeout);

        self.handle = TimerHandle::Timeout(handle);
    }

    fn start_interval(&mut self, interval: time::Duration) {
        let interval = interval.as_millis() as u32;

        let state = self.state;
        let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move || unsafe {
            (*state).wake_by_ref();
        }) as Box<dyn FnMut()>);
        let handle = setInterval(&cb, interval);

        self.handle = TimerHandle::Interval(handle);
    }

    fn state(&self) -> &TimerState {
        match unsafe { self.state.as_ref() } {
            Some(state) => state,
            None => unreach!(),
        }
    }
}

impl Drop for WebTimer {
    fn drop(&mut self) {
        self.reset();
    }
}

unsafe impl Send for WebTimer {}
unsafe impl Sync for WebTimer {}
