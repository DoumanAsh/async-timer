//! Async timer lib
//!
//! ## Timers
//!
//! - [Oneshot](oneshot/trait.Oneshot.html) interface to one-shot [Timer](oneshot/type.Timer.html)
//!
//! ## Primitives
//!
//! - [Timed](enum.Timed.html) - A wrapper over future that allows to limit time for the future to resolve.
//! - [Interval](enum.Interval.html) - Periodic timer, that on each completition returns itself to poll once again with the same interval.
//!
//! ## Features
//!
//! - `tokio_on` - Enables implementations that require platform's event loop
#![warn(missing_docs)]

#![no_std]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

#[allow(unused)]
extern crate alloc;
#[cfg(not(feature = "no_std"))]
extern crate std;

use core::{time, future};

#[macro_use]
mod utils;
pub mod oneshot;
mod timed;
mod interval;

pub use oneshot::Oneshot;
pub use timed::{Timed, Expired};
pub use interval::Interval;

///Run future in timed fashion using default Platform timer.
pub fn timed<F: future::Future>(job: F, timeout: time::Duration) -> impl future::Future<Output=Result<F::Output, Expired<F, oneshot::Timer>>> {
    unsafe {
        Timed::platform_new_unchecked(job, timeout)
    }
}

///Creates interval with default Platform timer.
pub fn interval(interval: time::Duration) -> impl future::Future<Output=Interval<oneshot::Timer>> {
    Interval::platform_new(interval)
}
