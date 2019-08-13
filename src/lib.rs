//! Async timer lib
//!
//! ## Timers
//!
//! - [Oneshot](oneshot/trait.Oneshot.html) interface to one-shot [Timer](oneshot/type.Timer.html)
//!
//! ## Primitives
//!
//! - [Timed](timed/struct.Timed.html) - A wrapper over future that allows to limit time for the future to resolve.
//! - [Interval](interval/struct.Interval.html) - Periodic timer, that on each completition returns itself to poll once again with the same interval.
//!
//! ## Features
//!
//! - `tokio_on` - Enables implementations that require platform's event loop
#![warn(missing_docs)]

#![cfg_attr(feature = "no_std", no_std)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

#[cfg(feature = "no_std")]
#[allow(unused)]
extern crate alloc;
#[cfg(not(feature = "no_std"))]
#[allow(unused)]
use std as alloc;

#[macro_use]
mod utils;
pub mod oneshot;
pub mod timed;
pub mod interval;

pub use oneshot::Oneshot;
pub use timed::Timed;
pub use interval::Interval;
