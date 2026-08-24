// ═══════════════════════════════════════════════════════════════════════════════
// gpu/windows/detector.rs — Windows GPU bilgi toplayıcı
//
// Windows'ta GPU bilgileri DXGI ve registry üzerinden toplanır.
// wgpu backend seçimi backend/selector.rs'de yapılır.
// ═══════════════════════════════════════════════════════════════════════════════

use crate::gpu::diagnostics::types::*;

pub struct WindowsGpuDetection {
    pub vendor: GpuVendor,
    pub renderer: String,
    pub driver_version: String,
    pub display_server: DisplayServer,
    pub log: Vec<LogEntry>,
}

pub fn detect() -> WindowsGpuDetection {
    let mut log = Vec::new();

    // Windows'ta display server kavramı yok.
    let display_server = DisplayServer::Unknown;

    // wgpu bu platformda bağımlı değildir (yalnızca Linux target'ında derlenir),
    // bu yüzden adapter enumeration yapılmaz. Render yolu WebView2 (Chromium) →
    // Direct3D 12 üzerinden yürür. GPU tercihi lib.rs::setup_windows_gpu_preference
    // ile "High Performance" olarak registry'ye yazılır.
    log.push(LogEntry::ok_detail(
        "Windows GPU",
        "WebView2 (Chromium / Direct3D 12) render yolu",
    ));

    WindowsGpuDetection {
        vendor: GpuVendor::Unknown,
        renderer: "WebView2 (Chromium / Direct3D 12)".to_string(),
        driver_version: "Unknown".to_string(),
        display_server,
        log,
    }
}
