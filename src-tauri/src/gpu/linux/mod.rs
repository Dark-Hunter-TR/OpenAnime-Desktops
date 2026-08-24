// gpu/linux/mod.rs
// Tüketiciler `linux::detector::...` tam yolunu kullanır; glob re-export gereksiz.
#[cfg(target_os = "linux")]
pub mod detector;
#[cfg(target_os = "linux")]
pub mod webkit;
