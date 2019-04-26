//! One-shot Timer

use core::{task, time};
use core::marker::Unpin;
use core::future::Future;

///One-shot timer that expires once
pub trait Oneshot: Send + Sync + Unpin + Future<Output=()> {
    ///Creates new instance without actually starting timer.
    ///
    ///Timer should start only on first `Future::poll`
    fn new(timeout: time::Duration) -> Self;

    ///Cancels ongoing timer, if it is not expired yet.
    fn cancel(&mut self);

    ///Restarts timer with new timeout value.
    ///
    ///If timer is already running, then over-write old value and replaces waker.
    fn restart(&mut self, timeout: &time::Duration, waker: &task::Waker);
}

#[cfg(any(windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios"))]
mod state;

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(windows)]
pub mod win;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod posix;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
#[cfg(all(feature = "romio_on", any(target_os = "linux", target_os = "android")))]
pub mod timer_fd;
#[cfg(all(feature = "romio_on", any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd")))]
pub mod kqueue;
#[cfg(not(any(windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios")))]
pub mod dummy;

#[cfg(all(feature = "romio_on", any(target_os = "linux", target_os = "android")))]
pub use timer_fd::TimerFd;

#[cfg(target_arch = "wasm32")]
///Alias to Web based Timer.
pub type Timer = web::WebTimer;

#[cfg(windows)]
///Alias to Windows Timer
pub type Timer = win::WinTimer;

#[cfg(all(not(feature = "romio_on"), any(target_os = "linux", target_os = "android")))]
///Alias to Posix Timer
pub type Timer = posix::PosixTimer;
#[cfg(all(feature = "romio_on", any(target_os = "linux", target_os = "android")))]
///Alias to Linux `timerfd` Timer
pub type Timer = timer_fd::TimerFd;

#[cfg(all(not(feature = "romio_on"), any(target_os = "macos", target_os = "ios")))]
///Alias to Apple Timer
pub type Timer = apple::AppleTimer;
#[cfg(all(feature = "romio_on", any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd")))]
///Alias to `kqueue` based Timer
pub type Timer = kqueue::KqueueTimer;

#[cfg(not(any(windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios")))]
///Dummy Timer
pub type Timer = dummy::DummyTimer;
