//! Timer implementations

#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub use win::WinTimer;

pub mod dummy;
pub use dummy::DummyTimer;
