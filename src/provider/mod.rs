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

#[cfg(all(feature = "romio_on", any(target_os = "linux", target_os = "android")))]
pub mod timer_fd;
#[cfg(all(feature = "romio_on", any(target_os = "linux", target_os = "android")))]
pub use timer_fd::TimerFd;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use apple::AppleTimer;

pub mod dummy;
pub use dummy::DummyTimer;
