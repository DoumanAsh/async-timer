//! Timer implementations

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebTimer;

#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub use win::WinTimer;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod posix;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use posix::PosixTimer;

pub mod dummy;
pub use dummy::DummyTimer;
