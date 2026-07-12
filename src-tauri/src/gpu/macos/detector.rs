// ═══════════════════════════════════════════════════════════════════════════════
// gpu/macos/detector.rs — macOS GPU bilgi toplayıcı
//
// macOS'ta Metal üzerinden GPU bilgileri toplanır.
// ═══════════════════════════════════════════════════════════════════════════════

use crate::gpu::diagnostics::types::*;

pub struct MacosGpuDetection {
    pub vendor: GpuVendor,
    pub renderer: String,
    pub driver_version: String,
    pub display_server: DisplayServer,
    pub log: Vec<LogEntry>,
}

pub fn detect() -> MacosGpuDetection {
    let mut log = Vec::new();

    let display_server = DisplayServer::Unknown;

    // wgpu bu platformda bağımlı değildir (yalnızca Linux target'ında derlenir),
    // bu yüzden Metal adapter enumeration yapılmaz. Render yolu WKWebView →
    // Metal üzerinden yürür.
    log.push(LogEntry::ok_detail(
        "macOS GPU",
        "WKWebView (Metal) render yolu",
    ));

    MacosGpuDetection {
        vendor: GpuVendor::Unknown,
        renderer: "WKWebView (Metal)".to_string(),
        driver_version: "Metal".to_string(),
        display_server,
        log,
    }
}
