// gpu/linux/mod.rs
#[cfg(target_os = "linux")]
pub mod detector;

#[cfg(target_os = "linux")]
pub use detector::*;
