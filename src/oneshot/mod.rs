//! One-shot Timer

use core::{task, time};
use core::marker::Unpin;
use core::future::Future;

///One-shot timer that expires once
///
///Trait itself describes `Future` that resolves after `timeout`
///
///Most common platforms are supplied via alias [Timer](type.Timer.html)
///
///## Common implementations:
///
///- Windows uses thread pooled timer
///- Apple systems uses dispatch source API
///- Posix compatible `timer_create`, currently available only on Linux systems
///- Wasm uses Web API `SetTimeout`
///- Dummy timer is used  when no implementation is available. Panics when used.
///
///## Feature `romio_on`
///
///- Linux uses `timerfd_create`, replaces Posix tiemr when enabled.
///- Other unix systems uses `kqueue`, replaces Apple timer when enabled.
///
///```rust, no_run
/// use async_timer::oneshot::{Oneshot, Timer};
/// use futures::executor::block_on;
///
/// use std::time;
///
/// let work = Timer::new(time::Duration::from_secs(2));
///
/// let before = time::SystemTime::now();
/// block_on(work);
/// let after = time::SystemTime::now();
/// let diff = after.duration_since(before).unwrap();
///
/// assert_eq!(diff.as_secs(), 2);
///
///```
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
#[cfg(not(any(
windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios",
all(feature = "romio_on", any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd"))
)))]
pub mod dummy;
mod extra;

pub use extra::NeverTimer;

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

#[cfg(not(any(
windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios",
all(feature = "romio_on", any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd", target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd"))
)))]
///Dummy Timer
pub type Timer = dummy::DummyTimer;
