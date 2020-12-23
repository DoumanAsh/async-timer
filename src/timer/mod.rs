//!Raw Timer

use core::{time, task};
use core::future::Future;

use crate::state::TimerState;

///Timer
///
///## Common implementations:
///
///- Windows uses thread pooled timer
///- Apple systems uses dispatch source API
///- Posix compatible `timer_create`, available on major Posix-compliant systems. Depends on availability of `siginfo_t::si_value` method.
///- Wasm uses Web API `SetTimeout`
///- Dummy timer is used  when no implementation is available. Panics when used.
///
///## Usage
///
///```no_run
///use async_timer::timer::{Timer, new_timer};
///
///use core::time;
///use core::pin::Pin;
///
///async fn do_something() {
///    let mut work = new_timer(time::Duration::from_secs(2));
///    assert!(!work.is_ticking()); //Timer starts only on initial poll
///    assert!(!work.is_expired());
///    Pin::new(&mut work).await; //Remember await consumes future, and we'd prefer to avoid that in order to re-use timer
///    assert!(work.is_expired());
///
///}
///```
pub trait Timer: Send + Sync + Unpin + Future<Output=()> {
    ///Creates new instance
    fn new(timeout: time::Duration) -> Self;

    ///Returns whether timer is ongoing.
    ///
    ///Note that if it returns `false` it doesn't mean that `is_expired` will return `true`
    ///as initially timer may not be armed.
    fn is_ticking(&self) -> bool;

    ///Returns whether timer has expired.
    fn is_expired(&self) -> bool;

    ///Restarts timer with new timeout value.
    fn restart(&mut self, timeout: time::Duration);

    ///Restarts timer with new timeout value and waker.
    fn restart_ctx(&mut self, timeout: time::Duration, waker: &task::Waker);

    ///Cancels timer, if it is still ongoing.
    fn cancel(&mut self);
}

///Describes timer interface that doesn't require async event loop.
///
///In most cases these timers are implemented by OS calling back a provided callback.
///
///As notification is not done via event loop, timer has to store callback in its own state.
///
///Whenever async timer relies on async event loop to handle notifications
///
///## Usage
///
///```
///use async_timer::timer::{Timer, SyncTimer, new_sync_timer};
///
///use core::sync::atomic::{AtomicBool, Ordering};
///use core::time;
///
///use std::thread;
///
///static EXPIRED: AtomicBool = AtomicBool::new(false);
///fn on_expire() {
///    EXPIRED.store(true, Ordering::Release);
///}
///
///let mut work = new_sync_timer(time::Duration::from_secs(1));
///assert!(!work.is_ticking());
///assert!(!work.is_expired());
///
///work.init(|state| state.register(on_expire as fn()));
///work.tick();
///
///assert!(work.is_ticking());
///assert!(!work.is_expired());
///thread::sleep(time::Duration::from_millis(1250)); //timer is not necessary expires immediately
///
///assert!(work.is_expired());
///assert!(EXPIRED.load(Ordering::Acquire));
///```
///
pub trait SyncTimer: Timer {
    ///Initializes timer state, performing initial arming and allowing to access `TimerState`
    ///during initialization
    ///
    ///The state can be used to register callback and check whether timer has notified user using
    ///configured callback
    ///
    ///If `Timer` is already armed, then `TimerState` is granted as it is.
    fn init<R, F: Fn(&TimerState) -> R>(&mut self, init: F) -> R;

    ///Ticks timer.
    ///
    ///If timer is not started yet, starts returning false.
    ///Otherwise performs necessary actions, if any, to drive timer
    ///and returns whether it is expired.
    ///
    ///Default implementation initializes timer state, if necessary, and
    ///returns whether timer has expired or not.
    #[inline(always)]
    fn tick(&mut self) -> bool {
        self.init(|state| state.is_done())
    }
}

#[inline(always)]
fn poll_sync<T: SyncTimer>(timer: &mut T, ctx: &mut task::Context) -> task::Poll<()> {
    timer.init(|state| {
        state.register(ctx.waker());
        match state.is_done() {
            true => task::Poll::Ready(()),
            false => task::Poll::Pending
        }
    })
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::WinTimer;
#[cfg(windows)]
///Platform alias to Windows timer
pub type Platform = win::WinTimer;
#[cfg(windows)]
///Platform alias to Windows timer
pub type SyncPlatform = win::WinTimer;

#[cfg(all(feature = "tokio1", unix))]
mod async_tokio1;
#[cfg(all(feature = "tokio1", unix))]
pub use async_tokio1::AsyncTimer;
#[cfg(all(feature = "tokio1", unix))]
///Timer based on tokio's `AsyncFd`
pub type Platform = AsyncTimer;

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
mod posix;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
pub use posix::PosixTimer;
#[cfg(all(not(feature = "tokio1"), not(any(target_os = "macos", target_os = "ios")), unix))]
///Platform alias to POSIX timer
pub type Platform = posix::PosixTimer;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
///Platform alias to POSIX Timer
pub type SyncPlatform = posix::PosixTimer;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use apple::AppleTimer;
#[cfg(all(not(feature = "tokio1"), any(target_os = "macos", target_os = "ios")))]
///Platform alias to Apple Dispatch timer
pub type Platform = apple::AppleTimer;
#[cfg(any(target_os = "macos", target_os = "ios"))]
///Platform Alias to `kqueue` based Timer
pub type SyncPlatform = apple::AppleTimer;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebTimer;
#[cfg(target_arch = "wasm32")]
///Platform alias to WASM Timer
pub type Platform = web::WebTimer;
#[cfg(target_arch = "wasm32")]
///Platform alias to WASM Timer
pub type SyncPlatform = web::WebTimer;

mod dummy;
pub use dummy::DummyTimer;
#[cfg(not(any(windows, target_arch = "wasm32", unix)))]
///Platform alias to Dummy Timer as no OS implementation is available.
pub type Platform = dummy::DummyTimer;
#[cfg(not(any(windows, target_arch = "wasm32", unix)))]
///Platform alias to Dummy Timer as no OS implementation is available.
pub type SyncPlatform = dummy::DummyTimer;

#[inline]
///Creates new timer, timer type depends on platform.
pub const fn new_timer(timeout: time::Duration) -> Platform {
    Platform::new(timeout)
}

#[inline]
///Creates new timer, which always implements `SyncTimer`
pub const fn new_sync_timer(timeout: time::Duration) -> SyncPlatform {
    SyncPlatform::new(timeout)
}
