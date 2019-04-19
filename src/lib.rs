//! Async timer lib

#![feature(futures_api)]

#![warn(missing_docs)]

#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

use core::{task, time};

#[macro_use]
mod utils;
///Timer's state.
pub mod state;
///Builtin implementations of `Timer`
pub mod provider;
///Delay's future
pub mod delay;
//pub mod timed;

pub use state::TimerState;
pub use delay::Delay;

///Platform selected timer alias
#[cfg(windows)]
pub type PlatformTimer = provider::win::WinTimer;

///Describes `Timer` interface
pub trait Timer: Send + Sync + Unpin {
    ///Creates new instance
    fn new(state: *const TimerState) -> Self;

    ///Resets timer, and cancells ongoing work, if necessary
    fn reset(&mut self);

    ///Starts timer as one-shot with specified timeout.
    fn start_delay(&mut self, timeout: time::Duration);

    ///Starts timer as periodic with specified interval.
    fn start_interval(&mut self, interval: time::Duration);

    ///Accesses state
    fn state(&self) -> &TimerState;

    #[inline]
    ///Registers waker with `Timer`
    fn register_waker(&self, waker: &task::Waker) {
        self.state().register(waker)
    }
}
