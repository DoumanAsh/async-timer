//! One-shot Timer

use core::{task, time};
use core::marker::Unpin;
use core::future::Future;

///One-shot timer that expires once
pub trait Oneshot: Send + Sync + Unpin + Future {
    ///Creates new instance without actually starting timer.
    ///
    ///Timer should start only on initial `Future::poll`
    fn new(timeout: time::Duration) -> Self;

    ///Cancels ongoing timer, if it is not expired yet.
    fn cancel(&mut self);

    ///Restarts timer with new timeout value.
    ///
    ///If timer is already running, then over-write old value and replaces waker.
    fn restart(&mut self, timeout: &time::Duration, waker: &task::Waker);
}

mod state;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod posix;

#[cfg(any(target_os = "linux", target_os = "android"))]
///Alias to Posix Timer
pub type Timer = posix::PosixTimer;

#[cfg(not(any(windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios")))]
///Dummy Timer
pub type DummyTimer = posix::PosixTimer;
