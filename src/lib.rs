//! Async timer lib
//!
//! ## Accuracy
//!
//! Regular timers that do not rely on async event loop tend to be on par with user space timers
//! like in `tokio`.
//! If that's not suitable for you you should enable event loop based timers which in most cases
//! give you the most accurate timers possible on unix platforms (See features.)
//!
//! ## Timers
//!
//! - [Timer](timer/trait.Timer.html) interface to one-shot [Platform Timer](timer/type.Platform.html), may require event loop.
//! - [SyncTimer](timer/trait.SyncTimer.html) interface to one-shot [Platform Timer](timer/type.SyncPlatform.html), does not require event loop.
//!
//! ## Primitives
//!
//! - [Timed](struct.Timed.html) - A wrapper over future that allows to limit time for the future to resolve.
//! - [Interval](struct.Interval.html) - Periodic timer, that on each completition returns itself to poll once again with the same interval.
//!
//! ## Features
//!
//! - `tokio1` - Enables event loop based timers using tokio, providing higher resolution timers on unix platforms.
//! - `c_wrapper` - Uses C shim to create bindings to platform API, which may be more reliable than `libc`.
//! - `std` - Enables usage of std types (e.g. Error)
//! - `stream` - Enables `Stream` implementation for `Interval`
#![warn(missing_docs)]

#![no_std]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

#[allow(unused_imports)]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::time;
use core::pin::Pin;
use core::future::Future;

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

#[inline(always)]
///Creates timed future with default Platform timer.
pub fn timed<'a, F: Future>(fut: Pin<&'a mut F>, timeout: time::Duration) -> Timed<'a, F, timer::Platform> {
    Timed::platform_new(fut, timeout)
}

#[inline(always)]
///Creates interval with default Platform timer.
pub fn interval(interval: time::Duration) -> Interval<timer::Platform> {
    Interval::platform_new(interval)
}
