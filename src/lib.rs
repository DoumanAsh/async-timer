//! Async timer lib
//!
//! ## Timers
//!
//! - [Timer](timer/trait.Timer.html) interface to one-shot [Platform Timer](timer/type.Platform.html), may require event loop.
//! - [SyncTimer](timer/trait.SyncTimer.html) interface to one-shot [Platform Timer](timer/type.SyncPlatform.html), does not require event loop.
//!
//! ## Primitives
//!
//! - [Timed](enum.Timed.html) - A wrapper over future that allows to limit time for the future to resolve.
//! - [Interval](struct.Interval.html) - Periodic timer, that on each completition returns itself to poll once again with the same interval.
//!
//! ## Features
//!
//! - `tokio02` - Enables async timers using tokio 0.2
#![warn(missing_docs)]

#![no_std]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

#[allow(unused_imports)]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::{time, future};

#[macro_use]
mod utils;
pub mod state;
pub mod timer;
mod timed;
mod interval;

pub use state::Callback;
pub use timer::{SyncTimer, Timer, new_sync_timer, new_timer};
pub use timed::{Timed, Expired};
pub use interval::Interval;

///Run future in timed fashion using default Platform timer.
pub fn timed<F: future::Future>(job: F, timeout: time::Duration) -> impl future::Future<Output=Result<F::Output, Expired<F, timer::Platform>>> {
    unsafe {
        Timed::platform_new_unchecked(job, timeout)
    }
}

///Creates interval with default Platform timer.
pub fn interval(interval: time::Duration) -> Interval<timer::Platform> {
    Interval::platform_new(interval)
}
