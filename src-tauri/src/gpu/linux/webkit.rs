// gpu/linux/webkit.rs
// Çalışma zamanında linklenen webkit2gtk-4.1 kütüphanesinin sürümünü okur.
// Bu semboller webkit2gtk 2.6'dan beri mevcut ve GTK init öncesi çağrılması
// güvenlidir (yalnızca derleme sabitlerini döndürürler).

extern "C" {
    fn webkit_get_major_version() -> u32;
    fn webkit_get_minor_version() -> u32;
    fn webkit_get_micro_version() -> u32;
}

/// Yüklü webkit2gtk-4.1 sürümü (major, minor, micro).
pub fn version() -> (u32, u32, u32) {
    unsafe {
        (
            webkit_get_major_version(),
            webkit_get_minor_version(),
            webkit_get_micro_version(),
        )
    }
}

/// webkit2gtk >= 2.44 sürümlerinde DMABUF renderer'ın Arch/Fedora gibi
/// güncel dağıtımlarda (AMD/Intel dahil) yaygın çökme ve beyaz ekran
/// sorunları raporlanmıştır. Bu sürümlerde varsayılan olarak DMABUF
/// renderer devre dışı bırakılmalıdır.
pub fn dmabuf_renderer_is_risky() -> bool {
    let (major, minor, _) = version();
    major > 2 || (major == 2 && minor >= 44)
}
