// gpu/windows/mod.rs
#[cfg(target_os = "windows")]
pub mod detector;

#[cfg(target_os = "windows")]
pub use detector::*;
