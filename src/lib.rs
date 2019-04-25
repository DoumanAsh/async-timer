//! Async timer lib

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
