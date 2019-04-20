//! Timer implementations

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebTimer;

#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub use win::WinTimer;

pub mod dummy;
pub use dummy::DummyTimer;
