//! Async timer lib

#![feature(futures_api)]

#![warn(missing_docs)]

#![cfg_attr(feature = "no_std", no_std)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

#[cfg(feature = "no_std")]
extern crate alloc;
#[cfg(not(feature = "no_std"))]
use std as alloc;

use core::{task, time};

#[macro_use]
mod utils;
///Timer's state.
pub mod state;
///Builtin implementations of `Timer`
pub mod provider;
///Delaying future
pub mod delay;
///Timed wrapper for futures
pub mod timed;

pub use state::TimerState;
pub use delay::Delay;
pub use timed::Timed;

#[cfg(windows)]
///Windows timer alias.
pub type PlatformTimer = provider::win::WinTimer;

#[cfg(target_arch = "wasm32")]
///Web based timer alias.
pub type PlatformTimer = provider::web::WebTimer;

#[cfg(any(target_os = "linux", target_os = "android"))]
///Posix based timer alias.
pub type PlatformTimer = provider::posix::PosixTimer;

#[cfg(any(target_os = "macos", target_os = "ios"))]
///Apple based timer alias.
pub type PlatformTimer = provider::apple::AppleTimer;

#[cfg(not(any(windows, target_arch = "wasm32", target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios")))]
///Dummy timer alias
pub type PlatformTimer = provider::dummy::DummyTimer;

///Describes `Timer` interface
pub trait Timer: Send + Sync + Unpin {
    ///Creates new instance
    ///
    ///[TimerState](state/struct.TimerState.html) is provided as pointer
    ///due to it being managed by abstraction that uses `Timer`
    ///It must be valid non-null pointer and is intended to be used with callbacks
    ///that accept pointer for user's provided data
    fn new(state: *const TimerState) -> Self;

    ///Resets timer, and cancells ongoing work, if necessary
    fn reset(&mut self);

    ///Starts timer as one-shot with specified timeout.
    ///
    ///After timer is expired, `TimerState` will remove registered waker, if any
    ///
    ///Cancels any non-expired jobs
    fn start_delay(&mut self, timeout: time::Duration);

    ///Starts timer as periodic with specified interval.
    ///
    ///Cancels any non-expired jobs
    fn start_interval(&mut self, interval: time::Duration);

    ///Accesses state
    fn state(&self) -> &TimerState;

    #[inline]
    ///Registers waker with `Timer`
    fn register_waker(&self, waker: &task::Waker) {
        self.state().register(waker)
    }
}