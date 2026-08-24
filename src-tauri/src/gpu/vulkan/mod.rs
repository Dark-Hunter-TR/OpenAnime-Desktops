// gpu/vulkan/mod.rs
pub mod probe;
#[allow(unused_imports)] // yalnızca Linux build'inde gpu/mod.rs'ten çağrılıyor
pub use probe::inner::run_vulkan_probe;
