// gpu/windows/mod.rs
// Tüketiciler `windows::detector::...` tam yolunu kullanır; glob re-export gereksiz.
#[cfg(target_os = "windows")]
pub mod detector;
