// gpu/macos/mod.rs
// Tüketiciler `macos::detector::...` tam yolunu kullanır; glob re-export gereksiz.
#[cfg(target_os = "macos")]
pub mod detector;
