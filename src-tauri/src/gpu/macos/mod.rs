// gpu/macos/mod.rs
#[cfg(target_os = "macos")]
pub mod detector;

#[cfg(target_os = "macos")]
pub use detector::*;
